use tinygl::*;
use array2d::Array2D;

use super::plate;
use super::plateoptimizer;

use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use lru_cache::LruCache;
use cgmath::*;

pub type Idx = u16;

static mut MEM_USAGE: i32 = 0;

fn add_mem_usage(delta: usize) { unsafe { MEM_USAGE += delta as i32; } }
fn del_mem_usage(delta: usize) { unsafe { MEM_USAGE -= delta as i32; } }

//
// Result of a triangle-optimization stage
//
pub struct Triangulation {
    detail: u8,
    pub vertices: Vec<Vector4<f32>>,
    pub triangles: Vec<Idx>,
    pub wireframe: Vec<Idx>,
}

impl Triangulation {
    fn new(detail: u8, vertices: Vec<Vector4<f32>>, triangles: Vec<Idx>, wireframe: Vec<Idx>,) -> Self {
        add_mem_usage(vertices.capacity() * 16 + (triangles.capacity() + wireframe.capacity()) * std::mem::size_of::<Idx>());
        Triangulation {
            detail,
            vertices,
            triangles,
            wireframe,
        }
    }
}
impl Drop for Triangulation {
    fn drop(&mut self) {
        del_mem_usage(self.vertices.capacity() * 16 + (self.triangles.capacity() + self.wireframe.capacity()) * std::mem::size_of::<Idx>());
    }
}

//
// Data generated on the GPU for one plate
//
pub struct Result {
    pub height_extent: (f32, f32),
    pub heights: Vec<f32>,
    pub vertex_data: Vec<Vector4<f32>>,
    pub merged_data: Vec<Vector4<f32>>,
    pub normals: Vec<u8>,
    pub channels: HashMap<String, Texture>,
    pub triangulation: Option<Triangulation>
}

impl Result {
    fn new(height_extent: (f32, f32),
           heights: Vec<f32>,
           vertex_data: Vec<Vector4<f32>>,
           merged_data: Vec<Vector4<f32>>,
           normals: Vec<u8>,
           triangulation: Option<Triangulation>,
           channels: HashMap<String, Texture>
    ) -> Self {
        add_mem_usage(heights.capacity() * 4 + (vertex_data.capacity() + merged_data.capacity()) * 16 + normals.capacity());
        Result {
            height_extent,
            heights,
            vertex_data,
            merged_data,
            normals,
            channels,
            triangulation
        }
    }
}

impl Drop for Result {
    fn drop(&mut self) {
        del_mem_usage(self.heights.capacity() * 4 + (self.vertex_data.capacity() + self.merged_data.capacity()) * 16 + self.normals.capacity());
    }
}

//
// Central instance for requesting and returning tile data
//
pub struct PlateDataManager {
    size: i32,
    radius: f32,
    generator: Generator,
    cache: LruCache<plate::Position, Result>,
    waiting: HashMap<plate::Position, f32>,
}
pub type PlateDataManagerPtr = Rc<RefCell<PlateDataManager>>;

impl PlateDataManager {
    pub fn new(pow2size: i32, radius: f32, vertex_generator: Program, post_generator: Program, channels: &super::Channels) -> Self {
        PlateDataManager {
            size: 2i32.pow(pow2size as _),
            radius,
            generator: Generator::new(pow2size, 100, radius, 3, vertex_generator, post_generator, channels),
            cache: LruCache::new(400),
            waiting: HashMap::new()
        }
    }

    pub fn request(&mut self, position: &plate::Position, priority: f32) -> Option<Result> {
        let cached = self.cache.remove(position);
        match cached {
            Some(rd) => Some(rd),
            None => {
                self.waiting.insert(*position, priority);
                None
            }
        }
    }

    pub fn abort(&mut self, position: &plate::Position) {
        self.waiting.remove(&position);
    }

    pub fn insert(&mut self, position: &plate::Position, data: Result) {
        self.cache.insert(*position, data);
        self.waiting.remove(&position);
    }

    pub fn start_data_generation(&mut self, max: usize) {
        let mut entries: Vec<(&plate::Position, &f32)> = self.waiting.iter().collect();
        entries.sort_by(|a,b| b.1.partial_cmp(a.1).unwrap());

        for i in 0..entries.len().min(max) {
            self.generator.generate(*entries[i].0);
        }
    }

    pub fn collect_render_data(&mut self) {
        for (tile, result) in self.generator.results() {
            self.insert(&tile, result);
        }
    }

    pub fn set_detail(&mut self, detail: u8) {
        self.generator.detail = detail;
        self.cache.clear();
    }

    pub fn retriangulate(&self, data: &mut Result) {
        data.triangulation = Some(self.generator.triangulate(&data));
    }

    pub fn generate_plate_coords(&self) -> Vec<u16> {
        self.generator.generate_plate_coords()
    }

    pub fn generate_indices(&self) -> (Vec<Idx>, Vec<Idx>) {
        self.generator.generate_indices()
    }

    pub fn size(&self) -> i32 {
        self.size
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    pub fn memory_usage(&self) -> usize {
        unsafe { MEM_USAGE as usize }
    }
}

//
// Offscreen FBOs used during the plate data generation and optimization phases
//
struct GeneratorBuffers {
    position_pass: OffscreenBuffer,
    normal_pass: OffscreenBuffer,
}

impl GeneratorBuffers {
    fn new(size: i32, channels: &super::Channels) -> Self {
        let mut position_pass = OffscreenBuffer::new((size, size));
        position_pass.add("position", gl::RGBA32F, gl::RGBA, gl::FLOAT);
        for chan in channels.channels() {
            let int_fmt = match chan.1 {
                1 => (gl::R8, gl::RED),
                2 => (gl::RG8, gl::RG),
                3 => (gl::RGB8, gl::RGB),
                4 => (gl::RGBA8, gl::RGBA),
                _ => { panic!("Does not compute"); },
            };
            position_pass.add(&chan.0, int_fmt.0, int_fmt.1, gl::UNSIGNED_BYTE);
        }

        let mut normal_pass = OffscreenBuffer::new((size, size));
        normal_pass.add("normal", gl::RGBA, gl::RGBA, gl::UNSIGNED_BYTE);
        normal_pass.add("merged", gl::RGBA32F, gl::RGBA, gl::FLOAT);

        GeneratorBuffers {
            position_pass,
            normal_pass,
        }
    }
}

//
// Shaders for Vertex + Height (+ Channels) generation
//
pub fn compile_generator(generator: &str, channels: &super::Channels) -> Program {
    let declarations = channels.declarations()
        .iter()
        .enumerate()
        .map(|chan| {
            format!("layout(location = {}) out {};\n", chan.0 + 1, chan.1)
        })
        .fold(String::new(), |acc, x| acc + &x);

    let vert = "
        in vec2 xy;
        void main()
        {
            gl_Position = vec4(xy, 0.0, 1.0);
        }";

    let frag = super::noise::ShaderNoise::declarations() + "
        uniform vec2 ofs;
        uniform float invsize;
        uniform float stretch;
        uniform float stretchAsin;
        uniform float mul;
        uniform float radius;
        uniform int depth;
        uniform mat3 cubeTransformMatrix;

        float height;

        layout(location = 0) out vec4 posHeight;
        "
        + &declarations + "
        \n#line 1\n"
        + generator + "

        void main()
        {
            vec2 rel = (gl_FragCoord.xy - vec2(1.5)) * vec2(invsize, invsize);
            vec2 rawXy = asin(stretch * (ofs + vec2(mul) * rel)) / stretchAsin;

            vec2 xy = clamp(vec2(-1.0), rawXy, vec2(1.0));
            vec2 diff = abs(rawXy - xy);
            float dz = max(diff.x, diff.y);

            vec3 cubePos = cubeTransformMatrix * vec3(xy, 1.0 - dz);
            vec3 position = normalize(cubePos);

            height = 0.0;
            generate(position * radius, depth);

            posHeight = vec4(position, height);
        }" + &super::noise::ShaderNoise::definitions();

    Program::new(vert, &frag)
}

//
// Shader for Normals, Interpolation, Vertex Merging (and Channels)
//
pub fn compile_postvertex(channels: &super::Channels) -> Program {
    let declarations = channels.declarations()
        .iter()
        .enumerate()
        .map(|chan| {
            format!("layout(location = {}) out {};\n", chan.0 + 2, chan.1)
        })
        .fold(String::new(), |acc, x| acc + &x);

    let vert = "
            in vec2 xy;
            void main()
            {
                gl_Position = vec4(xy, 0.0, 1.0);
            }";

    let frag = super::noise::ShaderNoise::declarations() + "
            uniform float size;
            uniform float radius;
            uniform sampler2D positions;
            uniform sampler2D parentCoords;

            layout(location = 0) out vec4 normal;
            layout(location = 1) out vec4 merged;
            "
        + &declarations +
        "
#line 1
"
        //+ channels.source() +
        + "
            vec3 _pos(vec2 tc) {
                vec4 heightPos = texture(positions, tc / (size + 3.0));
                return heightPos.xyz * (radius + heightPos.w);
            }

            void main()
            {
                vec4 heightPosCenter = texture(positions, gl_FragCoord.xy / (size + 3.0));
                vec3 pCenter = heightPosCenter.xyz * (radius + heightPosCenter.w);
                vec3 xp = _pos(gl_FragCoord.xy + vec2(1.0,  0.0));
                vec3 xn = _pos(gl_FragCoord.xy + vec2(-1.0, 0.0));
                vec3 yp = _pos(gl_FragCoord.xy + vec2(0.0,  1.0));
                vec3 yn = _pos(gl_FragCoord.xy + vec2(0.0, -1.0));

                vec3 norm = normalize(cross(xp - xn, yp - yn));
                if (dot(norm, xp) < 0.0)
                    norm = -norm;

                // get position of parent vertices within this tile (range: [0..1])
                vec4 parents = texture(parentCoords, (gl_FragCoord.xy - vec2(1.0)) / (size + 1.0));
                float parentDist = length(parents.xy - parents.zw);

                // read parent world positions
                vec3 pparent1 = _pos(vec2(1.5) + parents.xy * size);
                vec3 pparent2 = _pos(vec2(1.5) + parents.zw * size);

                // calculate interpolated position
                vec3 mid = 0.5 * (pparent1 + pparent2);
                float midLen = length(mid);
                float midHeight = midLen - radius;

                // calculate relative difference to this position
                float dParents = length(pparent1 - pparent2);
                float dMid = length(mid - pCenter);
                float interpolation = (pparent1 == pparent2) ? 0.0 : dMid / dParents;

                normal = vec4(vec3(0.5) + 0.5 * norm, 5.0 * interpolation * sqrt(parentDist * size));
            " +
        //+ if channels.source().is_empty() { "" } else { "generate(heightPosCenter.xyz, heightPosCenter.w, normal.xyz);" } +
        "
                merged = vec4(mid / midLen, midHeight);
            }
            " + &super::noise::ShaderNoise::definitions();

    Program::new(vert, &frag)
}

//
// Responsible for actually generating the tile data using specialized shaders
//
struct Generator {
    depth: usize,
    size: usize,
    pub detail: u8,
    channels: super::Channels,

    quad: VertexBuffer,
    vertex_generator: Program,
    post_generator: Program,
    optimizer: plateoptimizer::PlateOptimizer,

    offset_texture: Texture,    // parent vertices (the two that the actual vertex is between)
    framebuffer_cache: Vec<GeneratorBuffers>,
    max_framebuffer_cache: usize,

    generation_order: Vec<plate::Position>,
    framebuffers: HashMap<plate::Position, GeneratorBuffers>
}

impl Generator {
    // Generate map with parent/depth info for every vertex
    fn fill_offsets(depth: u8, maxdepth: u8, x1: Idx, y1: Idx, x2: Idx, y2: Idx,
                    vertex_parents: &mut Array2D<((Idx,Idx),(Idx,Idx))>) {
        let xm = ((x1 as i32 + x2 as i32) / 2) as Idx;
        let ym = ((y1 as i32 + y2 as i32) / 2) as Idx;
        if depth < maxdepth {
            Self::fill_offsets(depth + 1, maxdepth, x1, y1, xm, ym, vertex_parents);
            Self::fill_offsets(depth + 1, maxdepth, xm, y1, x2, ym, vertex_parents);
            Self::fill_offsets(depth + 1, maxdepth, x1, ym, xm, y2, vertex_parents);
            Self::fill_offsets(depth + 1, maxdepth, xm, ym, x2, y2, vertex_parents);
        }

        vertex_parents.set(x1 as _, y1 as _, ((x1, y1), (x1, y1)));
        vertex_parents.set(x2 as _, y1 as _, ((x2, y1), (x2, y1)));
        vertex_parents.set(x1 as _, y2 as _, ((x1, y2), (x1, y2)));
        vertex_parents.set(x2 as _, y2 as _, ((x2, y2), (x2, y2)));

        if depth < maxdepth {
            vertex_parents.set(x1 as _, ym as _, ((x1, y1), (x1, y2)));
            vertex_parents.set(x2 as _, ym as _, ((x2, y1), (x2, y2)));
            vertex_parents.set(xm as _, y1 as _, ((x1, y1), (x2, y1)));
            vertex_parents.set(xm as _, y2 as _, ((x1, y2), (x2, y2)));
            vertex_parents.set(xm as _, ym as _, ((x1, y1), (x2, y2)));
        }
    }

    pub fn new(pow2size: i32,
               detail: u8,
               radius: f32,
               max_framebuffer_cache: usize,
               vertex_generator: Program,
               post_generator: Program,
               channels: &super::Channels
    ) -> Self {
        let size = 2i32.pow(pow2size as _) as usize;

        // Screen-Space Quad
        let quad_verts: Vec<f32> = vec![-1.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0,];
        let quad = VertexBuffer::from(&quad_verts);

        //
        // The Offset texture decribes for each vertex the higher-level neighbor vertices,
        // against which the vertex has to compare itself
        //
        let mut vertex_parents = Array2D::new(size+1, size+1, ((0,0), (0,0)));
        Self::fill_offsets(0, pow2size as _, 0, 0, size as Idx, size as Idx, &mut vertex_parents);
        let mut offset_tex_data = Vec::new();
        for y in 0..(size+1) {
            for x in 0..(size+1) {
                let parents = vertex_parents.at(x, y);
                offset_tex_data.push((parents.0).0 as f32 / size as f32);
                offset_tex_data.push((parents.0).1 as f32 / size as f32);
                offset_tex_data.push((parents.1).0 as f32 / size as f32);
                offset_tex_data.push((parents.1).1 as f32 / size as f32);
            }
        }

        let mut offset_texture = Texture::new(gl::TEXTURE_2D);
        offset_texture.bind();
        offset_texture.filter(gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
        offset_texture.filter(gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);
        unsafe { offset_texture.teximage(((size+1) as _, (size+1) as _), gl::RGBA32F, gl::RGBA, gl::FLOAT, offset_tex_data.as_ptr() as _); }

        // Set uniforms for Vertex Generator
        vertex_generator.bind();
        vertex_generator.uniform("stretch", Uniform::Float(plate::STRETCH));
        vertex_generator.uniform("stretchAsin", Uniform::Float(plate::STRETCH_ASIN));
        vertex_generator.uniform("invsize", Uniform::Float(1.0 / size as f32));
        vertex_generator.uniform("radius", Uniform::Float(radius));

        // Set uniforms for normal generator / optimizer / channel generator
        post_generator.bind();
        post_generator.uniform("size", Uniform::Float(size as f32));
        post_generator.uniform("radius", Uniform::Float(radius));
        post_generator.uniform("positions", Uniform::Signed(0));
        post_generator.uniform("parentCoords", Uniform::Signed(1));

        Generator {
            depth: pow2size as _,
            size,
            detail: detail,
            channels: channels.clone(),

            quad,
            vertex_generator,
            optimizer: plateoptimizer::PlateOptimizer::new(pow2size as _),
            post_generator,
            offset_texture,

            framebuffer_cache: Vec::new(),
            generation_order: Vec::new(),
            max_framebuffer_cache,
            framebuffers: HashMap::new()
        }
    }

    // tile coords ranging from [1..size+1] for normal texture lookup
    pub fn generate_plate_coords(&self) -> Vec<u16> {
        let mut tile_coords = Vec::new();

        for j in 0..(self.size+3) {
            let j = j.max(1).min(self.size+1);
            let y = (2*j + 1) * 0x7FFF / (self.size+3);

            for i in 0..(self.size+3) {
                let i = i.max(1).min(self.size+1);
                let x = (2*i+1) * 0x7FFF / (self.size+3);

                tile_coords.push(x as u16);
                tile_coords.push(y as u16);
            }
        }

        tile_coords
    }

    pub fn generate_indices(&self) -> (Vec<Idx>, Vec<Idx>) {
        let mut triangles = Vec::new();
        let mut wireframe = Vec::new();

        for i in 0..(self.size+2) {
            let i_edge = (i == 0) || (i == self.size + 1);

            for j in 0..(self.size+2) {
                let j_edge = (j == 0) || (j == self.size + 1);

                let i00 = i       + j       * (self.size + 3);
                let i01 = i       + (j + 1) * (self.size + 3);
                let i10 = (i + 1) + j       * (self.size + 3);
                let i11 = (i + 1) + (j + 1) * (self.size + 3);

                if !i_edge || !j_edge {
                    triangles.push(i00 as Idx);
                    triangles.push(i01 as Idx);
                    triangles.push(i11 as Idx);
                    triangles.push(i00 as Idx);
                    triangles.push(i11 as Idx);
                    triangles.push(i10 as Idx);
                }
                if !i_edge && !j_edge {
                    wireframe.push(i00 as Idx);
                    wireframe.push(i01 as Idx);
                    wireframe.push(i00 as Idx);
                    wireframe.push(i11 as Idx);
                    wireframe.push(i00 as Idx);
                    wireframe.push(i10 as Idx);
                }
            }
        }

        (triangles, wireframe)
    }

    pub fn generate(&mut self, pos: plate::Position) {
        //
        // Generate FBOs
        //
        if self.framebuffers.contains_key(&pos) {
            println!("Already started generation for {}", pos);
            return;
        }

        let tex_size = (self.size + 3) as i32;
        let fbos = match self.framebuffer_cache.pop() {
            Some(fbos) => fbos,
            None => GeneratorBuffers::new(tex_size, &self.channels)
        };

        //
        // Choose Vertex program and assign uniforms
        //
        let fac = 0.5f32.powi(pos.depth() - 1);
        let xofs = -1.0 + pos.x() as f32 * fac;
        let yofs = -1.0 + pos.y() as f32 * fac;

        unsafe {
            gl::Disable(gl::DEPTH_TEST);
            gl::Disable(gl::CULL_FACE);
            gl::Disable(gl::BLEND);
        }

        // Prepare program
        self.vertex_generator.bind();
        self.vertex_generator.uniform("ofs", Uniform::Vec2(Vector2::new(xofs, yofs)));
        self.vertex_generator.uniform("mul", Uniform::Float(fac));
        self.vertex_generator.uniform("depth", Uniform::Signed(pos.depth()));
        self.vertex_generator.uniform("cubeTransformMatrix", Uniform::Mat3(get_vertex_transformation_matrix(pos.direction())));
        self.vertex_generator.vertex_attrib_buffer("xy", &self.quad, 2, gl::FLOAT, false, 8, 0);
        fbos.position_pass.bind();
        unsafe { gl::DrawArrays(gl::TRIANGLES, 0, 6) }

        //
        // Calculate normals
        //
        self.post_generator.bind();
        self.post_generator.vertex_attrib_buffer("xy", &self.quad, 2, gl::FLOAT, false, 8, 0);
        self.offset_texture.bind_at(1);
        fbos.position_pass.texture("position").unwrap().bind_at(0);
        fbos.normal_pass.bind();
        unsafe { gl::DrawArrays(gl::TRIANGLES, 0, 6) }

        OffscreenBuffer::unbind();

        self.generation_order.push(pos);
        self.framebuffers.insert(pos, fbos);
    }

    fn postprocess_ribbons(&self, buffer: &mut Vec<Vector4<f32>>, ribbon_height: f32) {
        let tex_size = (self.size + 3) as usize;

        //
        // adjust heights for ribbon vertices
        //
        for i in 0..(tex_size-1) {
            buffer[i].w -= ribbon_height;   // top
            buffer[(i+1) + tex_size * (tex_size-1)].w -= ribbon_height;   // bottom
            buffer[i * tex_size + (tex_size - 1)].w -= ribbon_height;   // right
            buffer[(i+1) * tex_size].w -= ribbon_height;    // left
        }

        //
        // merge 8 corner ribbon vertices into the actual corner vertices of the plate
        //
        buffer[1] = buffer[0];
        buffer[tex_size] = buffer[0];
        buffer[tex_size-2] = buffer[tex_size-1];
        buffer[2*tex_size-1] = buffer[tex_size-1];
        buffer[tex_size*(tex_size-2)] = buffer[tex_size*(tex_size-1)];
        buffer[tex_size*(tex_size-1)+1] = buffer[tex_size*(tex_size-1)];
        buffer[tex_size*(tex_size-1)-1] = buffer[tex_size*tex_size-1];
        buffer[tex_size*tex_size-2] = buffer[tex_size*tex_size-1];
    }

    pub fn triangulate(&self, data: &Result) -> Triangulation {
        let tex_size = (self.size + 3) as usize;
        let mut vertices = data.vertex_data.clone();

        let optimized = self.optimizer.optimize(|x,y| {
            data.normals[4 * (x + 1 +  tex_size * (y + 1)) + 3] < 255 - self.detail
        });

        // merge interpolated vertices
        for idx in optimized.merged_vertices {
            vertices[idx as usize] = data.merged_data[idx as usize];
        }

        Triangulation::new(self.detail, vertices, optimized.triangles, optimized.wireframe)
    }


    pub fn results(&mut self) -> HashMap<plate::Position, Result> {
        let mut ret = HashMap::new();

        // get generator results in order, in order to leave them as much time as possible on the GPU
        for pos in &self.generation_order {
            let mut fbos = self.framebuffers.remove(&pos).expect("No FBO found");

            //
            // Allocate buffers for regular attributes
            //
            let tex_size = (self.size + 3) as usize;
            let mut buf_positions = Vec::<Vector4<f32>>::with_capacity(tex_size * tex_size);
            let mut buf_heights = Vec::<f32>::with_capacity(tex_size * tex_size);
            let mut buf_merged = Vec::<Vector4<f32>>::with_capacity(tex_size * tex_size);
            let mut buf_normals = Vec::<u8>::with_capacity(tex_size * tex_size);
            buf_positions.resize(tex_size * tex_size, Vector4::new(0.0, 0.0, 0.0, 0.0));
            buf_merged.resize(tex_size * tex_size, Vector4::new(0.0, 0.0, 0.0, 0.0));
            buf_normals.resize(tex_size * tex_size * 4, 0);
            let mut channels = HashMap::new();

            unsafe {
                fbos.position_pass.read("position", buf_positions.as_mut_ptr() as _);

                for chan in self.channels.channels() {
                    if let Some(mut tex) = fbos.position_pass.take(chan.0) {
                        tex.filter(gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
                        tex.filter(gl::TEXTURE_MIN_FILTER, gl::LINEAR_MIPMAP_LINEAR as _);
                        tex.gen_mipmaps();
                        channels.insert(chan.0.clone(), tex);
                    }
                }

                fbos.normal_pass.read("normal", buf_normals.as_mut_ptr() as _);
                fbos.normal_pass.read("merged", buf_merged.as_mut_ptr() as _);
            }

            //
            // get min/max height
            //
            let mut min = 1.0e10f32;
            let mut max = -1.0e10f32;
            for pos_height in &buf_positions {
                let h = pos_height.w;
                buf_heights.push(h);
                min = min.min(h);
                max = max.max(h);
            }

            //
            // adjust ribbon heights and positions
            //
            self.postprocess_ribbons(&mut buf_positions, max - min);
            self.postprocess_ribbons(&mut buf_merged, max - min);

            let mut result = Result::new((min, max), buf_heights, buf_positions, buf_merged, buf_normals, None, channels);
            result.triangulation = Some(self.triangulate(&result));

            if self.framebuffer_cache.len() < self.max_framebuffer_cache {
                self.framebuffer_cache.push(fbos);
            }
            ret.insert(*pos, result);
        }

        self.generation_order.clear();

        ret
    }
}

fn get_vertex_transformation_matrix(dir: plate::Direction) -> Matrix3<f32> {
    match dir {
        plate::Direction::PosX =>
            Matrix3::new(
                0.0, 0.0, 1.0,
                0.0, 1.0, 0.0,
                1.0, 0.0, 0.0
            ),
        plate::Direction::NegX =>
            Matrix3::new(
                0.0, 1.0, 0.0,
                0.0, 0.0, 1.0,
                -1.0, 0.0, 0.0
            ),
        plate::Direction::PosY =>
            Matrix3::new(
                1.0, 0.0, 0.0,
                0.0, 0.0, 1.0,
                0.0, 1.0, 0.0
            ),
        plate::Direction::NegY =>
            Matrix3::new(
                0.0, 0.0, 1.0,
                1.0, 0.0, 0.0,
                0.0, -1.0, 0.0
            ),
        plate::Direction::PosZ =>
            Matrix3::new(
                0.0, 1.0, 0.0,
                1.0, 0.0, 0.0,
                0.0, 0.0, 1.0
            ),
        plate::Direction::NegZ =>
            Matrix3::new(
                1.0, 0.0, 0.0,
                0.0, 1.0, 0.0,
                0.0, 0.0, -1.0
            ),
    }
}
