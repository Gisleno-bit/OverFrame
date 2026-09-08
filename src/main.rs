//! OVERFRAME entry point.
//!
//! With the default `gui` feature this launches the macroquad game (local
//! versus + training mode). Built without `gui` it just prints how to build.

#[cfg(feature = "gui")]
fn main() {
    overframe::render::launch();
}

#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!(
        "overframe was built without the `gui` feature.\n\
         Build the playable game with:  cargo run --release --features gui\n\
         (the `gui` feature is on by default: `cargo run --release`)"
    );
}
