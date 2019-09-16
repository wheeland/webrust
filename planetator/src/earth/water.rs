use cgmath::prelude::*;
use cgmath::*;

pub struct WaterPlateFactory {
    water_depth: u32,  // number of vertices along the water plate edge is 2^depth
    plate_depth: u32,  // number of texels in plate vertices
    indices: tinygl::IndexBuffer,
    tex_coords: tinygl::VertexBuffer,
    ribbons: tinygl::VertexBuffer,
}

pub struct WaterPlate (
    pub u32,
    pub u32,
    pub tinygl::VertexBuffer,
);

impl WaterPlateFactory {
    /// Generate buffer that contains the 'ribbon' flag and that matches the plate's pos/height buffer
    fn gen_ribbon_buffer(plate_depth: u32) -> tinygl::VertexBuffer {
        let mut data = Vec::new();
        let vertsize = 2i16.pow(plate_depth);

        for y in 0..vertsize+3 {
            for x in 0..vertsize+3 {
                let ribbon = (x == 0) || (y == 0) || (x == vertsize+2) || (y == vertsize+2);
                data.push(if ribbon { 255u8 } else { 0u8 });
            }
        }

        tinygl::VertexBuffer::from(&data)
    }

    /// Generate index buffer for rendering of water-plates, with ribbons
    fn gen_indices(water_depth: u32, plate_depth: u32, ribbons: bool) -> tinygl::IndexBuffer {
        let mut indices = Vec::new();
        let vertsize = 2i16.pow(plate_depth);
        let water_depth = water_depth.min(plate_depth);
        let step = 2i16.pow((plate_depth as i32 - water_depth as i32) as u32);

        {
            let mut add = |x0, y0, sx, sy| {
                let y0base = (vertsize + 3) * y0;
                let y1base = (vertsize + 3) * (y0 + sy);
                let i00 = (y0base + x0) as u16;
                let i01 = (y1base + x0) as u16;
                let i10 = (y0base + x0 + sx) as u16;
                let i11 = (y1base + x0 + sx) as u16;
                indices.push(i00);
                indices.push(i01);
                indices.push(i11);
                indices.push(i11);
                indices.push(i10);
                indices.push(i00);
            };

            // add normal plate triangles
            for y in (0..vertsize).step_by(step as _) {
                for x in (0..vertsize).step_by(step as _) {
                    add(x + 1, y + 1, step, step);
                }
            }

            if ribbons {
                // add ribbons
                for v in (0..vertsize).step_by(step as _) {
                    add(0, v+1, 1, step);
                    add(vertsize+1, v+1, 1, step);
                    add(v+1, 0, step, 1);
                    add(v+1, vertsize+1, step, 1);
                }

                // add ribbon corners
                add(0, 0, 1, 1);
                add(vertsize+1, 0, 1, 1);
                add(0, vertsize+1, 1, 1);
                add(vertsize+1, vertsize+1, 1, 1);
            }
        }

        tinygl::IndexBuffer::from16(&indices)
    }

    pub fn new(water_depth: u32, plate_depth: u32, texture_delta: u32) -> Self {
        let tex_coords = super::util::generate_tex_coords_buffer(plate_depth, plate_depth, texture_delta);
        let tex_coords = tinygl::VertexBuffer::from(&tex_coords);

        Self {
            water_depth,
            plate_depth,
            tex_coords,
            indices: Self::gen_indices(water_depth, plate_depth, true),
            ribbons: Self::gen_ribbon_buffer(plate_depth),
        }
    }

    pub fn indices(&self) -> &tinygl::IndexBuffer {
        &self.indices
    }

    pub fn tex_coords(&self) -> &tinygl::VertexBuffer {
        &self.tex_coords
    }

    pub fn ribbons(&self) -> &tinygl::VertexBuffer {
        &self.ribbons
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

    /// Create vertex buffer with sphere coordinates for given position, including ribbon flag
    /// (this is outdated, we ain't doin that anymore)
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
                vertices.push(if xborder || yborder { 1.0 } else { 0.0 });

                vx += inv_vert_size;
            }

            vy += inv_vert_size;
        }

        tinygl::VertexBuffer::from(&vertices)
    }
}