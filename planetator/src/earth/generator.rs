use tinygl::*;
use array2d::Array2D;

use super::plate;
use super::plateoptimizer;
use super::channels::Channels;
use util3d::noise;

use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use lru_cache::LruCache;
use cgmath::*;

pub type Idx = u16;

//
// Result of a triangle-optimization stage
//
pub struct Triangulation {
    pub indices: Vec<Idx>,
    pub wireframe_count: usize,
}

//
// Data generated on the GPU for one plate
//
pub struct Result {
    pub height_extent: (f32, f32),
    pub heights: Vec<f32>,
    pub vertex_data: Vec<Vector4<f32>>,
    pub detail: Vec<u8>,
    pub tex_heights: Texture,
    pub tex_normals: Texture,
    pub channels: HashMap<String, Texture>,
    pub triangulation: Option<Triangulation>,
}

//
// Central instance for requesting and returning tile data
//
pub struct PlateDataManager {
    vertex_size: u32,
    radius: f32,
    generator: Generator,
    cache: LruCache<plate::Position, Result>,
    waiting: HashMap<plate::Position, f32>,
}
pub type PlateDataManagerPtr = Rc<RefCell<PlateDataManager>>;

impl PlateDataManager {
    pub fn new(vertex_depth: u32, texture_delta: u32, radius: f32, vertex_generator: Program, channels: &Channels) -> Self {
        PlateDataManager {
            vertex_size: 2u32.pow(vertex_depth),
            radius,
            generator: Generator::new(vertex_depth, texture_delta as _, 100, radius, 12, vertex_generator, channels),
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

        let mut count = 0;
        for entry in entries {
            if self.generator.start_generation(*entry.0) {
                count += 1;
                if count >= max {
                    break;
                }
            }
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
        let vertex_depth = self.generator.vertex_depth;
        let texture_delta = self.generator.texture_delta;
        super::util::generate_tex_coords_buffer(vertex_depth, vertex_depth, texture_delta)
    }

    pub fn generate_indices(&self) -> (Vec<Idx>, Vec<Idx>) {
        self.generator.generate_indices()
    }

    pub fn vertex_tile_size(&self) -> u32 {
        self.vertex_size
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }
}

//
// Offscreen FBOs used during the plate data generation and optimization phases
//
struct GeneratorBuffers {
    // high-res, contains one RGBA f32 texture with Pos/Height, plus one texture for each channel
    position_pass: FrameBufferObject,

    // high-res, contains only one normal RGB8 texture
    normal_pass: FrameBufferObject,

    // low-res, contains heights and detail numbers for each vertex and
    downscale_pass: FrameBufferObject,
}

impl GeneratorBuffers {
    fn new(tex_size: i32, vert_size: i32, channels: &Channels) -> Self {
        let mut position_pass = FrameBufferObject::new((tex_size, tex_size));
        position_pass.add("posHeight", gl::RGBA32F, gl::RGBA, gl::FLOAT);
        position_pass.add("height", gl::R32F, gl::RED, gl::FLOAT);
        // TODO: avoid duplication
        for chan in channels.iter() {
            let int_fmt = match chan.1 {
                1 => (gl::R8, gl::RED),
                2 => (gl::RG8, gl::RG),
                3 => (gl::RGB8, gl::RGB),
                4 => (gl::RGBA8, gl::RGBA),
                _ => { panic!("Does not compute"); },
            };
            position_pass.add(&chan.0, int_fmt.0, int_fmt.1, gl::UNSIGNED_BYTE);
        }

        let mut normal_pass = FrameBufferObject::new((tex_size, tex_size));
        normal_pass.add("normal", gl::RGB, gl::RGB, gl::UNSIGNED_BYTE);

        let mut downscale_pass = FrameBufferObject::new((vert_size, vert_size));
        downscale_pass.add("posHeight", gl::RGBA32F, gl::RGBA, gl::FLOAT);
        downscale_pass.add("detail", gl::RGBA, gl::RGBA, gl::UNSIGNED_BYTE);

        GeneratorBuffers {
            position_pass,
            normal_pass,
            downscale_pass,
        }
    }
}

fn passthrough_vert() ->  & 'static str {
    "in vec2 xy;
    void main()
    {
        gl_Position = vec4(xy, 0.0, 1.0);
    }"
}

//
// Shaders for Vertex + Height (+ Channels) generation
//
pub fn compile_generator(generator: &str, channels: &Channels) -> Program {
    let frag = noise::ShaderNoise::declarations() + "
        uniform vec2 ofs;
        uniform float invTargetSize;
        uniform float targetBorderOffset;
        uniform float stretch;
        uniform float stretchAsin;
        uniform float mul;
        uniform float radius;
        uniform int depth;
        uniform mat3 cubeTransformMatrix;

        layout(location = 0) out vec4 posHeight;
        layout(location = 1) out float height;
        "
        + &channels.glsl_output_declarations(2) + "
        \n#line 1\n"
        + generator + "

        void main()
        {
            vec2 rel = (gl_FragCoord.xy - vec2(targetBorderOffset)) * invTargetSize;
            vec2 rawXy = asin(stretch * (ofs + vec2(mul) * rel)) / stretchAsin;

            vec2 xy = clamp(vec2(-1.0), rawXy, vec2(1.0));
            vec2 diff = abs(rawXy - xy);
            float dz = max(diff.x, diff.y);

            vec3 cubePos = cubeTransformMatrix * vec3(xy, 1.0 - dz);
            vec3 position = normalize(cubePos);

            height = 0.0;
            generate(position * radius, depth);

            posHeight = vec4(position, height);
        }" + &noise::ShaderNoise::definitions();

    Program::new(passthrough_vert(), &frag)
}

//
// Shader for Normals generation
//
fn compile_normals_shader() -> Program {
    let frag = "
        uniform float invTargetSize;
        uniform float radius;
        uniform sampler2D positions;

        layout(location = 0) out vec3 normal;

        vec3 _pos(vec2 pixel) {
            vec4 heightPos = texture(positions, pixel * invTargetSize);
            return heightPos.xyz * (radius + heightPos.w);
        }

        void main()
        {
            //
            // Get coordinates of neighbor vertices and build normal
            //
            vec3 xp = _pos(gl_FragCoord.xy + vec2(1.0,  0.0));
            vec3 xn = _pos(gl_FragCoord.xy + vec2(-1.0, 0.0));
            vec3 yp = _pos(gl_FragCoord.xy + vec2(0.0,  1.0));
            vec3 yn = _pos(gl_FragCoord.xy + vec2(0.0, -1.0));

            vec3 norm = normalize(cross(xp - xn, yp - yn));
            if (dot(norm, xp) < 0.0)
                norm = -norm;

            normal = vec3(0.5) + 0.5 * norm;
        }";

    Program::new(passthrough_vert(), &frag)
}

//
// Shader for Downscaling and detail computation
//
fn compile_postvertex() -> Program {
    let frag = "
        uniform float vertexGridSize;
        uniform float textureDelta;
        uniform float textureSize;
        uniform float radius;
        uniform sampler2D positions;
        uniform sampler2D parentCoords;

        layout(location = 0) out vec4 posHeight;
        layout(location = 1) out vec4 detail;

        vec4 getHeightPos(vec2 pixelCenter) {
            vec2 texPixel = (pixelCenter - vec2(0.5)) * textureDelta + vec2(0.5);
            return texture(positions, texPixel / textureSize);
        }

        vec3 _pos(vec2 tc) {
            vec4 heightPos = getHeightPos(tc);
            return heightPos.xyz * (radius + heightPos.w);
        }

        void main()
        {
            //
            // Get coordinates of neighbor vertices
            //
            vec4 heightPosCenter = getHeightPos(gl_FragCoord.xy);
            vec3 pCenter = heightPosCenter.xyz * (radius + heightPosCenter.w);

            // get position of parent vertices within this tile (range: [0..1])
            vec4 parents = texture(parentCoords, (gl_FragCoord.xy - vec2(1.0)) / (vertexGridSize + 1.0));

            //
            // calculate interpolated position
            //
            float interpolation = 0.0;
            if (parents.xy != parents.zw) {
                // read parent world positions
                vec3 pparent1 = _pos(vec2(1.5) + parents.xy * vertexGridSize);
                vec3 pparent2 = _pos(vec2(1.5) + parents.zw * vertexGridSize);
                vec3 mid = mix(pparent1, pparent2, 0.5);

                // calculate relative difference to this position
                float dParents = length(pparent1 - pparent2);
                float dMid = length(mid - pCenter);
                interpolation = 0.5 * dMid / dParents * sqrt(length(parents.xy - parents.zw));
            }

            posHeight = heightPosCenter;
            detail = vec4(5.0 * interpolation * sqrt(vertexGridSize));
        }";

    Program::new(passthrough_vert(), &frag)
}

//
// Responsible for actually generating the tile data using specialized shaders
//
struct Generator {
    vertex_depth: u32,
    vertex_grid_size: u32,
    texture_delta: u32,
    texture_size: u32,      // actual size of the generated textures
    pub detail: u8,
    channels: Channels,

    quad: VertexBuffer,
    vertex_generator: Program,
    normals_generator: Program,
    post_generator: Program,
    optimizer: plateoptimizer::PlateOptimizer,

    offset_texture: Texture,    // parent vertices (the two that the actual vertex is between)
    framebuffer_cache: Vec<GeneratorBuffers>,
    max_framebuffer_cache: usize,

    current_operations: Vec<plate::Position>,
    previous_operations: Vec<plate::Position>,
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

    pub fn new(vertex_depth: u32,
               texture_delta: u32,
               detail: u8,
               radius: f32,
               max_framebuffer_cache: usize,
               vertex_generator: Program,
               channels: &Channels
    ) -> Self {
        let vertex_size = 2u32.pow(vertex_depth);
        let texture_size = (vertex_size + 2) * 2u32.pow(texture_delta) + 1;

        // Screen-Space Quad
        let quad_verts: Vec<f32> = vec![-1.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0,];
        let quad = VertexBuffer::from(&quad_verts);

        //
        // The Offset texture decribes for each vertex the higher-level neighbor vertices,
        // against which the vertex has to compare itself
        //
        let mut vertex_parents = Array2D::new((vertex_size+1) as _, (vertex_size+1) as _, ((0,0), (0,0)));
        Self::fill_offsets(0, vertex_depth as _, 0, 0, vertex_size as Idx, vertex_size as Idx, &mut vertex_parents);
        let mut offset_tex_data = Vec::new();
        for y in 0..(vertex_size+1) {
            for x in 0..(vertex_size+1) {
                let parents = vertex_parents.at(x as _, y as _);
                offset_tex_data.push((parents.0).0 as f32 / vertex_size as f32);
                offset_tex_data.push((parents.0).1 as f32 / vertex_size as f32);
                offset_tex_data.push((parents.1).0 as f32 / vertex_size as f32);
                offset_tex_data.push((parents.1).1 as f32 / vertex_size as f32);
            }
        }

        let mut offset_texture = Texture::new(gl::TEXTURE_2D);
        offset_texture.filter(gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
        offset_texture.filter(gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);
        unsafe { offset_texture.teximage(((vertex_size+1) as _, (vertex_size+1) as _), gl::RGBA32F, gl::RGBA, gl::FLOAT, offset_tex_data.as_ptr() as _); }

        // Set uniforms for Vertex/Height/Channels Generator
        vertex_generator.bind();
        vertex_generator.uniform("stretch", Uniform::Float(plate::STRETCH));
        vertex_generator.uniform("stretchAsin", Uniform::Float(plate::STRETCH_ASIN));
        vertex_generator.uniform("invTargetSize", Uniform::Float(0.5f32.powi((vertex_depth + texture_delta) as _)));
        vertex_generator.uniform("targetBorderOffset", Uniform::Float(2.0f32.powi(texture_delta as _) + 0.5));
        vertex_generator.uniform("radius", Uniform::Float(radius));

        // Set uniforms for normal generator
        let normals_generator = compile_normals_shader();
        normals_generator.bind();
        normals_generator.uniform("invTargetSize", Uniform::Float(1.0 / texture_size as f32));
        normals_generator.uniform("radius", Uniform::Float(radius));
        normals_generator.uniform("positions", Uniform::Signed(0));

        // Set uniforms for downscaling and detail computation shader
        let post_generator = compile_postvertex();
        post_generator.bind();
        post_generator.uniform("vertexGridSize", Uniform::Float(vertex_size as f32));
        post_generator.uniform("textureDelta", Uniform::Float(2.0f32.powi(texture_delta as _)));
        post_generator.uniform("textureSize", Uniform::Float(texture_size as f32));
        post_generator.uniform("radius", Uniform::Float(radius));
        post_generator.uniform("positions", Uniform::Signed(0));
        post_generator.uniform("parentCoords", Uniform::Signed(1));

        Generator {
            vertex_depth,
            vertex_grid_size: vertex_size,
            texture_delta,
            texture_size,
            detail: detail,
            channels: channels.clone(),

            quad,
            vertex_generator,
            optimizer: plateoptimizer::PlateOptimizer::new(vertex_depth as _),
            post_generator,
            normals_generator,
            offset_texture,

            framebuffer_cache: Vec::new(),
            current_operations: Vec::new(),
            previous_operations: Vec::new(),
            max_framebuffer_cache,
            framebuffers: HashMap::new()
        }
    }

    pub fn generate_indices(&self) -> (Vec<Idx>, Vec<Idx>) {
        let mut triangles = Vec::new();
        let mut wireframe = Vec::new();

        for i in 0..(self.vertex_grid_size+2) {
            let i_edge = (i == 0) || (i == self.vertex_grid_size + 1);

            for j in 0..(self.vertex_grid_size+2) {
                let j_edge = (j == 0) || (j == self.vertex_grid_size + 1);

                let i00 = i       + j       * (self.vertex_grid_size + 3);
                let i01 = i       + (j + 1) * (self.vertex_grid_size + 3);
                let i10 = (i + 1) + j       * (self.vertex_grid_size + 3);
                let i11 = (i + 1) + (j + 1) * (self.vertex_grid_size + 3);

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

    pub fn start_generation(&mut self, pos: plate::Position) -> bool {
        //
        // Generate FBOs
        //
        if self.framebuffers.contains_key(&pos) {
            return false;
        }

        let vert_size = (self.vertex_grid_size + 3) as i32;
        let fbos = match self.framebuffer_cache.pop() {
            Some(fbos) => fbos,
            None => GeneratorBuffers::new(self.texture_size as _, vert_size, &self.channels)
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

        //
        // Prepare program
        //
        self.vertex_generator.bind();
        self.vertex_generator.uniform("ofs", Uniform::Vec2(Vector2::new(xofs, yofs)));
        self.vertex_generator.uniform("mul", Uniform::Float(fac));
        self.vertex_generator.uniform("depth", Uniform::Signed(pos.depth()));
        self.vertex_generator.uniform("cubeTransformMatrix", Uniform::Mat3(pos.direction().square_to_cubic_transform()));
        self.vertex_generator.vertex_attrib_buffer("xy", &self.quad, 2, gl::FLOAT, false, 8, 0);
        fbos.position_pass.bind();
        unsafe { gl::DrawArrays(gl::TRIANGLES, 0, 6) }
        self.vertex_generator.disable_all_vertex_attribs();

        //
        // Calculate normals
        //
        self.normals_generator.bind();
        self.normals_generator.vertex_attrib_buffer("xy", &self.quad, 2, gl::FLOAT, false, 8, 0);
        fbos.position_pass.texture("posHeight").unwrap().bind_at(0);
        fbos.normal_pass.bind();
        unsafe { gl::DrawArrays(gl::TRIANGLES, 0, 6) }
        self.normals_generator.disable_all_vertex_attribs();

        //
        // Downscale heights and calculate detail factors
        //
        self.post_generator.bind();
        self.post_generator.vertex_attrib_buffer("xy", &self.quad, 2, gl::FLOAT, false, 8, 0);
        self.offset_texture.bind_at(1);
        fbos.downscale_pass.bind();
        unsafe { gl::DrawArrays(gl::TRIANGLES, 0, 6) }
        self.post_generator.disable_all_vertex_attribs();

        FrameBufferObject::unbind();

        self.current_operations.push(pos);
        self.framebuffers.insert(pos, fbos);

        true
    }

    fn postprocess_ribbons(&self, buffer: &mut Vec<Vector4<f32>>, ribbon_height: f32) {
        // size of the pos/height buffer texture
        let tex_size = (self.vertex_grid_size + 3) as usize;

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
        // size of the pos/height buffer texture
        let tex_size = (self.vertex_grid_size + 3) as usize;

        let optimized = self.optimizer.optimize(|x,y| {
            data.detail[x + 1 +  tex_size * (y + 1)] < 255 - self.detail
        });

        Triangulation {
            indices: optimized.indices,
            wireframe_count: optimized.wireframe_count,
        }
    }

    pub fn results(&mut self) -> HashMap<plate::Position, Result> {
        let mut ret = HashMap::new();

        // get generator results in order, in order to leave them as much time as possible on the GPU
        for pos in &self.previous_operations {
            let mut fbos = self.framebuffers.remove(&pos).expect("No FBO found");

            // sorry, we need this
            let mut tex_heights = fbos.position_pass.take("height").expect("No Height texture found");
            tex_heights.filter(gl::TEXTURE_MIN_FILTER, gl::LINEAR);
            tex_heights.filter(gl::TEXTURE_MAG_FILTER, gl::LINEAR);

            //
            // Allocate buffers for regular attributes
            //
            let tex_size = (self.vertex_grid_size + 3) as usize;    // size of the pos/height buffer texture
            let mut buf_positions = Vec::<Vector4<f32>>::with_capacity(tex_size * tex_size);
            let mut buf_heights = Vec::<f32>::with_capacity(tex_size * tex_size);
            let mut buf_detail = Vec::<u8>::with_capacity(4 * tex_size * tex_size);
            buf_positions.resize(tex_size * tex_size, Vector4::new(0.0, 0.0, 0.0, 0.0));
            buf_detail.resize(tex_size * tex_size * 4, 0);
            let mut channels = HashMap::new();

            unsafe {
                fbos.downscale_pass.read("posHeight", buf_positions.as_mut_ptr() as _);
                fbos.downscale_pass.read("detail", buf_detail.as_mut_ptr() as _);

                for chan in self.channels.iter() {
                    if let Some(mut tex) = fbos.position_pass.take(chan.0) {
                        tex.filter(gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
                        tex.filter(gl::TEXTURE_MIN_FILTER, gl::LINEAR_MIPMAP_LINEAR as _);
                        tex.gen_mipmaps();
                        channels.insert(chan.0.clone(), tex);
                    }
                }
            }

            //
            // Convert from RGBA_U8 to U8 detail texture
            //
            let mut buf_detail_u8 = Vec::<u8>::with_capacity(tex_size * tex_size);
            for i in 0..(tex_size*tex_size) {
                buf_detail_u8.push(buf_detail[4*i]);
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
            // Prepare normals texture
            //
            let mut tex_normals = fbos.normal_pass.take("normal").unwrap();
            tex_normals.filter(gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
            tex_normals.filter(gl::TEXTURE_MIN_FILTER, gl::LINEAR_MIPMAP_LINEAR as _);
            tex_normals.wrap(gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE);
            tex_normals.wrap(gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE);
            tex_normals.gen_mipmaps();

            //
            // adjust ribbon heights and positions
            //
            self.postprocess_ribbons(&mut buf_positions, max - min);
            let mut result = Result {
                height_extent: (min, max),
                heights: buf_heights,
                vertex_data: buf_positions,
                detail: buf_detail_u8,
                tex_heights,
                tex_normals,
                channels,
                triangulation: None,
            };
            result.triangulation = Some(self.triangulate(&result));

            if self.framebuffer_cache.len() < self.max_framebuffer_cache {
                self.framebuffer_cache.push(fbos);
            }
            ret.insert(*pos, result);
        }

        self.previous_operations = self.current_operations.drain(..).collect();
        self.current_operations.clear();

        ret
    }
}
