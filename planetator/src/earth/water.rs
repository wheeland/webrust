use cgmath::prelude::*;
use cgmath::*;

pub struct WaterPlateFactory {
    water_depth: u32,  // number of vertices along the water plate edge is 2^depth
    plate_depth: u32,  // number of vertices along the original plate edge is 2^depth
    indices: tinygl::IndexBuffer,
}

pub struct WaterPlate (
    pub u32,
    pub u32,
    pub tinygl::VertexBuffer,
);

impl WaterPlateFactory {
    fn gen_indices(water_depth: u32) -> tinygl::IndexBuffer {
        let mut indices = Vec::new();
        let size = 2u16.pow(water_depth);

        for y in 0..size {
            let y0base = (size + 1) * y;
            let y1base = (size + 1) * (y + 1);
            for x in 0..size {
                let i00 = y0base + x;
                let i01 = y1base + x;
                let i10 = y0base + x + 1;
                let i11 = y1base + x + 1;
                indices.push(i00);
                indices.push(i01);
                indices.push(i11);
                indices.push(i11);
                indices.push(i10);
                indices.push(i00);
            }
        }

        tinygl::IndexBuffer::from16(&indices)
    }

    pub fn new(water_depth: u32, plate_depth: u32) -> Self {
        Self {
            water_depth,
            plate_depth,
            indices: Self::gen_indices(water_depth),
        }
    }

    pub fn plate_depth(&self) -> u32 {
        self.plate_depth
    }

    pub fn water_depth(&self) -> u32 {
        self.water_depth
    }

    pub fn indices(&self) -> &tinygl::IndexBuffer {
        &self.indices
    }

    pub fn update(&self, position: &super::plate::Position, water_plate: &mut Option<WaterPlate>) {
        // create water plate, or update it, if it has the wrong dimensions
        let needs_creation = water_plate.as_ref().map(|water_plate|
            water_plate.0 != self.water_depth || water_plate.1 != self.plate_depth
        ).unwrap_or(true);

        if needs_creation {
            water_plate.replace(WaterPlate(self.water_depth, self.plate_depth, self.create(position)));
        }
    }

    pub fn create(&self, plate: &super::plate::Position) -> tinygl::VertexBuffer {
        let mut vertices = Vec::new();
        let inv_vert_size = 1.0 / 2.0f32.powi(self.water_depth as _);
        let water_plate_size = 2u32.pow(self.water_depth);

        let mut vy = 0.0;

        // we need to map from the [0..size] space of the water plate into the texture space
        // of the plate texture, which may have a different 2^N depth, and also a 1 pixel border,
        // and which is a texture and not a buffer, and thus needs a 0.5 offset for sampling
        let depth_diff = 2.0f32.powi(self.plate_depth as i32 - self.water_depth as i32);
        let tex_size = 2.0f32.powi(self.plate_depth as _) + 3.0;
        let mut ty = (1.5) / tex_size;

        for y in 0..water_plate_size+1 {
            let mut vx = 0.0;
            let mut tx = 1.5 / tex_size;

            for x in 0..water_plate_size+1 {
                let sphere = plate.uv_to_sphere(&Vector2::new(vx, vy));
                vertices.push(sphere.x);
                vertices.push(sphere.y);
                vertices.push(sphere.z);
                vertices.push(tx);
                vertices.push(ty);
                vx += inv_vert_size;
                tx += depth_diff / tex_size;
            }
            vy += inv_vert_size;
            ty += depth_diff / tex_size;
        }

        tinygl::VertexBuffer::from(&vertices)
    }
}