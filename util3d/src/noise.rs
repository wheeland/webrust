pub struct ShaderNoise {
}

impl ShaderNoise {
    pub fn definitions() -> & 'static str {
        include_str!("noise_definitions.glsl.in")
    }

    pub fn declarations() -> & 'static str {
        include_str!("noise_declarations.glsl.in")
    }
}