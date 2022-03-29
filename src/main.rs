use bevy::diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContext, EguiPlugin};
use bevy_fly_camera::{FlyCamera2d, FlyCameraPlugin};
use bevy_pancam::{PanCam, PanCamPlugin};
use bevy_prototype_lyon::prelude::*;
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const TIME_STEP: f32 = 1.0 / 120.0;
const BIG_G: f32 = 3.5;
const SOFTENING: f32 = 1.0;

#[derive(Default)]
struct Stats {
    time_step: f32,
    frame_number: usize,
    n_objects: usize,
    center_on_largest: bool,
    draw_traces: bool,
    largest_position: Vec2,
}

#[derive(Clone)]
struct Settings {
    n_objects: usize,
    collisions: bool,
    min_planet_size: f32,
    max_planet_size: f32,
    min_planet_density: f32,
    max_planet_density: f32,
    min_planet_orbit_radius: f32,
    max_planet_orbit_radius: f32,
    sun_size: f32,
    sun_density: f32
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            n_objects: 500,
            collisions: true,
            min_planet_size: 0.5,
            max_planet_size: 3.5,
            min_planet_density: 0.5,
            max_planet_density: 2.0,
            min_planet_orbit_radius: 100.0,
            max_planet_orbit_radius: 1000.0,
            sun_size: 30.0,
            sun_density: 5.0
        }
    }
}

struct ClearTraces;
struct Reset;

#[derive(Component, Debug, Clone)]
struct Velocity {
    velocity: Vec2,
}

#[derive(Component, Debug, Clone)]
struct Planet {
    radius: f32,
    density: f32,
    color: Color,
    is_sun: bool,
}

#[derive(Component)]
struct Trace {
    live_until: f64,
}

impl Planet {
    pub fn mass(&self) -> f32 {
        self.density * (4.0 / 3.0) * PI * self.radius.powf(3.0)
    }
}

fn move_camera(mut camera: Query<&mut Transform, With<Camera>>, stats: Res<Stats>) {
    for mut transform in camera.iter_mut() {
        if stats.center_on_largest {
            transform.translation.x = stats.largest_position.x;
            transform.translation.y = stats.largest_position.y;
        }
    }
}

fn gravity(
    mut commands: Commands,
    settings: Res<Settings>,
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut planet_query: Query<(Entity, &mut Planet, &mut Velocity, &mut Transform)>,
    mut stats: ResMut<Stats>,
    time: Res<Time>,
) {
    let mut accel_map: HashMap<u32, Vec2> = HashMap::new();
    let mut despawned = HashSet::new();
    stats.n_objects = 0;
    let mut largest = 0.0;
    let mut largest_position = Vec2::new(0.0, 0.0);
    stats.frame_number += 1;

    for (entity_1, mut planet_1, velocity_1, transform_1) in planet_query.iter() {
        if stats.frame_number % 5 == 0 && stats.draw_traces {
            let mut transform: Transform = transform_1.clone();
            transform.translation.z = 1.0;
            spawn_trace(
                &mut commands,
                transform,
                time.seconds_since_startup() + 10.0,
            );
        }
        if planet_1.radius > largest && !planet_1.is_sun {
            largest = planet_1.radius;
            stats.largest_position = transform_1.translation.truncate();
        }
        stats.n_objects += 1;
        let mut accel_cum = Vec2::new(0.0, 0.0);
        for (entity_2, mut planet_2, velocity_2, transform_2) in planet_query.iter() {
            if entity_1.id() != entity_2.id()
                && !despawned.contains(&entity_1.id())
                && !despawned.contains(&entity_2.id())
            {
                let r_vector =
                    transform_1.translation.truncate() - transform_2.translation.truncate();
                if r_vector.length() < planet_1.radius + planet_2.radius && settings.collisions {
                    let sum_mass = planet_1.mass() + planet_2.mass();
                    let final_velocity = Velocity {
                        velocity: velocity_1.velocity * planet_1.mass() / sum_mass
                            + velocity_2.velocity * planet_2.mass() / sum_mass,
                    };
                    commands.entity(entity_2).despawn();
                    despawned.insert(entity_2.id());
                    commands.entity(entity_1).despawn();
                    despawned.insert(entity_1.id());
                    if planet_1.mass() > planet_2.mass() {
                        spawn_planet(
                            &mut commands,
                            merge_planets(&planet_1, &planet_2),
                            final_velocity,
                            *transform_1,
                        );
                    } else {
                        spawn_planet(
                            &mut commands,
                            merge_planets(&planet_2, &planet_1),
                            final_velocity,
                            *transform_2,
                        );
                    }
                } else {
                    let r_mag = (r_vector + Vec2::new(SOFTENING, SOFTENING)).length();
                    let accel: f32 = (-1.0 * BIG_G * planet_2.mass() / r_mag.powf(2.0));
                    let r_vector_unit = r_vector / r_mag;
                    accel_cum += accel * r_vector_unit;
                }
            }
        }
        accel_map.insert(entity_1.id(), accel_cum);
    }

    for (entity_1, planet_1, mut velocity_1, mut transform_1) in planet_query.iter_mut() {
        if !despawned.contains(&entity_1.id()) {
            velocity_1.velocity += *accel_map.get(&entity_1.id()).unwrap() * TIME_STEP;
            transform_1.translation.x += velocity_1.velocity.x * TIME_STEP;
            transform_1.translation.y += velocity_1.velocity.y * TIME_STEP;
        }
    }
}

fn radius_to_area(r: f32) -> f32 {
    PI * r.powf(2.0)
}

fn area_to_radius(a: f32) -> f32 {
    (a / PI).sqrt()
}

fn merge_planets(planet_1: &Planet, planet_2: &Planet) -> Planet {
    let area_1 = radius_to_area(planet_1.radius);
    let area_2 = radius_to_area(planet_2.radius);
    let area_sum = area_1 + area_2;
    let new_radius = area_to_radius(area_sum);
    Planet {
        radius: new_radius,
        density: planet_1.density * (area_1 / area_sum) + planet_2.density * (area_2 / area_sum),
        color: planet_1.color.clone(),
        is_sun: planet_1.is_sun || planet_2.is_sun,
    }
}

fn merge_planets_radius(planet_1: &Planet, planet_2: &Planet) -> f32 {
    let area_1 = planet_1.radius;
    let area_2 = planet_1.radius;
    let area_sum = area_1 + area_2;
    area_to_radius(area_sum)
}

fn despawn_traces(
    mut ev_clear_trace: EventReader<ClearTraces>,
    mut commands: Commands,
    mut traces: Query<(Entity, &Trace)>,
    time: Res<Time>,
) {
    let mut manual_clear = false;
    for _ in ev_clear_trace.iter() {
        manual_clear = true;
    }
    for (entity, trace) in traces.iter() {
        if trace.live_until < time.seconds_since_startup() || manual_clear {
            commands.entity(entity).despawn();
        }
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut stats: ResMut<Stats>) {
    commands
        .spawn_bundle(OrthographicCameraBundle::new_2d())
        .insert(PanCam::default())
        .insert(FlyCamera2d::default());
}

fn setup_many_orbits(
    mut planet_query: Query<(Entity, &mut Planet)>,
    mut ev_reset: EventReader<Reset>,
    settings: Res<Settings>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut stats: ResMut<Stats>,
) {
    let mut manual_reset = false;
    for _ in ev_reset.iter() {
        manual_reset = true;
    }
    if manual_reset {
        for (ent, _) in planet_query.iter() {
            commands.entity(ent).despawn();
        }

        let mut rng = rand::thread_rng();
        let sun = Planet {
            radius: settings.sun_size,
            density: settings.sun_density,
            color: Color::YELLOW,
            is_sun: true,
        };
        spawn_planet(
            &mut commands,
            sun.clone(),
            Velocity {
                velocity: Vec2::new(0.0, 0.0),
            },
            Transform::from_xyz(0.0, 0.0, 10.0),
        );

        for _ in 0..settings.n_objects {
            let planet_radius = rng.gen::<f32>() * (settings.max_planet_size - settings.min_planet_size) + settings.min_planet_size;
            let density: f32 = rng.gen::<f32>() * (settings.max_planet_density - settings.min_planet_density) + settings.min_planet_density;
            let planet = Planet {
                radius: planet_radius,
                density,
                color: Color::WHITE,
                is_sun: false,
            };
            let orbit_radius: f32 = rng.gen::<f32>() * (settings.max_planet_orbit_radius - settings.min_planet_orbit_radius) + settings.min_planet_orbit_radius;
            let radian: f32 = rng.gen::<f32>() * 2.0 * PI;
            let x: f32 = orbit_radius * radian.cos();
            let y: f32 = orbit_radius * radian.sin();
            let orbital_velocity = (BIG_G * sun.mass() / orbit_radius).sqrt();
            let vx: f32 = -orbital_velocity * radian.sin();
            let vy: f32 = orbital_velocity * radian.cos();
            spawn_planet(
                &mut commands,
                planet,
                Velocity {
                    velocity: Vec2::new(vx, vy),
                },
                Transform::from_xyz(x, y, 10.0),
            );
        }
    }
}

fn spawn_planet(commands: &mut Commands, planet: Planet, velocity: Velocity, transform: Transform) {
    let shape = shapes::Circle {
        radius: planet.radius,
        center: Default::default(),
    };
    let mut entity_commands = commands.spawn_bundle(GeometryBuilder::build_as(
        &shape,
        DrawMode::Outlined {
            fill_mode: FillMode::color(planet.color.clone()),
            outline_mode: StrokeMode::new(planet.color.clone(), 0.0),
        },
        transform,
    ));

    entity_commands.insert(planet).insert(velocity);

    let entity_id = entity_commands.id();
}

fn spawn_trace(commands: &mut Commands, transform: Transform, live_until: f64) {
    commands
        .spawn_bundle(SpriteBundle {
            sprite: Sprite {
                color: Color::GRAY,
                custom_size: Some(Vec2::new(1.0, 1.0)),
                ..Default::default()
            },
            transform,
            ..Default::default()
        })
        .insert(Trace { live_until });
}

fn ui_box(
    mut ev_clear_traces: EventWriter<ClearTraces>,
    mut ev_reset: EventWriter<Reset>,
    mut settings: ResMut<Settings>,
    diagnostics: Res<Diagnostics>,
    mut egui_context: ResMut<EguiContext>,
    mut stats: ResMut<Stats>,
    time: Res<Time>,
) {
    egui::Window::new("Moon creator").show(egui_context.ctx_mut(), |ui| {
        if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(average) = fps.average() {
                // Update the value of the second section
                ui.label("WASD to move, drag to move,\nscrool wheel to zoom in/out");
                ui.label(format!("Time {:.2}", time.seconds_since_startup()));
                ui.label(format!("FPS {:.2}", average));
                ui.label(format!("Number of objects {:}", stats.n_objects));
                ui.checkbox(&mut stats.center_on_largest, "Center on the largest");
                ui.checkbox(&mut stats.draw_traces, "Draw traces");
                if ui.button("Clear traces").clicked() {
                    ev_clear_traces.send(ClearTraces);
                };
                ui.label("Simulation settings");
                ui.add(egui::Slider::new(&mut settings.n_objects, 10..=1000).text("Number of planets"));
                ui.checkbox(&mut settings.collisions, "Enable colissions");
                ui.add(egui::Slider::new(&mut settings.min_planet_size, 0.5..=3.0).text("Minimum planet radius"));
                ui.add(egui::Slider::new(&mut settings.max_planet_size, 3.0..=10.0).text("Maximum planet radius"));
                ui.add(egui::Slider::new(&mut settings.min_planet_density, 0.5..=5.0).text("Minimum planet density"));
                ui.add(egui::Slider::new(&mut settings.max_planet_density, 0.5..=5.0).text("Maximum planet density"));
                ui.add(egui::Slider::new(&mut settings.min_planet_orbit_radius, 100.0..=500.0).text("Minimum planet orbit radius"));
                ui.add(egui::Slider::new(&mut settings.max_planet_orbit_radius, 500.0..=2000.0).text("Maximum planet orbit radius"));
                ui.add(egui::Slider::new(&mut settings.sun_size, 30.0..=100.0).text("Sun radius"));
                ui.add(egui::Slider::new(&mut settings.sun_density, 5.0..=100.0).text("Sun density"));
                if ui.button("Start").clicked() {
                    ev_reset.send(Reset);
                }
            }
        }
    });
}

#[wasm_bindgen]
pub fn game() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
        .insert_resource(Settings::default())
        .add_event::<ClearTraces>()
        .add_event::<Reset>()
        .add_plugins(DefaultPlugins)
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_plugin(EguiPlugin)
        .add_plugin(ShapePlugin)
        .add_plugin(FlyCameraPlugin)
        .add_plugin(PanCamPlugin::default())
        .add_startup_system(setup)
        // .add_startup_system(setup_many_orbits)
        .add_system(gravity)
        .add_system(ui_box)
        .add_system(move_camera)
        .add_system(despawn_traces)
        .add_system(setup_many_orbits)
        .insert_resource(Stats::default())
        .run();
}

pub fn main() {
    game()
}
