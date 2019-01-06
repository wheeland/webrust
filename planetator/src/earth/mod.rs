use std::collections::HashMap;

pub mod flycamera;
pub mod renderer;
mod plate;
mod tree;
mod generator;
mod noise;
mod plateoptimizer;

#[derive(Clone)]
pub struct Channels {
    channels: HashMap<String, usize>,
}

impl Channels {
    pub fn new(chans: &Vec<(String, i32)>) -> Self {
        let mut channels = HashMap::new();

        for chan in chans {
            channels.insert(chan.0.clone(), chan.1 as usize + 1);
        }

        Self {
            channels,
        }
    }

    pub fn channels(&self) -> &HashMap<String, usize> {
        &self.channels
    }

    pub fn to_glsl_type(size: usize) -> String {
        String::from(match size {
            1 => "float",
            2 => "vec2",
            3 => "vec3",
            4 => "vec4",
            _ => panic!("Does not compute")
        })
    }

    pub fn declarations(&self) -> Vec<String> {
        self.channels.iter().map(|entry| {
            Self::to_glsl_type(*entry.1) + " " + entry.0
        }).collect()
    }
}

pub struct Configuration {
    pub size: usize,
    pub detail: u8,
    pub radius: f32,
    pub generator: String,
    pub channels: Channels
}