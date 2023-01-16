use std::ffi::CString;
use std::sync::Mutex;
use std::sync::Arc;
use gfx_maths::*;
use glad_gl::gl::*;
use crate::camera::Camera;
use crate::helpers::{calculate_model_matrix, set_shader_if_not_already};
use crate::ht_renderer;
use crate::meshes::Mesh;
use crate::textures::UiTexture;

#[derive(Clone, Copy)]
pub struct UiMesh {
    pub position: Vec2,
    pub rotation: Quaternion,
    pub scale: Vec2,
    pub texture: Option<UiTexture>,
    pub opacity: f32,
    pub vao: GLuint,
    pub vbo: GLuint,
    pub ebo: GLuint,
    pub uvbo: GLuint,
    pub num_vertices: usize,
    pub num_indices: usize,
}

lazy_static!{
    pub static ref UI_MASTER: Arc<Mutex<Option<UiMesh>>> = Arc::new(Mutex::new(None));
}

impl UiMesh {
    #[cfg(feature = "glfw")]
    pub fn new_master(renderer: &mut ht_renderer, shader_index: usize) -> Result<Arc<Mutex<Option<UiMesh>>>, String> {
        // create the mesh
        let vertices: [f32; 12] = [
            -1.0, -1.0, 0.0,
            1.0, -1.0, 0.0,
            1.0, 1.0, 0.0,
            -1.0, 1.0, 0.0,
        ];
        let indices: [u32; 6] = [
            2, 1, 0,
            0, 3, 2,
        ];
        // uvs are vertically upside down to fix the texture
        let uvs: [f32; 8] = [
            0.0, 1.0,
            1.0, 1.0,
            1.0, 0.0,
            0.0, 0.0,
        ];

        let mut vao: GLuint = 0;
        let mut vbo: GLuint = 0;
        let mut ebo: GLuint = 0;
        let mut uvbo: GLuint = 0;

        unsafe {
            set_shader_if_not_already(renderer, shader_index);

            // positions, indices, and uvs
            GenVertexArrays(1, &mut vao);
            BindVertexArray(vao);
            GenBuffers(1, &mut vbo);
            BindBuffer(ARRAY_BUFFER, vbo);
            BufferData(ARRAY_BUFFER, (vertices.len() * std::mem::size_of::<f32>()) as GLsizeiptr, vertices.as_ptr() as *const GLvoid, STATIC_DRAW);

            // position attribute
            let pos = GetAttribLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("in_pos").unwrap().as_ptr());
            VertexAttribPointer(pos as GLuint, 3, FLOAT, FALSE as GLboolean, 0, std::ptr::null());
            EnableVertexAttribArray(0);

            // uvs
            GenBuffers(1, &mut uvbo);
            BindBuffer(ARRAY_BUFFER, uvbo);
            BufferData(ARRAY_BUFFER, (uvs.len() * std::mem::size_of::<f32>()) as GLsizeiptr, uvs.as_ptr() as *const GLvoid, STATIC_DRAW);

            // uv attribute
            let uv = GetAttribLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("in_uv").unwrap().as_ptr());
            VertexAttribPointer(uv as GLuint, 2, FLOAT, FALSE as GLboolean, 0, std::ptr::null());
            EnableVertexAttribArray(1);

            // indices
            GenBuffers(1, &mut ebo);
            BindBuffer(ELEMENT_ARRAY_BUFFER, ebo);
            BufferData(ELEMENT_ARRAY_BUFFER, (indices.len() * std::mem::size_of::<u32>()) as GLsizeiptr, indices.as_ptr() as *const GLvoid, STATIC_DRAW);

        }

        let master = UiMesh {
            position: Vec2::new(0.0, 0.0),
            rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            scale: Vec2::new(1.0, 1.0),
            texture: Option::None,
            opacity: 1.0,
            vao,
            vbo,
            ebo,
            uvbo,
            num_vertices: vertices.len(),
            num_indices: indices.len(),
        };

        *UI_MASTER.lock().unwrap() = Some(master.clone());

        Ok(UI_MASTER.clone())
    }

    #[cfg(feature = "glfw")]
    pub fn new_element_from_name(name: &str, master: &UiMesh, renderer: &mut ht_renderer, shader_index: usize) -> Result<UiMesh, String> {
        // load the texture
        let texture = UiTexture::new_from_name(name.to_string())?;
        Ok(UiMesh {
            position: Vec2::new(0.0, 0.0),
            rotation: Quaternion::identity(),
            scale: Vec2::new(128.0, 128.0),
            texture: Option::Some(texture),
            opacity: 1.0,
            vao: master.vao,
            vbo: master.vbo,
            ebo: master.ebo,
            uvbo: master.uvbo,
            num_vertices: master.num_vertices,
            num_indices: master.num_indices,
        })
    }

    #[cfg(feature = "glfw")]
    pub fn new_element_from_data_assume_master_init(data: &[u8], dimensions: (u32, u32)) -> Result<UiMesh, String> {
        let texture = UiTexture::new_from_rgba_bytes(data, dimensions)?;
        let master = UI_MASTER.lock().unwrap().unwrap().clone();
        Ok(UiMesh {
            position: Vec2::new(0.0, 0.0),
            rotation: Quaternion::identity(),
            scale: Vec2::new(128.0, 128.0),
            texture: Option::Some(texture),
            opacity: 1.0,
            vao: master.vao,
            vbo: master.vbo,
            ebo: master.ebo,
            uvbo: master.uvbo,
            num_vertices: master.num_vertices,
            num_indices: master.num_indices,
        })
    }

    #[cfg(feature = "glfw")]
    pub fn render_at(&self, master: UiMesh, renderer: &mut ht_renderer) {
        let master = master;

        let gbuffer_shader = *renderer.shaders.get("gbuffer").unwrap();
        set_shader_if_not_already(renderer, gbuffer_shader);
        let shader = renderer.backend.shaders.as_mut().unwrap()[gbuffer_shader].clone();

        unsafe {
            // disable culling and depth testing
            Disable(CULL_FACE);
            Disable(DEPTH_TEST);

            EnableVertexAttribArray(0);
            BindVertexArray(master.vao);
            ActiveTexture(TEXTURE0);
            BindTexture(TEXTURE_2D, self.texture.unwrap().diffuse_texture);
            let texture_c = CString::new("diffuse").unwrap();
            Uniform1i(GetUniformLocation(shader.program, texture_c.as_ptr() as *const GLchar), 0);
            if self.opacity != 1.0 {
                let opacity_c = CString::new("opacity").unwrap();
                Uniform1f(GetUniformLocation(shader.program, opacity_c.as_ptr()), self.opacity);
            }
            let unlit_c = CString::new("unlit").unwrap();
            Uniform1i(GetUniformLocation(shader.program, unlit_c.as_ptr() as *const GLchar), 1);

            // transformation time!
            // calculate the model matrix
            let fake_coords = screen_coords_to_gl_coords(self.position, self.scale, renderer.window_size);
            let model_matrix = calculate_model_matrix(fake_coords.0, self.rotation, fake_coords.1);

            // send the mvp matrix to the shader
            let mvp_c = CString::new("u_mvp").unwrap();
            let mvp_loc = GetUniformLocation(shader.program, mvp_c.as_ptr());
            UniformMatrix4fv(mvp_loc, 1, FALSE as GLboolean, model_matrix.as_ptr());

            DrawElements(TRIANGLES, master.num_indices as GLsizei, UNSIGNED_INT, std::ptr::null());

            if self.opacity != 1.0 {
                let opacity_c = CString::new("opacity").unwrap();
                Uniform1f(GetUniformLocation(shader.program, opacity_c.as_ptr()), 1.0);
            }
            let unlit_c = CString::new("unlit").unwrap();
            Uniform1i(GetUniformLocation(shader.program, unlit_c.as_ptr() as *const GLchar), 0);

            // re-enable culling and depth testing
            Enable(CULL_FACE);
            Enable(DEPTH_TEST);

            // print opengl errors
            let mut error = GetError();
            while error != NO_ERROR {
                error!("OpenGL error while rendering uimesh: {}", error);
                error = GetError();
            }
        }
    }
}

// converts screen coordinates to gl coordinates
pub fn screen_coords_to_gl_coords(position: Vec2, scale: Vec2, window_size: Vec2) -> (Vec3, Vec3) {
    let mut x =  (position.x / window_size.x) * 2.0 - 1.0;
    let mut y = (-position.y / window_size.y) * 2.0 + 1.0;
    let z = 1.0;
    let w = (scale.x / window_size.x);
    let h = (scale.y / window_size.y);
    let d = 1.0;
    x += w;
    y -= h;

    (Vec3::new(x, y, z), Vec3::new(w, h, d))
}