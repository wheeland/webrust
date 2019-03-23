use gl::types::*;

struct BufferBase {
    target: GLenum,
    buffer: GLuint
}

impl BufferBase {
    fn new<T>(target: GLenum, usage: GLenum, data: &Vec<T>) -> BufferBase {
        let mut buffer = 0;

        unsafe {
            gl::GenBuffers(1, &mut buffer);
            gl::BindBuffer(target, buffer);
            gl::BufferData(target,
                           (data.len() * std::mem::size_of::<T>()) as GLsizeiptr,
                           data.as_ptr() as *const GLvoid,
                           usage);
            gl::BindBuffer(target, 0);
        }

        BufferBase {
            target,
            buffer
        }
    }

    fn bind(&self) {
        if self.buffer != 0 {
            unsafe {
                gl::BindBuffer(self.target, self.buffer);
            }
        }
    }
}

impl Drop for BufferBase {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.buffer);
        }
    }
}

pub struct VertexBuffer {
    buffer: BufferBase
}

pub struct IndexBuffer {
    buffer: BufferBase,
    idx_type: GLenum,
    size: usize
}

impl VertexBuffer {
    pub fn from<T>(data: &Vec<T>) -> VertexBuffer {
        VertexBuffer {
            buffer: BufferBase::new(gl::ARRAY_BUFFER, gl::STATIC_DRAW, data)
        }
    }

    pub fn bind(&self) {
        self.buffer.bind();
    }

    pub fn release() {
        unsafe { gl::BindBuffer(gl::ARRAY_BUFFER, 0); }
    }
}

impl IndexBuffer {
    pub fn from32(data: &Vec<u32>) -> IndexBuffer {
        IndexBuffer {
            buffer: BufferBase::new(gl::ELEMENT_ARRAY_BUFFER, gl::STATIC_DRAW, data),
            idx_type: gl::UNSIGNED_INT,
            size: data.len()
        }
    }

    pub fn from16(data: &Vec<u16>) -> IndexBuffer {
        IndexBuffer {
            buffer: BufferBase::new(gl::ELEMENT_ARRAY_BUFFER, gl::STATIC_DRAW, data),
            idx_type: gl::UNSIGNED_SHORT,
            size: data.len()
        }
    }

    pub fn bind(&self) {
        self.buffer.bind();
    }

    pub fn release() {
        unsafe { gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0); }
    }

    pub fn draw(&self, mode: GLenum, count: GLsizei, ofs: GLsizei) {
        self.buffer.bind();
        unsafe { gl::DrawElements(mode, count, self.idx_type, ofs as _); }
    }

    pub fn draw_all(&self, mode: GLenum) -> usize {
        self.draw(mode, self.size as _, 0);
        self.size
    }

    pub fn count(&self) -> usize {
        self.size
    }
}
