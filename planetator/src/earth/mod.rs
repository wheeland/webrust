pub mod renderer;
mod plate;
mod tree;
mod water;
mod util;
mod generator;
mod channels;
mod plateoptimizer;

struct Configuration {
    pub size: usize,
    pub detail: u8,
    pub radius: f32,
    pub generator: String,
    pub channels: channels::Channels,
}
