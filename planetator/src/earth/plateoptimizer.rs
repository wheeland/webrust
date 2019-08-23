use array2d::Array2D;

type Idx = u16;

// TODO:
// - profile
// - merge positions, normals, and all the other vertex data that might have been generated by the user
//   (we can do that in the shader, but need to apply the values here)
pub struct PlateOptimizer {
    depth: Idx,
    size: Idx,
}

pub struct Result {
    pub indices: Vec<Idx>,
    pub wireframe_count: usize,
}

impl PlateOptimizer {
    pub fn new(depth: Idx) -> Self {
        PlateOptimizer {
            depth,
            size: 2usize.pow(depth as _) as _,
        }
    }

    pub fn optimize<T: FnMut(usize, usize)->bool>(&self, can_merge: T) -> Result {
//        time("build", ||IndexBufferBuilder::build(self.depth as _, can_merge))
        IndexBufferBuilder::build(self.depth as _, can_merge)
    }
}

struct IndexBufferBuilder {
    depth: usize,
    size: usize,
    mutable: Array2D<bool>,
    rendered: Array2D<bool>,
    indices: Vec<Idx>,                          // used for both triangles and wireframe
    additional_vertex_indices: Vec<Idx>,        // used only for vertices
}

struct QuadIndices {
    pub i00: Idx,
    pub i0m: Idx,
    pub i01: Idx,
    pub im0: Idx,
    pub imm: Idx,
    pub im1: Idx,
    pub i10: Idx,
    pub i1m: Idx,
    pub i11: Idx,
    pub edge_left: bool,
    pub edge_right: bool,
    pub edge_top: bool,
    pub edge_bottom: bool,
}

impl IndexBufferBuilder {
    // initial foldability for each vertex = (cross < threshold)
    fn build<T: FnMut(usize, usize)->bool>(depth: usize, mut can_merge: T) -> Result {
        let size = 2usize.pow(depth as _);

        //
        // Init vertex data with mutability
        //
        let mut mutable = Array2D::new((size + 1) as _, (size + 1) as _, false);
        let rendered = Array2D::new((size + 1) as _, (size + 1) as _, false);
        for y in 0..(size + 1) {
            for x in 0..(size + 1) {
                mutable.set(x as _, y as _, can_merge(x as _, y as _));
            }
        }

        let mut builder = IndexBufferBuilder {
            depth,
            size,
            mutable,
            rendered,
            indices: Vec::with_capacity(6 * size * size),
            additional_vertex_indices: Vec::with_capacity(6 * size * size),
        };

        builder.process();

        let wireframe_count = builder.indices.len();
        let mut indices = builder.indices;
        indices.append(&mut builder.additional_vertex_indices);

        Result {
            indices,
            wireframe_count
        }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> Idx {
        (x + 1 +  (self.size + 3) * (y + 1)) as Idx
    }

    fn get_indices(&self, x1: usize, xm: usize, x2: usize, y1: usize, ym: usize, y2: usize) -> QuadIndices {
        QuadIndices {
            i00: self.idx(x1, y1),
            i0m: self.idx(x1, ym),
            i01: self.idx(x1, y2),
            im0: self.idx(xm, y1),
            imm: self.idx(xm, ym),
            im1: self.idx(xm, y2),
            i10: self.idx(x2, y1),
            i1m: self.idx(x2, ym),
            i11: self.idx(x2, y2),
            edge_left: (x1 == 0),
            edge_right: (x2 == self.size),
            edge_top: (y1 == 0),
            edge_bottom: (y2 == self.size),
        }
    }

    fn set_rendered_flags_and_get_indices(&mut self, x1: usize, xm: usize, x2: usize, y1: usize, ym: usize, y2: usize) -> QuadIndices {
        self.rendered.set(x1, y1, true);
        self.rendered.set(x1, ym, true);
        self.rendered.set(x1, y2, true);
        self.rendered.set(xm, y1, true);
        self.rendered.set(xm, ym, true);
        self.rendered.set(xm, y2, true);
        self.rendered.set(x2, y1, true);
        self.rendered.set(x2, ym, true);
        self.rendered.set(x2, y2, true);
        self.get_indices(x1, xm, x2, y1, ym, y2)
    }

    fn render(&mut self, i00: Idx, i10: Idx, i01: Idx, i11: Idx) {
        self.indices.push(i00);
        self.indices.push(i01);
        self.indices.push(i11);
        self.indices.push(i11);
        self.indices.push(i10);
        self.indices.push(i00);
    }

    fn ribbon(&mut self, i0: Idx, i1: Idx, ofs: i32) {
        let r0 = (i0 as i32 + ofs) as Idx;
        let r1 = (i1 as i32 + ofs) as Idx;
        self.additional_vertex_indices.push(i0);
        self.additional_vertex_indices.push(i1);
        self.additional_vertex_indices.push(r0);
        self.additional_vertex_indices.push(i1);
        self.additional_vertex_indices.push(r1);
        self.additional_vertex_indices.push(r0);
    }

    fn process(&mut self) {
        let mut thisquads = Some(Array2D::new(0, 0, false));

        let maxlevel = (self.depth - 2).min(1);
        let vribbonofs = (self.size + 3) as i32;

        for level in (maxlevel .. (self.depth) as usize).rev() {
            let count = 2usize.pow(level as _);
            let size = 2usize.pow((self.depth as usize - level) as _) as usize;
            let size2 = size / 2;

            let childquads = thisquads.take().unwrap();
            thisquads = Some(Array2D::new(count, count, false));
            let mut quads = thisquads.as_mut().unwrap();

            debug_assert!(size2 > 0);

            // iterate through all quads, to see if the children have to be rendered
            for quady in 0..count {
                let (y1, ym, y2) = (quady * size, quady * size + size2, quady * size + size);

                for quadx in 0..count {
                    let (x1, xm, x2) = (quadx * size, quadx * size + size2, quadx * size + size);

                    // if non-lowest -> check if one of the children was rendered -> if so, render all the other ones, too
                    if size2 > 1 {
                        let child_rendered: [bool;4] = [
                            *childquads.at(2*quadx    , 2*quady    ),
                            *childquads.at(2*quadx + 1, 2*quady    ),
                            *childquads.at(2*quadx    , 2*quady + 1),
                            *childquads.at(2*quadx + 1, 2*quady + 1)
                        ];
                        if child_rendered[0] || child_rendered[1] || child_rendered[2] || child_rendered[3] {
                            let idx = self.set_rendered_flags_and_get_indices(x1, xm, x2, y1, ym, y2);
                            if !child_rendered[0] {
                                self.render(idx.i00, idx.im0, idx.i0m, idx.imm);
                                if idx.edge_left { self.ribbon(idx.i0m, idx.i00, -1); }
                                if idx.edge_top  { self.ribbon(idx.i00, idx.im0, -vribbonofs); }
                            }
                            if !child_rendered[1] {
                                self.render(idx.im0, idx.i10, idx.imm, idx.i1m);
                                if idx.edge_top  { self.ribbon(idx.im0, idx.i10, -vribbonofs); }
                                if idx.edge_right  { self.ribbon(idx.i10, idx.i1m, 1); }
                            }
                            if !child_rendered[2] {
                                self.render(idx.i0m, idx.imm, idx.i01, idx.im1);
                                if idx.edge_left { self.ribbon(idx.i01, idx.i0m, -1); }
                                if idx.edge_bottom { self.ribbon(idx.im1, idx.i01, vribbonofs); }
                            }
                            if !child_rendered[3] {
                                self.render(idx.imm, idx.i1m, idx.im1, idx.i11);
                                if idx.edge_right  { self.ribbon(idx.i1m, idx.i11, 1); }
                                if idx.edge_bottom { self.ribbon(idx.i11, idx.im1, vribbonofs); }
                            }
                            quads.set(quadx, quady, true);
                            continue;
                        }
                    }

                    let verts = [ (x1, ym), (x2, ym), (xm, y1), (xm, y2), (xm, ym) ];

                    // check if one of the vertices is immutable
                    let mutable = [
                        *self.mutable.at(verts[0].0, verts[0].1),
                        *self.mutable.at(verts[1].0, verts[1].1),
                        *self.mutable.at(verts[2].0, verts[2].1),
                        *self.mutable.at(verts[3].0, verts[3].1),
                        *self.mutable.at(verts[4].0, verts[4].1),
                    ];
                    let all_mutable = mutable[0] && mutable[1] && mutable[2] && mutable[3] && mutable[4];

                    // look at the 5 keypoints. if any of them is immutable, we have to split this
                    // tile. since none of the children have been rendered yet, this means that we render all 4 of them now
                    if !all_mutable {
                        let idx = self.set_rendered_flags_and_get_indices(x1, xm, x2, y1, ym, y2);
                        self.render(idx.i00, idx.im0, idx.i0m, idx.imm);
                        self.render(idx.im0, idx.i10, idx.imm, idx.i1m);
                        self.render(idx.i0m, idx.imm, idx.i01, idx.im1);
                        self.render(idx.imm, idx.i1m, idx.im1, idx.i11);

                        if idx.edge_left { self.ribbon(idx.i01, idx.i0m, -1); }
                        if idx.edge_left { self.ribbon(idx.i0m, idx.i00, -1); }
                        if idx.edge_right  { self.ribbon(idx.i10, idx.i1m, 1); }
                        if idx.edge_right  { self.ribbon(idx.i1m, idx.i11, 1); }
                        if idx.edge_top  { self.ribbon(idx.i00, idx.im0, -vribbonofs); }
                        if idx.edge_top  { self.ribbon(idx.im0, idx.i10, -vribbonofs); }
                        if idx.edge_bottom { self.ribbon(idx.i11, idx.im1, vribbonofs); }
                        if idx.edge_bottom { self.ribbon(idx.im1, idx.i01, vribbonofs); }

                        quads.set(quadx, quady, true);
                        continue;
                    }
                }
            }

            // iterate through all quads AGAIN, to see if we have to render THIS quad
            // (because neighboring children have been rendered in the step above)
            for quady in 0..count {
                let (y1, ym, y2) = (quady * size, quady * size + size2, quady * size + size);

                for quadx in 0..count {
                    if *quads.at(quadx, quady) {
                        continue;
                    }

                    let (x1, xm, x2) = (quadx * size, quadx * size + size2, quadx * size + size);

                    // check if one of the 4 keypoints has been rendered -> if so, we need to render this
                    // in this case, we also need to interpolate all the offending keypoints!
                    let rendered = [
                        *self.rendered.at(x1, ym),
                        *self.rendered.at(x2, ym),
                        *self.rendered.at(xm, y1),
                        *self.rendered.at(xm, y2),
                    ];
                    let some_rendered = rendered[0] || rendered[1] || rendered[2] || rendered[3];
                    if some_rendered || level == maxlevel {
                        let idx = self.get_indices(x1, xm, x2, y1, ym, y2);
                        self.rendered.set(x1, y1, true);
                        self.rendered.set(x1, y2, true);
                        self.rendered.set(x2, y1, true);
                        self.rendered.set(x2, y2, true);
                        self.render(idx.i00, idx.i10, idx.i01, idx.i11);
                        quads.set(quadx, quady, true);

                        if idx.edge_left { self.ribbon(idx.i01, idx.i00, -1); }
                        if idx.edge_right  { self.ribbon(idx.i10, idx.i11, 1); }
                        if idx.edge_top  { self.ribbon(idx.i00, idx.i10, -vribbonofs); }
                        if idx.edge_bottom { self.ribbon(idx.i11, idx.i01, vribbonofs); }

                        // if there is an edge-split towards a neighboring quad, we'll have to
                        //  (a) modify that middle vertex so that it's interpolated between the two corner ones
                        //  (b) add a little triangle that fills the hole, because otherwise there may be artifacts
                        if rendered[0] {
                            self.additional_vertex_indices.push(idx.i00);
                            self.additional_vertex_indices.push(idx.i0m);
                            self.additional_vertex_indices.push(idx.i01);
                        }
                        if rendered[1] {
                            self.additional_vertex_indices.push(idx.i10);
                            self.additional_vertex_indices.push(idx.i11);
                            self.additional_vertex_indices.push(idx.i1m);
                        }
                        if rendered[2] {
                            self.additional_vertex_indices.push(idx.i00);
                            self.additional_vertex_indices.push(idx.i10);
                            self.additional_vertex_indices.push(idx.im0);
                        }
                        if rendered[3] {
                            self.additional_vertex_indices.push(idx.i01);
                            self.additional_vertex_indices.push(idx.im1);
                            self.additional_vertex_indices.push(idx.i11);
                        }
                    }
                }
            }
        }
    }
}
