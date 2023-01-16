use std::ffi::CString;
use std::mem;
use std::ops::Index;
use std::ptr::null;
use std::time::Instant;
use gfx_maths::*;
use glad_gl::gl::*;
use crate::helpers::{calculate_model_matrix, set_shader_if_not_already};
use crate::ht_renderer;
use crate::renderer::MAX_LIGHTS;
use crate::skeletal_animation::SkeletalAnimations;
use crate::textures::Texture;

#[derive(Clone)]
pub struct Mesh {
    pub position: Vec3,
    pub rotation: Quaternion,
    pub scale: Vec3,
    pub vao: GLuint,
    pub vbo: GLuint,
    pub ebo: GLuint,
    pub num_vertices: usize,
    pub num_indices: usize,
    pub uvbo: GLuint,
    pub normal_vbo: GLuint,
    pub animations: Option<SkeletalAnimations>,
    pub animation_delta: Option<Instant>,
}

#[derive(Debug)]
pub enum MeshComponent {
    Mesh,
    Tris,
    VerticesMap,
    Vertices,
    SourceMap,
    Source,
    UvSource,
    SourceArray,
    UvSourceArray,
    Indices,
}

#[derive(Debug)]
pub enum MeshError {
    FunctionNotImplemented,
    MeshNotFound,
    MeshNameNotFound,
    MeshComponentNotFound(MeshComponent),
    UnsupportedArrayType,
}

impl Mesh {
    pub fn new(path: &str, mesh_name: &str, shader_index: usize, renderer: &mut ht_renderer) -> Result<Mesh, MeshError> {
        // load from gltf
        let (document, buffers, images) = gltf::import(path).map_err(|_| MeshError::MeshNotFound)?;

        // get the mesh
        let mesh = document.meshes().find(|m| m.name() == Some(mesh_name)).ok_or(MeshError::MeshNameNotFound)?;

        let skin = document.skins().next();

        let mut animations = None;

        let mut shader_index = shader_index;

        if let Some(skin) = skin {
            animations = Some(SkeletalAnimations::load_skeleton_stuff(&skin, &mesh, document.animations(), &buffers).expect("Failed to load skeleton stuff"));
        }

        let gbuffer_shader = *renderer.shaders.get("gbuffer_anim").unwrap();
        shader_index = gbuffer_shader;

        // for each primitive in the mesh
        let mut vertices_array = Vec::new();
        let mut indices_array = Vec::new();
        let mut uvs_array = Vec::new();
        let mut normals_array = Vec::new();
        let mut joint_array = Vec::new();
        let mut weight_array = Vec::new();
        for primitive in mesh.primitives() {
            // get the vertex positions
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let positions = reader.read_positions().ok_or(MeshError::MeshComponentNotFound(MeshComponent::Vertices))?;
            let positions = positions.collect::<Vec<_>>();

            // get the indices
            let indices = reader.read_indices().ok_or(MeshError::MeshComponentNotFound(MeshComponent::Indices))?;
            let indices = indices.into_u32().collect::<Vec<_>>();

            // get the texture coordinates
            let tex_coords = reader.read_tex_coords(0).ok_or(MeshError::MeshComponentNotFound(MeshComponent::UvSource))?;
            let tex_coords = tex_coords.into_f32();
            let tex_coords = tex_coords.collect::<Vec<_>>();

            // get the normals
            let normals = reader.read_normals().ok_or(MeshError::MeshComponentNotFound(MeshComponent::SourceMap))?;
            let normals = normals.collect::<Vec<_>>();

            // add the vertices (with each grouping of three f32s as three separate f32s)
            vertices_array.extend(positions.iter().flat_map(|v| vec![v[0], v[1], v[2]]));

            // add the indices
            indices_array.extend_from_slice(&indices);

            // add the uvs (with each grouping of two f32s as two separate f32s)
            uvs_array.extend(tex_coords.iter().flat_map(|v| vec![v[0], v[1]]));

            // add the normals (with each grouping of three f32s as three separate f32s)
            normals_array.extend(normals.iter().flat_map(|v| vec![v[0], v[1], v[2]]));

            // get the bone ids

            if let Some(animations) = animations.clone() {
                let joints = reader.read_joints(0).ok_or(MeshError::MeshComponentNotFound(MeshComponent::Source))?;
                let joints = joints.into_u16();
                let joints = joints.collect::<Vec<_>>();

                let weights = reader.read_weights(0).ok_or(MeshError::MeshComponentNotFound(MeshComponent::Source))?;
                let weights = weights.into_f32();
                let weights = weights.collect::<Vec<_>>();

                joint_array.extend(joints.iter().flat_map(|v| vec![v[0] as i32, v[1] as i32, v[2] as i32, v[3] as i32]));
                weight_array.extend(weights.iter().flat_map(|v| vec![v[0], v[1], v[2], v[3]]));
            }
        }

        // get the u32 data from the mesh
        let mut vbo = 0 as GLuint;
        let mut vao = 0 as GLuint;
        let mut ebo = 0 as GLuint;
        let mut uvbo= 0 as GLuint;
        let mut normal_vbo = 0 as GLuint;
        let mut joint_bo = 0 as GLuint;
        let mut weight_bo = 0 as GLuint;
        unsafe {
            // set the shader program
            set_shader_if_not_already(renderer, shader_index);

            GenVertexArrays(1, &mut vao);
            BindVertexArray(vao);
            GenBuffers(1, &mut vbo);
            BindBuffer(ARRAY_BUFFER, vbo);
            BufferData(ARRAY_BUFFER, (vertices_array.len() * mem::size_of::<GLfloat>()) as GLsizeiptr, vertices_array.as_ptr() as *const GLvoid, STATIC_DRAW);
            // vertex positions for vertex shader
            let in_pos_c = CString::new("in_pos").unwrap();
            let pos = GetAttribLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, in_pos_c.as_ptr());
            VertexAttribPointer(pos as GLuint, 3, FLOAT, FALSE as GLboolean, 0, null());
            EnableVertexAttribArray(0);

            // uvs
            GenBuffers(1, &mut uvbo);
            BindBuffer(ARRAY_BUFFER, uvbo);
            BufferData(ARRAY_BUFFER, (uvs_array.len() * mem::size_of::<GLfloat>()) as GLsizeiptr, uvs_array.as_ptr() as *const GLvoid, STATIC_DRAW);
            // vertex uvs for fragment shader
            let in_uv_c = CString::new("in_uv").unwrap();
            let uv = GetAttribLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, in_uv_c.as_ptr());
            VertexAttribPointer(uv as GLuint, 2, FLOAT, FALSE as GLboolean, 0, null());
            EnableVertexAttribArray(1);

            // normals
            GenBuffers(1, &mut normal_vbo);
            BindBuffer(ARRAY_BUFFER, normal_vbo);
            BufferData(ARRAY_BUFFER, (normals_array.len() * mem::size_of::<GLfloat>()) as GLsizeiptr, normals_array.as_ptr() as *const GLvoid, STATIC_DRAW);
            // vertex normals for fragment shader
            let in_normal_c = CString::new("in_normal").unwrap();
            let normal = GetAttribLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, in_normal_c.as_ptr());
            VertexAttribPointer(normal as GLuint, 3, FLOAT, FALSE as GLboolean, 0, null());
            EnableVertexAttribArray(2);

            if animations.is_some() {
                // joint array
                GenBuffers(1, &mut joint_bo);
                BindBuffer(ARRAY_BUFFER, joint_bo);
                BufferData(ARRAY_BUFFER, (joint_array.len() * mem::size_of::<GLint>()) as GLsizeiptr, joint_array.as_ptr() as *const GLvoid, STATIC_DRAW);
                let in_joint_c = CString::new("a_joint").unwrap();
                let joint = GetAttribLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, in_joint_c.as_ptr());
                VertexAttribPointer(joint as GLuint, 4, FLOAT, FALSE as GLboolean, 0, null());
                EnableVertexAttribArray(5);

                // weight array
                GenBuffers(1, &mut weight_bo);
                BindBuffer(ARRAY_BUFFER, weight_bo);
                BufferData(ARRAY_BUFFER, (weight_array.len() * mem::size_of::<GLfloat>()) as GLsizeiptr, weight_array.as_ptr() as *const GLvoid, STATIC_DRAW);
                let in_weight_c = CString::new("a_weight").unwrap();
                let weight = GetAttribLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, in_weight_c.as_ptr());
                VertexAttribPointer(weight as GLuint, 4, FLOAT, FALSE as GLboolean, 0, null());
                EnableVertexAttribArray(6);
            }


            // now the indices
            GenBuffers(1, &mut ebo);
            BindBuffer(ELEMENT_ARRAY_BUFFER, ebo);
            BufferData(ELEMENT_ARRAY_BUFFER, (indices_array.len() * mem::size_of::<GLuint>()) as GLsizeiptr, indices_array.as_ptr() as *const GLvoid, STATIC_DRAW);
        }

        Ok(Mesh {
            position: Vec3::new(0.0, 0.0, 0.0),
            rotation: Quaternion::identity(),
            scale: Vec3::new(1.0, 1.0, 1.0),
            vbo,
            vao,
            ebo,
            uvbo,
            num_vertices: indices_array.len() as usize,
            num_indices: indices_array.len() as usize,
            animations,
            animation_delta: Some(Instant::now()),
            normal_vbo,
        })
    }

    pub fn render(&mut self, renderer: &mut ht_renderer, texture: Option<&Texture>) {
        // load the shader
        let gbuffer_shader = *renderer.shaders.get("gbuffer_anim").unwrap();
        set_shader_if_not_already(renderer, gbuffer_shader);
        let mut shader = renderer.backend.shaders.as_mut().unwrap()[gbuffer_shader].clone();
        unsafe {

            EnableVertexAttribArray(0);
            BindVertexArray(self.vao);
            if let Some(texture) = texture {
                let gbuffer_shader = *renderer.shaders.get("gbuffer_anim").unwrap();
                set_shader_if_not_already(renderer, gbuffer_shader);
                shader = renderer.backend.shaders.as_mut().unwrap()[gbuffer_shader].clone();
                // send the material struct to the shader
                let material = texture.material;
                let diffuse_c = CString::new("diffuse").unwrap();
                let material_diffuse = GetUniformLocation(shader.program, diffuse_c.as_ptr());
                let roughness_c = CString::new("specular").unwrap();
                let material_roughness = GetUniformLocation(shader.program, roughness_c.as_ptr());
                let normal_c = CString::new("normalmap").unwrap();
                let material_normal = GetUniformLocation(shader.program, normal_c.as_ptr());

                // load textures
                ActiveTexture(TEXTURE0);
                BindTexture(TEXTURE_2D, material.diffuse_texture);
                Uniform1i(material_diffuse, 0);
                ActiveTexture(TEXTURE1);
                BindTexture(TEXTURE_2D, material.roughness_texture);
                Uniform1i(material_roughness, 1);
                ActiveTexture(TEXTURE2);
                BindTexture(TEXTURE_2D, material.normal_texture);
                Uniform1i(material_normal, 2);

            }

            if let Some(animations) = self.animations.as_mut() {
                if let Some(mut animation) = animations.animations.get("auto").cloned() {
                    let current_time = Instant::now();
                    let delta = current_time.duration_since(self.animation_delta.unwrap_or(current_time)).as_secs_f32();

                    animation.advance_time(delta);

                    // fill bone matrice uniform
                    for bone in animations.root_bones.clone().iter() {
                        animations.apply_poses_i_stole_this_from_reddit_user_a_carotis_interna(*bone, Mat4::identity(), &animation);
                    }
                    for (i, transform) in animation.get_joint_matrices(animations).iter().enumerate() {
                        let bone_transforms_c = CString::new(format!("joint_matrix[{}]", i)).unwrap();
                        let bone_transforms_loc = GetUniformLocation(shader.program, bone_transforms_c.as_ptr());
                        UniformMatrix4fv(bone_transforms_loc as i32, 1, FALSE, transform.as_ptr());
                    }
                    let care_about_animation_c = CString::new("care_about_animation").unwrap();
                    let care_about_animation_loc = GetUniformLocation(shader.program, care_about_animation_c.as_ptr());
                    Uniform1i(care_about_animation_loc, 1);

                    self.animation_delta = Some(current_time);
                    *animations.animations.get_mut("auto").unwrap() = animation;
                }
            } else {
                let care_about_animation_c = CString::new("care_about_animation").unwrap();
                let care_about_animation_loc = GetUniformLocation(shader.program, care_about_animation_c.as_ptr());
                Uniform1i(care_about_animation_loc, 0);
            }

            // transformation time!
            let camera_projection = renderer.camera.get_projection();
            let camera_view = renderer.camera.get_view();

            // calculate the model matrix
            let model_matrix = calculate_model_matrix(self.position, self.rotation, self.scale);

            // calculate the mvp matrix
            let mvp = camera_projection * camera_view * model_matrix;

            // send the mvp matrix to the shader
            let mvp_c = CString::new("u_mvp").unwrap();
            let mvp_loc = GetUniformLocation(shader.program, mvp_c.as_ptr());
            UniformMatrix4fv(mvp_loc, 1, FALSE as GLboolean, mvp.as_ptr());

            // send the view and projection matrices to the shader
            let view_c = CString::new("u_view").unwrap();
            let view_loc = GetUniformLocation(shader.program, view_c.as_ptr());
            UniformMatrix4fv(view_loc, 1, FALSE as GLboolean, camera_view.as_ptr());

            let projection_c = CString::new("u_projection").unwrap();
            let projection_loc = GetUniformLocation(shader.program, projection_c.as_ptr());
            UniformMatrix4fv(projection_loc, 1, FALSE as GLboolean, camera_projection.as_ptr());

            // send the model matrix to the shader
            let model_c = CString::new("u_model").unwrap();
            let model_loc = GetUniformLocation(shader.program, model_c.as_ptr());
            UniformMatrix4fv(model_loc, 1, FALSE as GLboolean, model_matrix.as_ptr());

            // send the camera position to the shader
            //let camera_pos_c = CString::new("u_camera_pos").unwrap();
            //let camera_pos_loc = GetUniformLocation(shader.program, camera_pos_c.as_ptr());
            //Uniform3f(camera_pos_loc,
            //            renderer.camera.get_position().x,
            //            renderer.camera.get_position().y,
            //            renderer.camera.get_position().z);

            DrawElements(TRIANGLES, self.num_indices as GLsizei, UNSIGNED_INT, null());

            let care_about_animation_c = CString::new("care_about_animation").unwrap();
            let care_about_animation_loc = GetUniformLocation(shader.program, care_about_animation_c.as_ptr());
            Uniform1i(care_about_animation_loc, 0);

            // print opengl errors
            let mut error = GetError();
            while error != NO_ERROR {
                error!("OpenGL error while rendering: {}", error);
                error = GetError();
            }
        }
    }

    pub fn render_basic_lines(&self, renderer: &mut ht_renderer, shader_index: usize) {
        // load the shader
        set_shader_if_not_already(renderer, shader_index);
        let shader = renderer.backend.shaders.as_mut().unwrap()[shader_index].clone();
        unsafe {

            EnableVertexAttribArray(0);
            BindVertexArray(self.vao);

            // transformation time!
            let camera_projection = renderer.camera.get_projection();
            let camera_view = renderer.camera.get_view();

            // calculate the model matrix
            let model_matrix = calculate_model_matrix(self.position, self.rotation, self.scale);

            // calculate the mvp matrix
            let mvp = camera_projection * camera_view * model_matrix;

            // send the mvp matrix to the shader
            let mvp_c = CString::new("u_mvp").unwrap();
            let mvp_loc = GetUniformLocation(shader.program, mvp_c.as_ptr());
            UniformMatrix4fv(mvp_loc, 1, FALSE as GLboolean, mvp.as_ptr());

            // send the model matrix to the shader
            /*let model_c = CString::new("u_model").unwrap();
            let model_loc = glGetUniformLocation(shader.program, model_c.as_ptr());
            glUniformMatrix4fv(model_loc, 1, FALSE as GLboolean, model_matrix.as_ptr());

            // send the camera position to the shader
            let camera_pos_c = CString::new("u_camera_pos").unwrap();
            let camera_pos_loc = glGetUniformLocation(shader.program, camera_pos_c.as_ptr());
            glUniform3f(camera_pos_loc,
                        renderer.camera.get_position().x,
                        renderer.camera.get_position().y,
                        renderer.camera.get_position().z);

             */

            DrawElements(LINES, self.num_indices as GLsizei, UNSIGNED_INT, null());

            // print opengl errors
            let mut error = GetError();
            while error != NO_ERROR {
                error!("OpenGL error while rendering: {}", error);
                error = GetError();
            }
        }
    }
}