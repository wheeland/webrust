use imgui::*;
// use cgmath::prelude::*;
use cgmath::{Vector2, Vector3, Vector4, Matrix3, InnerSpace};
use appbase::imgui_helper::staticwindow;
use rand::{Rng,SeedableRng};

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
    fn scaled(&self, rel: f32) -> Self {
        Rectangle { x: self.x * rel, y: self.y * rel, w: self.w * rel, h: self.h * rel }
    }
}

struct FallingPiece {
    piece: piece::Piece,
    size: f32,
    pos: (f32, f32),
    depth: f32,
    speed: f32,
}

pub struct Renderer {
    timestamp: i32,
    state: Option<Snapshot>,

    pos_field: Rectangle,
    pos_next: Rectangle,
    pos_info: Rectangle,
    pos_stats: Rectangle,
    z: f32,

    pub ghost_piece: bool,
    pub threed: bool,
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

    fn vertex_size() -> i32 { 5 }

    fn block(&mut self, block: piece::Type, pos: (f32, f32, f32), sz: f32, alpha: f32) {
        let idx = block as usize;
        self.data[idx].push(pos.0);
        self.data[idx].push(pos.1);
        self.data[idx].push(-pos.2);
        self.data[idx].push(sz);
        self.data[idx].push(alpha);
    }

    fn piece(&mut self, piece: piece::Piece, x: f32, y: f32, z: f32, sz: f32, ymax: f32, alpha: f32) {
        let blocks = piece.blocks();
        for i in 0..4 {
            for j in 0..4 {
                let y = y - j as f32 * sz;
                if y < ymax && blocks[4*j + i] {
                    self.block(piece.get_type(), (x + i as f32 * sz, y, z), sz, alpha);
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

    fn draw_block(&self, buffers: &mut BlockBuffers, piece: piece::Type, x: i32, y: i32, z: f32, alpha: f32) {
        buffers.block(piece,
                      (self.pos_field.x + self.tile_size * x as f32,
                      self.pos_field.bottom() - self.tile_size * (y + 1) as f32, z),
                      self.tile_size, alpha
        );
    }

    fn gen_level_base_color<F>(rnd: &F) -> Vector3<f32> where F: Fn(f32, f32) -> f32 {
        let hue = rnd(0.0, 360.0);
        let mut saturation = rnd(0.4, 0.9);
        let mut col = util3d::hsv(hue, saturation, 1.0);
        // if it's too dark, let's adjust it a little bit
        while col.dot(Vector3::new(0.3, 0.59, 0.11)) < 0.55 {
            saturation -= 0.05;
            col = util3d::hsv(hue, saturation, 1.0);
        }
        return col;
    }

    fn gen_level_colors() -> Vec<[Vector3<f32>; 7]> {
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
            let col = Self::gen_level_base_color(&rnd);
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

    pub fn new(pos_field: Rectangle, pos_next: Rectangle, pos_info: Rectangle, pos_stats: Rectangle, z: f32) -> Self {
        let cube = tinygl::shapes::Cube::new(1);

        Renderer {
            timestamp: 0,
            state: None,

            pos_field,
            pos_next,
            pos_info,
            pos_stats,

            ghost_piece: false,
            threed: false,
            tile_size: pos_field.w / 10.0,
            z,

            background: Vec::new(),
            background_timer: 0.0,

            program: tinygl::Program::new_versioned("
                attribute vec2 vertex;
                uniform vec3 pos;
                uniform vec2 size;
                uniform mat4 view;
                void main() {
                    gl_Position = view * vec4(pos.xy + vertex * size, -pos.z, 1.0);
                }
                ", "
                uniform vec4 color;
                void main() {
                    gl_FragColor = color;
                }
                ", 100),

            block_program: tinygl::Program::new_versioned("
                attribute vec3 vertex;
                attribute vec3 normal;
                attribute vec3 position;
                attribute float size;
                attribute float alpha;
                uniform mat4 model;
                uniform mat4 view;

                varying float v_alpha;
                varying vec3 v_normal;
                varying vec3 v_position;

                void main() {
                    v_alpha = alpha;
                    vec3 v = (model * vec4(vertex, 1.0)).xyz;
                    vec3 pos = position + 0.5 * size * (v + vec3(1.0, 1.0, -1.0));
                    v_normal = normal;
                    v_position = pos;
                    gl_Position = view * vec4(pos, 1.0);
                }
                ", "
                uniform vec3 color;

                varying float v_alpha;
                varying vec3 v_normal;
                varying vec3 v_position;

                void main() {
                    float light = max(dot(-v_normal, normalize(v_position)), 0.1);
                    gl_FragColor = vec4(light * color, v_alpha);
                }
                ", 100),

            square: tinygl::VertexBuffer::from::<f32>(&vec!(0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0)),

            cube_vertices: cube.vertices(),
            cube_normals: cube.normals(),
            cube_indices: cube.indices(),

            piece_colors: Self::gen_level_colors(),
        }
    }

    pub fn set_state(&mut self, timestamp: i32, state: &Snapshot) {
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
                    self.draw_block(&mut buffers, *piece, x, y, self.z, 1.0);
                }
            }
        });

        // add curr piece - only if no burn animation is running!
        if let Some(piece) = state.piece() {
            buffers.piece(piece.0,
                          self.pos_field.x + self.tile_size * piece.1 as f32,
                          self.pos_field.bottom() - self.tile_size * (piece.2 + 1) as f32, self.z,
                          self.tile_size, self.pos_field.bottom(), 1.0);
        }

        // add ghost piece
        if self.ghost_piece {
            if let Some(piece) = state.ghost_piece() {
                buffers.piece(piece.0,
                              self.pos_field.x + self.tile_size * piece.1 as f32,
                              self.pos_field.bottom() - self.tile_size * (piece.2 + 1) as f32, self.z,
                              self.tile_size, self.pos_field.bottom(), 0.4);
            }
        }

        // add next piece
        let ofs = state.next_piece().get_type().offset();
        buffers.piece(state.next_piece(),
                      self.pos_next.x + ofs.0 * self.tile_size,
                      self.pos_next.bottom() + (ofs.1 - 1.0) * self.tile_size, self.z,
                      self.tile_size, 0.0, 1.0);

        // add stats pieces
        for i in 0..7 {
            let y = self.pos_stats.y + 80.0 * i as f32;
            let pc = piece::Piece::new(piece::Type::from_int(i), 2);
            let ofs = pc.get_type().offset();
            buffers.piece(pc,
                          self.pos_stats.x + 40.0 + ofs.0 * 20.0,
                          y + (ofs.1 + 3.0) * 20.0, self.z,
                          20.0, 10000.0, 1.0);
        }

        buffers
    }

    fn render_blocks(&self, view: &cgmath::Matrix4<f32>, buffers: BlockBuffers, palette: usize) {
        let threed = if self.threed { 1.0 } else { 0.0 };
        let scale = if self.threed { 0.9 } else { 0.95 };
        let model = cgmath::Matrix4::from_nonuniform_scale(scale, scale, threed * scale);
        let model = cgmath::Matrix4::from_translation(cgmath::Vector3::new(0.0, 0.0, (1.0 - threed) * scale)) * model;

        unsafe {
            gl::Enable(gl::BLEND);
            gl::Enable(gl::CULL_FACE);
            gl::Enable(gl::DEPTH_TEST);
            gl::Disable(gl::STENCIL_TEST);
        }

        self.cube_indices.bind();
        self.block_program.bind();
        self.block_program.uniform("model", tinygl::Uniform::Mat4(model));
        self.block_program.uniform("view", tinygl::Uniform::Mat4(*view));
        self.block_program.vertex_attrib_buffer("vertex", &self.cube_vertices, 3, gl::FLOAT, false, 12, 0);
        self.block_program.vertex_attrib_divisor("vertex", 0);
        self.block_program.vertex_attrib_buffer("normal", &self.cube_normals, 3, gl::FLOAT, false, 12, 0);
        self.block_program.vertex_attrib_divisor("normal", 0);
        self.block_program.vertex_attrib_divisor("position", 1);
        self.block_program.vertex_attrib_divisor("size", 1);
        self.block_program.vertex_attrib_divisor("alpha", 1);

        let palette = palette.min(self.piece_colors.len() - 1);
        let colors = &self.piece_colors[palette];

        for i in 0..7 {
            if let Some(vbo) = buffers.vbo(i) {
                self.block_program.uniform("color", tinygl::Uniform::Vec3(colors[i]));
                self.block_program.vertex_attrib_buffer("position", &vbo.0, 3, gl::FLOAT, false, 4 * BlockBuffers::vertex_size(), 0);
                self.block_program.vertex_attrib_buffer("size", &vbo.0, 1, gl::FLOAT, false, 4 * BlockBuffers::vertex_size(), 12);
                self.block_program.vertex_attrib_buffer("alpha", &vbo.0, 1, gl::FLOAT, false, 4 * BlockBuffers::vertex_size(), 16);
                unsafe { gl::DrawElementsInstanced(gl::TRIANGLES, self.cube_indices.count() as i32, gl::UNSIGNED_SHORT, std::ptr::null(), vbo.1 as i32) }
            }
        }

        self.block_program.vertex_attrib_divisor("position", 0);
        self.block_program.vertex_attrib_divisor("size", 0);
        self.block_program.vertex_attrib_divisor("alpha", 0);

        self.block_program.disable_all_vertex_attribs();
    }

    pub fn render(&mut self, view: &cgmath::Matrix4<f32>) {
        if self.state.is_none() {
            return;
        }

        unsafe { gl::Enable(gl::DEPTH_TEST) }
        unsafe { gl::Enable(gl::BLEND) }

        // draw squares for stack / next piece
        self.program.bind();
        self.program.uniform("view", tinygl::Uniform::Mat4(*view));
        self.program.vertex_attrib_divisor("vertex", 0);
        self.program.vertex_attrib_buffer("vertex", &self.square, 2, gl::FLOAT, false, 8, 0);
        unsafe { gl::DepthFunc(gl::ALWAYS) }
        self.draw_square(self.pos_field.expanded(50.0), self.z - 100.0, Vector4::new(0.2, 0.0, 0.0, 0.0));
        self.draw_square(self.pos_next.scaled(1.3), 1.3 * self.z, Vector4::new(0.0, 0.0, 0.0, 1.0));
        self.draw_square(self.pos_field.scaled(1.3), 1.3 * self.z, Vector4::new(0.0, 0.0, 0.0, 1.0));
        self.program.disable_all_vertex_attribs();

        let buffers = self.collect_blocks();
        unsafe { gl::DepthFunc(gl::LEQUAL) }
        self.render_blocks(view, buffers, self.state.as_ref().unwrap().level() as usize);
    }

    pub fn render_background(&mut self, dt: f32, view: &cgmath::Matrix4<f32>) {
        let mut buffers = BlockBuffers::new();

        // add new block every N secs
        self.background_timer += dt;
        if self.background_timer > 0.2 && self.background.len() < 150 {
            self.background_timer -= 0.2;
            let size = 10.0 + 30.0 * rand::random::<f32>();
            let x = -600.0 + 1200.0 * rand::random::<f32>();
            let depth = 0.5 + 1.0 * rand::random::<f32>();

            self.background.push(FallingPiece {
                piece: piece::Piece::new(piece::Type::from_int(rand::random::<u32>() % 7), rand::random::<u8>() % 4),
                size,
                pos: (x, -500.0 * depth),
                depth,
                speed: 40.0 + 200.0 * rand::random::<f32>(),
            });
        }

        // purge blocks that are below the horizon
        self.background.retain(|block| block.pos.1 < 600.0 * (block.depth + 0.3));

        // advance and render blocks
        for block in &mut self.background {
            block.pos.1 += block.speed * dt;
            buffers.piece(block.piece, block.pos.0, block.pos.1, block.depth * self.z, block.size, 9999.0, 1.0);
        }

        let unixtime = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let palette = (unixtime / 20) % 30;

        unsafe { gl::Enable(gl::DEPTH_TEST) }
        unsafe { gl::Enable(gl::BLEND) }
        unsafe { gl::DepthFunc(gl::LEQUAL) }
        self.render_blocks(view, buffers, palette as usize);
    }

    pub fn do_ui(&mut self, ui: &imgui::Ui, offset: (f32, f32), scale: f32) {
        if self.state.is_none() {
            return;
        }

        let state = self.state.as_ref().unwrap();

        staticwindow(ui, "scores",
                     (offset.0 + self.pos_info.x * scale, offset.1 + self.pos_info.y * scale),
                     (self.pos_info.w * scale, self.pos_info.h * scale),
                     (0.0, 0.0, 0.0, 0.0), || {
                ui.set_window_font_scale(1.5 * scale);
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
                [1.0, 1.0, 1.0, stats.1 as f32 / step as f32]
            } else if stats.1 < 2*step {
                [1.0, 1.0, 2.0 - stats.1 as f32 / step as f32, 1.0]
            } else if stats.1 < 3*step {
                [1.0, 3.0 - stats.1 as f32 / step as f32, 0.0, 1.0]
            } else {
                [1.0, 0.0, 0.0, 1.0]
            };

            staticwindow(ui, &format!("droughtstats#window{}", i),
                         (offset.0 + self.pos_stats.x * scale, offset.1 + y * scale),
                         (40.0 * scale, 60.0 * scale),
                         (0.0, 0.0, 0.0, 0.0), || {
                    ui.set_window_font_scale(1.5 * scale);
                    ui.text_colored(col, format!("{}", stats.1));
                });

            staticwindow(ui, &format!("countstats#window{}", i),
                         (offset.0 + (self.pos_stats.x + 140.0) * scale, offset.1 + y * scale),
                         (40.0 * scale, 60.0 * scale),
                         (0.0, 0.0, 0.0, 0.0), || {
                    ui.set_window_font_scale(1.5 * scale);
                    ui.text(format!("{}", stats.0));
                });
        }
    }
}
