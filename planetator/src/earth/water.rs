use cgmath::prelude::*;
use cgmath::*;

pub struct WaterPlateFactory {
    water_depth: u32,  // number of vertices along the water plate edge is 2^depth
    texture_depth: u32,  // number of texels in plate textures is 2^depth
    indices: tinygl::IndexBuffer,
    tex_coords: tinygl::VertexBuffer,
}

pub struct WaterPlate (
    pub u32,
    pub u32,
    pub tinygl::VertexBuffer,
);

impl WaterPlateFactory {
    /// Generate index buffer for rendering of water-plates, with ribbons
    fn gen_indices(water_depth: u32, ribbons: bool) -> tinygl::IndexBuffer {
        let mut indices = Vec::new();
        let size = 2i16.pow(water_depth);
        let ofs = if ribbons { 1 } else { 0 };

        for y in -ofs..size+ofs {
            let y0base = (size + 3) * (y + 1);
            let y1base = (size + 3) * (y + 2);
            for x in -ofs..size+ofs {
                let i00 = (y0base + x + 1) as u16;
                let i01 = (y1base + x + 1) as u16;
                let i10 = (y0base + x + 2) as u16;
                let i11 = (y1base + x + 2) as u16;
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

    pub fn new(water_depth: u32, texture_depth: u32, texture_delta: u32) -> Self {
        let tex_coords = super::util::generate_tex_coords_buffer(water_depth, texture_depth, texture_delta);
        let tex_coords = tinygl::VertexBuffer::from(&tex_coords);

        Self {
            water_depth,
            texture_depth,
            tex_coords,
            indices: Self::gen_indices(water_depth, false),
        }
    }

    pub fn indices(&self) -> &tinygl::IndexBuffer {
        &self.indices
    }

    pub fn tex_coords(&self) -> &tinygl::VertexBuffer {
        &self.tex_coords
    }

    pub fn update(&self, position: &super::plate::Position, water_plate: &mut Option<WaterPlate>) {
        // create water plate, or update it, if it has the wrong dimensions
        let needs_creation = water_plate.as_ref().map(|water_plate|
            water_plate.0 != self.water_depth || water_plate.1 != self.texture_depth
        ).unwrap_or(true);

        if needs_creation {
            water_plate.replace(WaterPlate(self.water_depth, self.texture_depth, self.create(position)));
        }
    }

    /// Create vertex buffer with sphere coordinates for given position, including ribbon flag
    pub fn create(&self, plate: &super::plate::Position) -> tinygl::VertexBuffer {
        let mut vertices = Vec::new();
        let inv_vert_size = 1.0 / 2.0f32.powi(self.water_depth as _);
        let water_plate_size = 2i32.pow(self.water_depth);

        let mut vy = -inv_vert_size;

        for y in -1..water_plate_size+2 {
            let mut vx = -inv_vert_size;
            let yborder = (y == -1) || (y == water_plate_size + 1);

            for x in -1..water_plate_size+2 {
                let xborder = (x == -1) || (x == water_plate_size + 1);
                let sphere = plate.uv_to_sphere(&Vector2::new(vx, vy));
                vertices.push(sphere.x);
                vertices.push(sphere.y);
                vertices.push(sphere.z);
                // vertices.push(if xborder || yborder { 1.0 } else { 0.0 });

                vx += inv_vert_size;
            }

            vy += inv_vert_size;
        }

        tinygl::VertexBuffer::from(&vertices)
    }
}