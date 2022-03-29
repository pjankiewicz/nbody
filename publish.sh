cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --out-dir ./out/ --target web ./target/wasm32-unknown-unknown/release/nbody.wasm
cp out/*.* ../pjankiewicz.github.io/nbody/out
cd ../pjankiewicz.github.io
git add -A
git commit -m "Nbody update"
git push
