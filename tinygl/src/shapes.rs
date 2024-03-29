use cgmath::{Vector2, Vector3};

pub struct FullscreenQuad {
    vertices: super::VertexBuffer,
}

impl FullscreenQuad {
    pub fn new() -> Self {
        let mut vertices = Vec::<Vector2<f32>>::new();

        vertices.push(Vector2::new(-1.0, -1.0));
        vertices.push(Vector2::new(1.0,   1.0));
        vertices.push(Vector2::new(-1.0,  1.0));

        vertices.push(Vector2::new(-1.0, -1.0));
        vertices.push(Vector2::new(1.0,  -1.0));
        vertices.push(Vector2::new(1.0,   1.0));

        let vertices = super::VertexBuffer::from(&vertices);

        FullscreenQuad {
            vertices,
        }
    }

    pub fn vertices(&self) -> &super::VertexBuffer {
        &self.vertices
    }

    pub fn render(&self, program: &super::Program, attrname: &str) {
        program.bind();
        program.vertex_attrib_buffer(attrname, &self.vertices, 2, gl::FLOAT, false, 0, 0);
        unsafe { gl::DrawArrays(gl::TRIANGLES, 0, 6) }
        program.disable_all_vertex_attribs();
    }
}

#[derive(Copy)]
#[derive(Clone)]
#[derive(PartialEq)]
pub enum Orientation {
    Clockwise,
    CounterClockwise,
}

pub struct Plane {
    vertices: super::VertexBuffer,
    indices: super::IndexBuffer,
}

impl Plane {
    pub fn new(x_extent: u16, y_extent: u16, orientation: Orientation) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for y in 0..y_extent+1 {
            for x in 0..x_extent+1 {
                vertices.push(x as f32 / x_extent as f32);
                vertices.push(y as f32 / y_extent as f32);
            }
        }

        for y in 0..y_extent {
            let y0base = (x_extent + 1) * y;
            let y1base = (x_extent + 1) * (y + 1);
            for x in 0..x_extent {
                let i00 = y0base + x;
                let i01 = y1base + x;
                let i10 = y0base + x + 1;
                let i11 = y1base + x + 1;
                indices.push(i00);
                indices.push(if orientation == Orientation::Clockwise { i01 } else { i10 });
                indices.push(i11);
                indices.push(i11);
                indices.push(if orientation == Orientation::Clockwise { i10 } else { i01 });
                indices.push(i00);
            }
        }

        Self {
            vertices: super::VertexBuffer::from(&vertices),
            indices: super::IndexBuffer::from16(&indices),
        }
    }

    pub fn bind_vertex_data(&self, program: &super::Program, attrname: &str) {
        program.bind();
        program.vertex_attrib_buffer(attrname, &self.vertices, 2, gl::FLOAT, false, 0, 0);
        self.indices.bind();
    }

    pub fn render(&self) {
        unsafe { gl::DrawElements(gl::TRIANGLES, 0, self.indices.count() as _, std::ptr::null()) }
    }
}

pub struct Cube {
    indices: Vec<u16>,
    vertices: Vec<Vector3<f32>>,
    normals: Vec<Vector3<f32>>,
}

impl Cube {
    pub fn new(dimension: usize) -> Self {
        let dimension = dimension.max(1);

        let mut indices = Vec::new();
        let mut vertices = Vec::new();
        let mut normals = Vec::new();

        for side in 0..6 {
            let norm: Vector3<f32> = match side {
                0 => Vector3::new(1.0,  0.0,  0.0),
                1 => Vector3::new(-1.0, 0.0,  0.0),
                2 => Vector3::new(0.0,  1.0,  0.0),
                3 => Vector3::new(0.0, -1.0,  0.0),
                4 => Vector3::new(0.0,  0.0,  1.0),
                5 => Vector3::new(0.0,  0.0, -1.0),
                _ => unreachable!(),
            };
            let an = Vector3::new(norm.x.abs(), norm.y.abs(), norm.z.abs());

            let dir1 = Vector3::new(an.y, an.z, an.x);
            let dir2 = Vector3::new(an.z, an.x, an.y);

            for x in 0..dimension {
                for y in 0..dimension {
                    let x0 = x as f32 / dimension as f32;
                    let y0 = y as f32 / dimension as f32;
                    let step = 1.0 / dimension as f32;

                    let base = vertices.len() as u16;

                    vertices.push(norm + dir1 * (-1.0 + 2.0 * x0) + dir2 * (-1.0 + 2.0 * y0));
                    vertices.push(norm + dir1 * (-1.0 + 2.0 * x0) + dir2 * (-1.0 + 2.0 * (y0 + step)));
                    vertices.push(norm + dir1 * (-1.0 + 2.0 * (x0 + step)) + dir2 * (-1.0 + 2.0 * y0));
                    vertices.push(norm + dir1 * (-1.0 + 2.0 * (x0 + step)) + dir2 * (-1.0 + 2.0 * (y0 + step)));

                    normals.push(norm);
                    normals.push(norm);
                    normals.push(norm);
                    normals.push(norm);

                    if an == norm {
                        indices.push(base);
                        indices.push(base + 3);
                        indices.push(base + 1);
                        indices.push(base);
                        indices.push(base + 2);
                        indices.push(base + 3);
                    } else {
                        indices.push(base);
                        indices.push(base + 1);
                        indices.push(base + 3);
                        indices.push(base);
                        indices.push(base + 3);
                        indices.push(base + 2);
                    }
                }
            }
        }

        Cube {
            indices,
            vertices,
            normals
        }
    }

    pub fn indices(&self) -> super::IndexBuffer {
        super::IndexBuffer::from16(&self.indices)
    }

    pub fn vertices(&self) -> super::VertexBuffer {
        super::VertexBuffer::from(&self.vertices)
    }

    pub fn normals(&self) -> super::VertexBuffer {
        super::VertexBuffer::from(&self.normals)
    }
}