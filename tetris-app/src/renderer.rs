use imgui::*;
use cgmath::{Vector2, Vector3, Vector4};
use appbase::imgui_helper::staticwindow;
use rand::{Rng,SeedableRng};

use super::util;
use tetris::piece;
use tetris::state::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rectangle {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rectangle { x, y, w, h }
    }
    fn bottom(&self) -> f32 {
        self.y + self.h
    }
    fn expanded(&self, border: f32) -> Self {
        Rectangle { x: self.x - border, y: self.y - border, w: self.w + 2.0 * border, h: self.h + 2.0 * border}
    }
}

struct FallingPiece {
    piece: piece::Piece,
    size: f32,
    pos: (f32, f32),
    speed: f32,
}

pub struct Renderer {
    timestamp: usize,
    state: Option<Snapshot>,

    pos_field: Rectangle,
    pos_next: Rectangle,
    pos_info: Rectangle,
    pos_stats: Rectangle,
    z: f32,

    pub ghost_piece: bool,
    tile_size: f32,

    square: tinygl::VertexBuffer,
    cube_vertices: tinygl::VertexBuffer,
    cube_normals: tinygl::VertexBuffer,
    cube_indices: tinygl::IndexBuffer,
    program: tinygl::Program,
    block_program: tinygl::Program,

    piece_colors: Vec<[Vector3<f32>; 7]>,

    background: Vec<FallingPiece>,
    background_timer: f32,
}

struct BlockBuffers {
    data: [Vec<f32>; 7],
}

impl BlockBuffers {
    fn new() -> Self {
        BlockBuffers {
            data: [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()],
        }
    }

    fn vertex_size() -> i32 { 4 }

    fn block(&mut self, block: piece::Type, pos: (f32, f32), sz: f32, alpha: f32) {
        let idx = block as usize;
        self.data[idx].push(pos.0);
        self.data[idx].push(pos.1);
        self.data[idx].push(sz);
        self.data[idx].push(alpha);
    }

    fn piece(&mut self, piece: piece::Piece, x: f32, y: f32, sz: f32, ymax: f32, alpha: f32) {
        let blocks = piece.blocks();
        for i in 0..4 {
            for j in 0..4 {
                let y = y - j as f32 * sz;
                if y < ymax && blocks[4*j + i] {
                    self.block(piece.get_type(),
                               (x + i as f32 * sz + 1.0, y + 1.0),
                               sz - 2.0,
                                alpha
                    );
                }
            }
        }
    }

    fn vbo(&self, idx: usize) -> Option<(tinygl::VertexBuffer, usize)> {
        if self.data[idx].is_empty() {
            None
        } else {
            let count = self.data[idx].len() / Self::vertex_size() as usize;
            Some((tinygl::VertexBuffer::from(&self.data[idx]), count))
        }
    }
}

impl Renderer {
    fn draw_square(&self, pos: Rectangle, z: f32, color: Vector4<f32>) {
        self.program.uniform("pos", tinygl::Uniform::Vec3(Vector3::new(pos.x, pos.y, z)));
        self.program.uniform("size", tinygl::Uniform::Vec2(Vector2::new(pos.w, pos.h)));
        self.program.uniform("color", tinygl::Uniform::Vec4(color));
        unsafe { gl::DrawArrays(gl::TRIANGLES, 0, 6) }
    }

    fn draw_block(&self, buffers: &mut BlockBuffers, piece: piece::Type, x: i32, y: i32, alpha: f32) {
        buffers.block(piece,
                      (self.pos_field.x + self.tile_size * x as f32 + 1.0, self.pos_field.bottom() - self.tile_size * (y + 1) as f32 + 1.0),
                      self.tile_size - 2.0, alpha
        );
    }

    fn gen_level_colors() -> Vec<[Vector3<f32>; 7]> {
        let rng = std::cell::RefCell::new(rand::rngs::SmallRng::from_seed([0,1,2,3,4,5,6,7,8,9,9,8,7,6,5,4]));
        let rng = std::cell::RefCell::new(rand::rngs::OsRng::new().unwrap());
        let rnd = |min: f32, max: f32| { min + (max-min) * rng.borrow_mut().gen::<f32>() };

        // level 0 base colors
        let base = [
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
            Vector3::new(1.0, 0.0, 1.0),
            Vector3::new(1.0, 0.5, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
        ];

        let mut piece_colors = Vec::new();

        for lvl in 0..100 {
            let col = util::hsv(rnd(0.0, 360.0), rnd(0.4, 0.8), 1.0);
            let ratio = (1.0 - 0.1 * lvl as f32).max(0.17);
            let fac = 0.7;
            piece_colors.push([
                fac * (ratio * base[0] + (1.0 - ratio) * col),
                fac * (ratio * base[1] + (1.0 - ratio) * col),
                fac * (ratio * base[2] + (1.0 - ratio) * col),
                fac * (ratio * base[3] + (1.0 - ratio) * col),
                fac * (ratio * base[4] + (1.0 - ratio) * col),
                fac * (ratio * base[5] + (1.0 - ratio) * col),
                fac * (ratio * base[6] + (1.0 - ratio) * col),
            ]);
        };

        piece_colors
    }

    pub fn new(pos_field: Rectangle, pos_next: Rectangle, pos_info: Rectangle, pos_stats: Rectangle) -> Self {
        let cube = tinygl::shapes::Cube::new(1);

        Renderer {
            timestamp: 0,
            state: None,

            pos_field,
            pos_next,
            pos_info,
            pos_stats,

            ghost_piece: false,
            tile_size: pos_field.w / 10.0,
            z: 10000.0,

            background: Vec::new(),
            background_timer: 0.0,

            program: tinygl::Program::new("
                in vec2 vertex;
                uniform vec3 pos;
                uniform vec2 size;
                uniform mat4 mvp;
                void main() {
                    gl_Position = mvp * vec4(pos.xy + vertex * size, -pos.z, 1.0);
                }
                ", "
                uniform vec4 color;
                out vec4 outColor;
                void main() {
                    outColor = color;
                }
                "),

            block_program: tinygl::Program::new("
                in vec3 vertex;
                in vec2 position;
                in float size;
                in float alpha;
                uniform mat4 mvp;
                uniform float z;
                out float v_alpha;
                void main() {
                    v_alpha = alpha;
                    vec3 pos = vec3(position.xy, 0.0) + 0.5 * size * (vertex + vec3(1.0));
                    pos += vec3(0.0, 0.0, -z);
                    gl_Position = mvp * vec4(pos, 1.0);
                }
                ", "
                uniform vec3 color;
                in float v_alpha;
                out vec4 outColor;
                void main() {
                    outColor = vec4(color, v_alpha);
                }
                "),

            square: tinygl::VertexBuffer::from::<f32>(&vec!(0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0)),

            cube_vertices: cube.vertices(),
            cube_normals: cube.normals(),
            cube_indices: cube.indices(),

            piece_colors: Self::gen_level_colors(),
        }
    }

    pub fn set_state(&mut self, timestamp: usize, state: &Snapshot) {
        self.timestamp = timestamp;
        self.state = Some(state.clone());
    }

    pub fn clear(&mut self) {
        self.state = None;
    }

    pub fn gen_new_colors(&mut self) {
        self.piece_colors = Self::gen_level_colors();
    }

    fn collect_blocks(&self) -> BlockBuffers {
        let state = self.state.as_ref().unwrap();
        let stack = state.stack().blocks();

        let mut buffers = BlockBuffers::new();

        // compute line burn
        let t = state.are_duration()
            .map(|d| (self.timestamp - state.timestamp()) as f32 / d as f32)
            .unwrap_or(1.0);
        let burn = state.animation().map(|a| a.0);

        // add stack
        stack.for_each(&mut |x, y, piece| {
            if *piece != piece::Type::None {
                let render_block = {
                    if let Some(burn) = &burn {
                        !burn.contains(&y) || t < 2.0 * (0.5 - x as f32 / (stack.width() - 1) as f32).abs()
                    } else {
                        true
                    }
                };
                if render_block {
                    self.draw_block(&mut buffers, *piece, x, y, 1.0);
                }
            }
        });

        // add curr piece - only if no burn animation is running!
        if let Some(piece) = state.piece() {
            buffers.piece(piece.0,
                          self.pos_field.x + self.tile_size * piece.1 as f32,
                          self.pos_field.bottom() - self.tile_size * (piece.2 + 1) as f32,
                          self.tile_size, self.pos_field.bottom(), 1.0);
        }

        // add ghost piece
        if self.ghost_piece {
            if let Some(piece) = state.ghost_piece() {
                buffers.piece(piece.0,
                              self.pos_field.x + self.tile_size * piece.1 as f32,
                              self.pos_field.bottom() - self.tile_size * (piece.2 + 1) as f32,
                              self.tile_size, self.pos_field.bottom(), 0.4);
            }
        }

        // add next piece
        let ofs = state.next_piece().get_type().offset();
        buffers.piece(state.next_piece(),
                      self.pos_next.x + ofs.0 * self.tile_size,
                      self.pos_next.bottom() + (ofs.1 - 1.0) * self.tile_size,
                      self.tile_size, 0.0, 1.0);

        // add stats pieces
        for i in 0..7 {
            let y = self.pos_stats.y + 80.0 * i as f32;
            let pc = piece::Piece::new(piece::Type::from_int(i), 2);
            let ofs = pc.get_type().offset();
            buffers.piece(pc,
                          self.pos_stats.x + 40.0 + ofs.0 * 20.0,
                          y + (ofs.1 + 3.0) * 20.0,
                          20.0, 10000.0, 1.0);
        }

        buffers
    }

    fn render_blocks(&self, mvp: &cgmath::Matrix4<f32>, buffers: BlockBuffers, palette: usize) {
        self.cube_indices.bind();
        self.block_program.bind();
        self.block_program.uniform("mvp", tinygl::Uniform::Mat4(*mvp));
        self.block_program.uniform("z", tinygl::Uniform::Float(self.z));
        self.block_program.vertex_attrib_buffer("vertex", &self.cube_vertices, 3, gl::FLOAT, false, 12, 0);
        self.block_program.vertex_attrib_divisor("vertex", 0);
        self.block_program.vertex_attrib_divisor("position", 1);
        self.block_program.vertex_attrib_divisor("size", 1);
        self.block_program.vertex_attrib_divisor("alpha", 1);

        unsafe {
            gl::Enable(gl::BLEND);
            gl::Disable(gl::CULL_FACE);
        }

        let palette = palette.min(self.piece_colors.len() - 1);
        let colors = &self.piece_colors[palette];

        for i in 0..7 {
            if let Some(vbo) = buffers.vbo(i) {
                self.block_program.uniform("color", tinygl::Uniform::Vec3(colors[i]));
                self.block_program.vertex_attrib_buffer("position", &vbo.0, 2, gl::FLOAT, false, 4 * BlockBuffers::vertex_size(), 0);
                self.block_program.vertex_attrib_buffer("size", &vbo.0, 1, gl::FLOAT, false, 4 * BlockBuffers::vertex_size(), 8);
                self.block_program.vertex_attrib_buffer("alpha", &vbo.0, 1, gl::FLOAT, false, 4 * BlockBuffers::vertex_size(), 12);
                unsafe { gl::DrawElementsInstanced(gl::TRIANGLES, self.cube_indices.count() as i32, gl::UNSIGNED_SHORT, std::ptr::null(), vbo.1 as i32) }
            }
        }

        self.block_program.vertex_attrib_divisor("position", 0);
        self.block_program.vertex_attrib_divisor("size", 0);
        self.block_program.vertex_attrib_divisor("alpha", 0);

        self.block_program.disable_vertex_attrib("position");
        self.block_program.disable_vertex_attrib("size");
        self.block_program.disable_vertex_attrib("vertex");
        self.block_program.disable_vertex_attrib("alpha");
    }

    pub fn render(&mut self, mvp: &cgmath::Matrix4<f32>) {
        if self.state.is_none() {
            return;
        }

        unsafe { gl::Enable(gl::DEPTH_TEST) }
        unsafe { gl::Enable(gl::BLEND) }

        // draw squares for stack / next piece
        self.program.bind();
        self.program.uniform("mvp", tinygl::Uniform::Mat4(*mvp));
        self.program.vertex_attrib_divisor("vertex", 0);
        self.program.vertex_attrib_buffer("vertex", &self.square, 2, gl::FLOAT, false, 8, 0);
        unsafe { gl::DepthFunc(gl::ALWAYS) }
        self.draw_square(self.pos_field.expanded(50.0), self.z - 100.0, Vector4::new(0.2, 0.0, 0.0, 0.0));
        self.draw_square(self.pos_next, self.z, Vector4::new(0.0, 0.0, 0.0, 1.0));
        self.draw_square(self.pos_field, self.z, Vector4::new(0.0, 0.0, 0.0, 1.0));

        let buffers = self.collect_blocks();
        unsafe { gl::DepthFunc(gl::LEQUAL) }
        self.render_blocks(mvp, buffers, self.state.as_ref().unwrap().level() as usize);
    }

    pub fn render_background(&mut self, dt: f32, mvp: &cgmath::Matrix4<f32>) {
        let mut buffers = BlockBuffers::new();

        // add new block every N secs
        self.background_timer += dt;
        if self.background_timer > 0.2 && self.background.len() < 150 {
            self.background_timer -= 0.2;
            let size = 10.0 + 30.0 * rand::random::<f32>();
            let x = -600.0 + 1200.0 * rand::random::<f32>();

            self.background.push(FallingPiece {
                piece: piece::Piece::new(piece::Type::from_int(rand::random::<u32>() % 7), rand::random::<u8>() % 4),
                size,
                pos: (x, -400.0),
                speed: 40.0 + 200.0 * rand::random::<f32>(),
            });
        }

        // purge blocks that are below the horizon
        self.background.retain(|block| block.pos.1 < 500.0);

        // advance and render blocks
        for block in &mut self.background {
            block.pos.1 += block.speed * dt;
            buffers.piece(block.piece, block.pos.0, block.pos.1, block.size, 9999.0, 1.0);
        }

        let unixtime = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let palette = (unixtime / 20) % 30;
        self.render_blocks(mvp, buffers, palette as usize);
    }

    pub fn do_ui(&mut self, ui: &imgui::Ui, offset: (f32, f32), scale: f32) {
        if self.state.is_none() {
            return;
        }

        let state = self.state.as_ref().unwrap();

        staticwindow(ui, im_str!("scores"),
                     (offset.0 + self.pos_info.x * scale, offset.1 + self.pos_info.y * scale),
                     (self.pos_info.w * scale, self.pos_info.h * scale),
                     1.5 * scale, (0.0, 0.0, 0.0, 0.0), || {
                ui.text(format!(" Level: {}", state.level()));
                ui.text(format!(" Score: {}", state.score()));
                ui.text(format!(" Lines: {}", state.lines()));
                ui.text(format!("Tetris: {}%", (100.0 * state.tetris_rate()) as i32));
            });

        for i in 0..7 {
            let stats = state.stats().get(piece::Type::from_int(i));
            let y = self.pos_stats.y + 80.0 * i as f32 + 20.0;

            // choose color
            let step = 10;
            let col = if stats.1 < step {
                imgui::ImVec4::new(1.0, 1.0, 1.0, stats.1 as f32 / step as f32)
            } else if stats.1 < 2*step {
                imgui::ImVec4::new(1.0, 1.0, 2.0 - stats.1 as f32 / step as f32, 1.0)
            } else if stats.1 < 3*step {
                imgui::ImVec4::new(1.0, 3.0 - stats.1 as f32 / step as f32, 0.0, 1.0)
            } else {
                imgui::ImVec4::new(1.0, 0.0, 0.0, 1.0)
            };

            staticwindow(ui, im_str!("droughtstats#window{}", i),
                         (offset.0 + self.pos_stats.x * scale, offset.1 + y * scale),
                         (40.0 * scale, 60.0 * scale),
                         1.5 * scale, (0.0, 0.0, 0.0, 0.0), || {
                    ui.text_colored(col, im_str!("{}", stats.1));
                });

            staticwindow(ui, im_str!("countstats#window{}", i),
                         (offset.0 + (self.pos_stats.x + 140.0) * scale, offset.1 + y * scale),
                         (40.0 * scale, 60.0 * scale),
                         1.5 * scale, (0.0, 0.0, 0.0, 0.0), || {
                    ui.text(format!("{}", stats.0));
                });
        }
    }
}
