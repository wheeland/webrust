use std::collections::HashMap;

#[derive(Clone)]
pub struct Channels {
    channels: HashMap<String, usize>,
}

impl Channels {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new()
        }
    }

    pub fn from(channels: &HashMap<String, usize>) -> Self {
        Self {
            channels: channels.clone(),
        }
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<String, usize> {
        self.channels.iter()
    }

    fn to_glsl_type(size: usize) -> String {
        String::from(match size {
            1 => "float",
            2 => "vec2",
            3 => "vec3",
            4 => "vec4",
            _ => panic!("Does not compute: size = {}", size)
        })
    }

    fn swizzle_mask(size: usize) -> String {
        String::from(match size {
            1 => "r",
            2 => "rg",
            3 => "rgb",
            4 => "rgba",
            _ => panic!("Does not compute: size = {}", size)
        })
    }

    pub fn glsl_base_declarations(&self) -> Vec<String> {
        self.channels.iter().map(|entry| {
            Self::to_glsl_type(*entry.1) + " " + entry.0
        }).collect()
    }

    pub fn glsl_output_declarations(&self, first_output_unit: usize) -> String {
        self.glsl_base_declarations()
            .iter()
            .enumerate()
            .map(|chan| {
                format!("layout(location = {}) out {};\n", chan.0 + first_output_unit, chan.1)
            })
            .fold(String::new(), |acc, x| acc + &x)
    }

    pub fn glsl_texture_declarations(&self) -> String {
        self.channels.iter().fold(
            String::new(), |acc, chan| {
                acc + "uniform sampler2D _channel_texture_" + chan.0 + ";\n"
            }
        )
    }

    pub fn glsl_assignments(&self, tex_coords_name: &str) -> String {
        self.channels.iter().fold(
            String::new(), |acc, chan| {
                let swizzler = Self::swizzle_mask(*chan.1);
                acc + chan.0 + " = texture(_channel_texture_" + chan.0 + ", " + tex_coords_name + ")." + &swizzler + ";\n"
            }
        )
    }
}