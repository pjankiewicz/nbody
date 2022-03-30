use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    render::camera::OrthographicProjection,
};
use bevy_egui::EguiContext;

/// This plugin is taken from PanCamPLugin with added egui interaction
/// Unfortunately bevy does not allow conditional plugin.

#[derive(Default)]
pub struct PanCamPlugin;

impl Plugin for PanCamPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(camera_movement).add_system(camera_zoom);
    }
}

// Zoom doesn't work on bevy 0.5 due to: https://github.com/bevyengine/bevy/pull/2015
fn camera_zoom(
    mut egui_ctx: ResMut<EguiContext>,
    mut query: Query<(&PanCam, &mut OrthographicProjection)>,
    mut scroll_events: EventReader<MouseWheel>,
) {
    if egui_ctx.ctx_mut().wants_pointer_input()
        || egui_ctx.ctx_mut().wants_keyboard_input()
        || egui_ctx.ctx_mut().is_pointer_over_area()
    {
        return;
    }

    let pixels_per_line = 100.; // Maybe make configurable?
    let scroll = scroll_events
        .iter()
        .map(|ev| match ev.unit {
            MouseScrollUnit::Pixel => ev.y,
            MouseScrollUnit::Line => ev.y * pixels_per_line,
        })
        .sum::<f32>();

    if scroll == 0. {
        return;
    }

    for (_cam, mut projection) in query.iter_mut() {
        projection.scale = (projection.scale * (1. + -scroll * 0.001)).max(0.00001);
    }
}

fn camera_movement(
    mut egui_ctx: ResMut<EguiContext>,
    mut windows: ResMut<Windows>,
    mouse_buttons: Res<Input<MouseButton>>,
    mut query: Query<(&PanCam, &mut Transform, &OrthographicProjection)>,
    mut last_pos: Local<Option<Vec2>>,
) {
    if egui_ctx.ctx_mut().wants_pointer_input()
        || egui_ctx.ctx_mut().wants_keyboard_input()
        || egui_ctx.ctx_mut().is_pointer_over_area()
    {
        return;
    }

    let window = windows.get_primary_mut().unwrap();

    // Use position instead of MouseMotion, otherwise we don't get acceleration movement
    let current_pos = match window.cursor_position() {
        Some(current_pos) => current_pos,
        None => return,
    };
    let delta = current_pos - last_pos.unwrap_or(current_pos);

    for (cam, mut transform, projection) in query.iter_mut() {
        if cam
            .grab_buttons
            .iter()
            .any(|btn| mouse_buttons.pressed(*btn))
        {
            let scaling = Vec2::new(
                window.width() / (projection.right - projection.left),
                window.height() / (projection.top - projection.bottom),
            ) * projection.scale;

            transform.translation -= (delta * scaling).extend(0.);
        }
    }
    *last_pos = Some(current_pos);
}

#[derive(Component)]
pub struct PanCam {
    grab_buttons: Vec<MouseButton>,
}

impl Default for PanCam {
    fn default() -> Self {
        Self {
            grab_buttons: vec![MouseButton::Left, MouseButton::Right, MouseButton::Middle],
        }
    }
}
