use std::ffi::CString;
use dae_parser::Document;
use gfx_maths::*;
use libsex::bindings::*;
use crate::camera::Camera;
use crate::ht_renderer;
use crate::meshes::Mesh;
use crate::textures::UiTexture;

#[derive(Clone, Copy)]
pub struct UiMesh {
    pub position: Vec2,
    pub rotation: Quaternion,
    pub scale: Vec2,
    pub texture: Option<UiTexture>,
    pub vao: GLuint,
    pub vbo: GLuint,
    pub ebo: GLuint,
    pub uvbo: GLuint,
    pub num_vertices: usize,
    pub num_indices: usize,
}

impl UiMesh {
    #[cfg(feature = "glfw")]
    pub fn new_master(renderer: &mut ht_renderer, shader_index: usize) -> Result<UiMesh, String> {
        // create the mesh
        let vertices: [f32; 12] = [
            -1.0, -1.0, 0.0,
            1.0, -1.0, 0.0,
            1.0, 1.0, 0.0,
            -1.0, 1.0, 0.0,
        ];
        let indices: [u32; 6] = [
            0, 1, 2,
            0, 2, 3,
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
            if renderer.backend.current_shader != Some(shader_index) {
                unsafe {
                    glUseProgram(renderer.backend.shaders.as_mut().unwrap()[shader_index].program);
                    renderer.backend.current_shader = Some(shader_index);
                }
            }

            // positions, indices, and uvs
            glGenVertexArrays(1, &mut vao);
            glBindVertexArray(vao);
            glGenBuffers(1, &mut vbo);
            glBindBuffer(GL_ARRAY_BUFFER, vbo);
            glBufferData(GL_ARRAY_BUFFER, (vertices.len() * std::mem::size_of::<f32>()) as GLsizeiptr, vertices.as_ptr() as *const GLvoid, GL_STATIC_DRAW);

            // position attribute
            let pos = glGetAttribLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("in_pos").unwrap().as_ptr());
            glVertexAttribPointer(pos as GLuint, 3, GL_FLOAT, GL_FALSE as GLboolean, 0, std::ptr::null());
            glEnableVertexAttribArray(0);

            // uvs
            glGenBuffers(1, &mut uvbo);
            glBindBuffer(GL_ARRAY_BUFFER, uvbo);
            glBufferData(GL_ARRAY_BUFFER, (uvs.len() * std::mem::size_of::<f32>()) as GLsizeiptr, uvs.as_ptr() as *const GLvoid, GL_STATIC_DRAW);

            // uv attribute
            let uv = glGetAttribLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("in_uv").unwrap().as_ptr());
            glVertexAttribPointer(uv as GLuint, 2, GL_FLOAT, GL_FALSE as GLboolean, 0, std::ptr::null());
            glEnableVertexAttribArray(1);

            // indices
            glGenBuffers(1, &mut ebo);
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, ebo);
            glBufferData(GL_ELEMENT_ARRAY_BUFFER, (indices.len() * std::mem::size_of::<u32>()) as GLsizeiptr, indices.as_ptr() as *const GLvoid, GL_STATIC_DRAW);

        }

        Ok(UiMesh {
            position: Vec2::new(0.0, 0.0),
            rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            scale: Vec2::new(1.0, 1.0),
            texture: Option::None,
            vao,
            vbo,
            ebo,
            uvbo,
            num_vertices: vertices.len(),
            num_indices: indices.len(),
        })
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
            vao: master.vao,
            vbo: master.vbo,
            ebo: master.ebo,
            uvbo: master.uvbo,
            num_vertices: master.num_vertices,
            num_indices: master.num_indices,
        })
    }

    #[cfg(feature = "glfw")]
    pub fn render_at(&mut self, mut master: UiMesh, renderer: &mut ht_renderer, shader_index: usize) {
        let mut master = master.clone();

        if renderer.backend.current_shader != Some(shader_index) {
            unsafe {
                glUseProgram(renderer.backend.shaders.as_mut().unwrap()[shader_index].program);
                renderer.backend.current_shader = Some(shader_index);
            }
        }
        unsafe {
            glEnableVertexAttribArray(0);
            glBindVertexArray(master.vao);
            glActiveTexture(GL_TEXTURE0);
            glBindTexture(GL_TEXTURE_2D, self.texture.unwrap().diffuse_texture);
            glUniform1i(glGetUniformLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("u_texture").unwrap().as_ptr()), 0);

            // transformation time!
            let fake_camera = Camera::new(renderer.window_size, 45.0, 0.1, 100.0);
            let camera_projection = fake_camera.get_projection();
            let camera_view = fake_camera.get_view();

            // calculate the model matrix
            let fake_coords = screen_coords_to_gl_coords(self.position, self.scale, renderer.window_size);
            let model_matrix = calculate_model_matrix(fake_coords.0, self.rotation, fake_coords.1);

            // calculate the mvp matrix
            let mvp = camera_projection * camera_view * model_matrix;

            // send the mvp matrix to the shader
            let mvp_loc = glGetUniformLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("u_mvp").unwrap().as_ptr());
            glUniformMatrix4fv(mvp_loc, 1, GL_FALSE as GLboolean, model_matrix.as_ptr());

            glDrawElements(GL_TRIANGLES, master.num_indices as GLsizei, GL_UNSIGNED_INT, std::ptr::null());
            glDisableVertexAttribArray(0);
        }
    }
}

// converts screen coordinates to gl coordinates
pub fn screen_coords_to_gl_coords(position: Vec2, scale: Vec2, window_size: Vec2) -> (Vec3, Vec3) {
    let mut x =  (position.x / window_size.x) - 1.0;
    let mut y = (-position.y / window_size.y) + 1.0;
    let z = 0.0;
    let w = scale.x / window_size.x;
    let h = scale.y / window_size.y;
    let d = 1.0;
    x += w;
    y -= h;

    (Vec3::new(x, y, z), Vec3::new(w, h, d))
}

fn calculate_model_matrix(position: Vec3, rotation: Quaternion, scale: Vec3) -> Mat4 {
    let mut model_matrix = Mat4::identity();
    model_matrix = model_matrix * Mat4::translate(position);
    model_matrix = model_matrix * Mat4::rotate(rotation);
    model_matrix = model_matrix * Mat4::scale(scale);
    model_matrix
}