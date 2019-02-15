use std::rc::Rc;
use std::cell::RefCell;

use cgmath::prelude::*;
use cgmath::*;
use super::super::culling;
use super::generator;
use super::plate;

pub struct Planet {
    plate_size: i32,
    radius: f32,
    seed: cgmath::Vector3<f32>,

    plate_coords: tinygl::VertexBuffer,
    triangles: tinygl::IndexBuffer,
    wireframe: tinygl::IndexBuffer,

    root_plates: Vec<PlatePtr>,
    plate_data_manager: generator::PlateDataManagerPtr
}

impl Planet {
    pub fn new(conf: &super::Configuration) -> Result<Planet, (Option<String>, Option<String>)> {
        let size = 2i32.pow(conf.size as _);

        // Create Generator program
        let generator = generator::compile_generator(&conf.generator, &conf.channels);
        let generator_errs = if generator.valid() { None } else { Some(generator.fragment_log()) };

        // Create normals / channels / optimization program
        let post = generator::compile_postvertex(&super::Channels::new(&Vec::new()));
        let post_errs = if post.valid() { None } else { Some(post.fragment_log()) };

        if !generator.valid() || !post.valid() {
            return Err((generator_errs, post_errs));
        }

        let mut manager = generator::PlateDataManager::new(conf.size as _, conf.radius,generator, post, &conf.channels);
        manager.set_detail(conf.detail as _);

        let indices = manager.generate_indices();
        let plate_coords = tinygl::VertexBuffer::from(&manager.generate_plate_coords());

        let manager = Rc::new(RefCell::new(manager));

        let dirs = [plate::Direction::PosX, plate::Direction::PosY, plate::Direction::PosZ, plate::Direction::NegX, plate::Direction::NegY, plate::Direction::NegZ];
        let root_plates = dirs.iter().map(|dir| {
            Plate::new(plate::Position::root(*dir), &manager)
        }).collect();

        Ok(Planet {
            plate_size: size,
            radius: conf.radius,
            seed: cgmath::Vector3::new(0.0, 0.0, 0.0),
            plate_coords,
            triangles: tinygl::IndexBuffer::from16(&indices.0),
            wireframe: tinygl::IndexBuffer::from16(&indices.1),
            root_plates,
            plate_data_manager: manager
        })
    }

    pub fn plate_coords(&self) -> &tinygl::VertexBuffer { &self.plate_coords }
    pub fn triangle_indices(&self) -> &tinygl::IndexBuffer { &self.triangles }
    pub fn wireframe_indices(&self) -> &tinygl::IndexBuffer { &self.wireframe }

    pub fn set_detail(&mut self, detail: u8) {
        self.plate_data_manager.borrow_mut().set_detail(detail);

        let sz = self.plate_size + 3;
        self.traverse_mut(|node| {
            if node.generated_data.is_some() {
                node.data_manager.borrow().retriangulate(node.generated_data.as_mut().unwrap());
                node.gpu_data = Some(GpuData::new(node.generated_data.as_ref().unwrap(), sz));
            }
            true
        });
    }

    pub fn traverse<T: FnMut(&Plate) -> bool>(&self, mut functor: T) {
        let f = &mut functor;
        for root in &self.root_plates {
            Plate::traverse_helper(&(*root.borrow()), f);
        }
    }

    pub fn traverse_mut<T: FnMut(&mut Plate) -> bool>(&mut self, mut functor: T) {
        let f = &mut functor;
        for root in &mut self.root_plates {
            Plate::traverse_mut_helper(&mut(*root.borrow_mut()), f);
        }
    }

    // Adjust Quad-Tree to the current camera frustum and LOD
    pub fn update_quad_tree(&mut self, eye: &Vector3<f32>, culler: &culling::Culler, max_level: i32, hide_backside: bool) {
        let radius = self.radius;
        let camdir = plate::Direction::spherical_to_dir_and_square(eye);

        self.traverse_mut(|node| {
            let in_bounds = culler.visible(&node.bounds);
            node.visible = in_bounds && !(hide_backside && node.is_backside(eye, camdir.0, camdir.1));

            let dist = node.distance(eye) / radius;
            let required_detail = 4.0 * 0.5f32.powi(node.position().depth());

            if dist < required_detail && node.visible && node.position.depth() < max_level {
                node.create_children();
            } else {
                node.delete_children();
            }
            node.my_priority = required_detail / dist;

            true
        });
    }

    pub fn waiting_plates_size(&self) -> usize {
        self.plate_data_manager.borrow().waiting()
    }

    pub fn start_data_generation(&mut self, max: usize) {
        self.plate_data_manager.borrow_mut().start_data_generation(max);
    }

    pub fn collect_render_data(&mut self) {
        self.plate_data_manager.borrow_mut().collect_render_data();
    }

    // Update requests with appropriate priorities
    pub fn update_priorities(&mut self) {
        for root in &mut self.root_plates {
            root.borrow_mut().update_priority();
        }
    }

    fn collect_rendered_plates(plate: &PlatePtr, out: &mut Vec<PlatePtr>) {
        let node = &(*plate.borrow());
        if node.visible {
            // check if we need to render the node
            let render = match node.children.as_ref() {
                None => true,
                Some(c) => !c.iter().all(|child| child.borrow().has_render_data())
            };

            if render && node.has_render_data() {
                out.push(plate.clone());
            }

            if !render {
                for c in node.children.as_ref().unwrap() {
                    Self::collect_rendered_plates(c, out);
                }
            }
        }
    }

    // Call render callback for all leaf nodes with RenderData
    pub fn rendered_plates(&self) -> Vec<PlatePtr> {
        let mut ret = Vec::new();
        for root in &self.root_plates {
            Self::collect_rendered_plates(&root, &mut ret);
        }
        ret
    }
}

struct GpuData {
    pub positions: tinygl::VertexBuffer,
    pub normals: tinygl::Texture,
    pub triangles: tinygl::IndexBuffer,
    pub wireframe: tinygl::IndexBuffer
}

impl GpuData {
    fn new(data: &generator::Result, tex_size: i32) -> Self {
        let triangulation = data.triangulation.as_ref().expect("No Triangulation data found");

        let mut ret = GpuData {
            positions: tinygl::VertexBuffer::from(&triangulation.vertices),
            normals: tinygl::Texture::new(gl::TEXTURE_2D),
            triangles: tinygl::IndexBuffer::from16(&triangulation.triangles),
            wireframe: tinygl::IndexBuffer::from16(&triangulation.wireframe),
        };
        unsafe {
            ret.normals.teximage((tex_size, tex_size), gl::RGBA8, gl::RGBA, gl::UNSIGNED_BYTE, data.normals.as_ptr() as _);
        }
        ret.normals.filter(gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
        ret.normals.filter(gl::TEXTURE_MIN_FILTER, gl::LINEAR_MIPMAP_LINEAR as _);
        ret.normals.wrap(gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE);
        ret.normals.wrap(gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE);
        ret.normals.gen_mipmaps();

        ret
    }
}

type PlatePtr = Rc<RefCell<Plate>>;

pub struct Plate {
    position: plate::Position,
    bogo_points: [Vector3<f32>;9],      // list of points covering all the extreme positions of this plate
    bounds: culling::Sphere,
    visible: bool,

    my_priority: f32,
    total_priority: f32,
    data_manager: generator::PlateDataManagerPtr,

    generated_data: Option<generator::Result>,
    gpu_data: Option<GpuData>,
    debug_color: Vector3<f32>,

    children: Option<[PlatePtr;4]>,
}

impl Plate {
    fn bogo_bounding_box(bogo_points: &[Vector3<f32>;9], radius: f32, minheight: f32, maxheight: f32) -> culling::Sphere {
        let mut maxr2 = 0.0f32;
        let center = bogo_points[4] * radius;

        for pt in bogo_points {
            for h in [minheight, maxheight].iter() {
                let pt = pt * (radius + h);
                let dist2 = (pt - center).magnitude2();
                maxr2 = maxr2.max(dist2);
            }
        };
        let maxr = maxr2.sqrt();
        culling::Sphere::from(center, maxr)
    }

    fn set_data(&mut self, data: generator::Result) {
        let radius = self.data_manager.borrow().radius();
        self.bounds = Self::bogo_bounding_box(&self.bogo_points, radius, data.height_extent.0, data.height_extent.1);
        self.gpu_data = Some(GpuData::new(&data, self.data_manager.borrow().size() + 3));
        self.generated_data = Some(data);
    }

    fn new(position: plate::Position, data_manager: &generator::PlateDataManagerPtr) -> PlatePtr {
        let data_manager = data_manager.clone();
        let render_data = data_manager.borrow_mut().request(&position, 0.0);
        let minmax = render_data.as_ref().map_or((0.0, 0.0), |rd| rd.height_extent);

        let radius = data_manager.borrow().radius();
        let bogo_points = [
            position.square_to_sphere(&Vector2::new(0.0, 0.0)),
            position.square_to_sphere(&Vector2::new(0.5, 0.0)),
            position.square_to_sphere(&Vector2::new(1.0, 0.0)),
            position.square_to_sphere(&Vector2::new(0.0, 0.5)),
            position.square_to_sphere(&Vector2::new(0.5, 0.5)),
            position.square_to_sphere(&Vector2::new(1.0, 0.5)),
            position.square_to_sphere(&Vector2::new(0.0, 1.0)),
            position.square_to_sphere(&Vector2::new(0.5, 1.0)),
            position.square_to_sphere(&Vector2::new(1.0, 1.0)),
        ];
        let bounds = Self::bogo_bounding_box(&bogo_points, radius, minmax.0, minmax.1);

        let mut ret = Plate {
            position,
            bogo_points,
            bounds,
            visible: false,
            my_priority: 0.0,
            total_priority: 0.0,
            data_manager,
            generated_data: None,
            debug_color: tinygl::util::hsv(((position.x() + 100) * (position.y() + 200)) as f32, 1.0, 1.0),
            gpu_data: None,
            children: None
        };
        if let Some(data) = render_data {
            ret.set_data(data);
        }

        Rc::new(RefCell::new(ret))
    }

    pub fn is_leaf(&self) -> bool { self.children.is_none() }
    pub fn position(&self) -> plate::Position { self.position }
    pub fn has_render_data(&self) -> bool { self.generated_data.is_some() }

    pub fn height_at(&self, local: Vector2<f32>) -> f32 {
        match self.generated_data.as_ref() {
            None => 0.0,
            Some(rd) => {
                let sz = self.data_manager.borrow().size();
                let x = local.x.min(1.0).max(0.0);
                let y = local.y.min(1.0).max(0.0);
                let x = (x * sz as f32) as i32 + 1;
                let y = (y * sz as f32) as i32 + 1;
                let idx = x * (sz + 3) + y;
                let ret = rd.heights[idx as usize];
                if ret.is_nan() { 0.0 } else { ret }
            }
        }
    }

    pub fn distance(&self, pos: &Vector3<f32>) -> f32 {
        let root = self.position.direction().spherical_to_square(pos);
        let mut local = self.position.square_from_root(&root);

        local.x = local.x.min(1.0).max(0.0);
        local.y = local.y.min(1.0).max(0.0);
        let radius = self.data_manager.borrow().radius();
        let height = self.height_at(local);
        let global = self.position.square_to_sphere(&local) * (radius + height);

        (global - pos).magnitude()
    }

    pub fn bogo_distance(&self, pos: Vector3<f32>) -> f32 {
        self.bogo_points
            .iter()
            .map(|pt| (*pt - pos).magnitude2())
            .min_by(|a,b| a.partial_cmp(b).unwrap())
            .expect("No bogo_points defined")
            .sqrt()
    }

    pub fn is_backside(&self, eye: &Vector3<f32>, dir: plate::Direction, root: Vector2<f32>) -> bool {
        // if the eye is on top of this plate, we should ignore the randomly scattered checkpoints
        // and look at the normal directly underneath us
        if self.position.direction() == dir {
            let mut local = self.position.square_from_root(&root);
            local.x = local.x.min(1.0).max(0.0);
            local.y = local.y.min(1.0).max(0.0);
            let pt = self.position.square_to_sphere(&local);
            if (pt - eye).dot(pt) < 0.0 {
               return false
            }
        }

        let minh = self.generated_data.as_ref().map_or(0.0, |rd| rd.height_extent.0);
        let radius = self.data_manager.borrow().radius();
        self.bogo_points.iter().all(|pt| {
            let pt = pt * (radius + minh);
            let dir = (pt - eye).normalize();
            dir.dot(pt) > 0.2
        })
    }

    fn create_children(&mut self) {
        self.children = match self.children.take() {
            None => { Some(
                [
                    Self::new(self.position.child(0, 0), &self.data_manager, self.minmax),
                    Self::new(self.position.child(0, 1), &self.data_manager, self.minmax),
                    Self::new(self.position.child(1, 0), &self.data_manager, self.minmax),
                    Self::new(self.position.child(1, 1), &self.data_manager, self.minmax),
                ]) },
            Some(some) => Some(some)
        };
    }

    fn delete_children(&mut self) {
        // make sure to go recursively till the leaves of the tree to release double parent-child links
        if let Some(ref children) = self.children {
            for c in children {
                c.borrow_mut().delete_children();
            }
        }
        self.children = None;
    }

    fn update_priority(&mut self) -> f32 {
        let mut prio = self.my_priority;
        if let Some(children) = self.children.as_ref() {
            for c in children {
                prio += c.borrow_mut().update_priority();
            }
        }
        self.total_priority = prio;

        if !self.has_render_data() {
            let data = self.data_manager.borrow_mut().request(&self.position, self.total_priority);
            if let Some(rd) = data {
                self.set_data(rd);
            }
        }

        prio
    }

    fn traverse_helper<T: FnMut(&Plate) -> bool>(this: &Self, functor: &mut T) {
        if functor(this) {
            if let Some(children) = this.children.as_ref() {
                for c in children {
                    Self::traverse_helper(&(*c.borrow()), functor);
                }
            }
        }
    }

    fn traverse_mut_helper<T: FnMut(&mut Plate) -> bool>(this: &mut Plate, functor: &mut T) {
        if functor(this) {
            if let Some(children) = this.children.as_mut() {
                for c in children {
                    Self::traverse_mut_helper(&mut (*c.borrow_mut()), functor);
                }
            }
        }
    }

    pub fn traverse<T: FnMut(&Plate) -> bool>(&self, mut functor: T) {
        Self::traverse_helper(self, &mut functor);
    }

    pub fn traverse_mut<T: FnMut(&mut Plate) -> bool>(&mut self, mut functor: T) {
        Self::traverse_mut_helper(self, &mut functor);
    }

    pub fn bind_render_data(&self, program: &tinygl::Program) {
        let render_data = self.gpu_data.as_ref().expect("Expected GpuData");
        program.vertex_attrib_buffer("posHeight", &render_data.positions, 4, gl::FLOAT, false, 16, 0);

        for channel in self.generated_data.as_ref().unwrap().channels.iter().enumerate() {
            let idx = channel.0 + 1;
            (channel.1).1.bind_at(idx as _);
            program.uniform(&(String::from("texture_") + (channel.1).0), tinygl::Uniform::Signed(idx as i32));
        }

        program.uniform("debugColor", tinygl::Uniform::Vec3(self.debug_color));

        render_data.normals.bind_at(0);
    }

    pub fn debug_color(&self) -> Vector3<f32> {
        self.debug_color
    }

    pub fn indices(&self) -> &tinygl::IndexBuffer {
        let rd = self.gpu_data.as_ref().expect("Expected GpuData");
        &rd.triangles
    }

    pub fn wireframe(&self) -> &tinygl::IndexBuffer {
        let rd = self.gpu_data.as_ref().expect("Expected GpuData");
        &rd.wireframe
    }
}

impl Drop for Plate {
    fn drop(&mut self) {
        match self.generated_data.take() {
            None => self.data_manager.borrow_mut().abort(&self.position),
            Some(data) => self.data_manager.borrow_mut().insert(&self.position, data)
        }
    }
}
