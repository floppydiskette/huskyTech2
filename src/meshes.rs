use std::ffi::CString;
use std::mem;
use std::ops::Index;
use std::ptr::null;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Instant;
use gfx_maths::*;
use gl_matrix::vec3::{dot, multiply, normalize, subtract};
use gl_matrix::vec4::add;
use glad_gl::gl::*;
use crate::helpers::{calculate_model_matrix, set_shader_if_not_already};
use crate::ht_renderer;
use crate::renderer::MAX_LIGHTS;
use crate::skeletal_animation::SkeletalAnimations;
use crate::textures::Texture;

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
    pub tangent_vbo: GLuint,
    pub animations: Option<SkeletalAnimations>,
    pub animation_delta: Option<Instant>,
    atomic_ref_count: Arc<AtomicUsize>,
}

impl Clone for Mesh {
    fn clone(&self) -> Self {
        self.atomic_ref_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Mesh {
            position: self.position,
            rotation: self.rotation,
            scale: self.scale,
            vao: self.vao,
            vbo: self.vbo,
            ebo: self.ebo,
            num_vertices: self.num_vertices,
            num_indices: self.num_indices,
            uvbo: self.uvbo,
            normal_vbo: self.normal_vbo,
            tangent_vbo: self.tangent_vbo,
            animations: self.animations.clone(),
            animation_delta: self.animation_delta.clone(),
            atomic_ref_count: self.atomic_ref_count.clone(),
        }
    }
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
    Tangents,
    Normals,
}

#[derive(Debug)]
pub enum MeshError {
    FunctionNotImplemented,
    MeshNotFound,
    MeshNameNotFound,
    MeshComponentNotFound(MeshComponent),
    UnsupportedArrayType,
}

fn calculate_tangents(positions: &Vec<[f32; 3]>, uvs: &[[f32; 2]], normals: &Vec<[f32; 3]>, indices: &Vec<u32>) -> Vec<[f32; 4]> {
    let mut tangents = vec![[0.0, 0.0, 0.0, 0.0]; positions.len() * 2];
    let mut i = 0;
    while i < indices.len() {
        let i0 = indices[i];
        let i1 = indices[i + 1];
        let i2 = indices[i + 2];
        let p0 = positions[i0 as usize];
        let p1 = positions[i1 as usize];
        let p2 = positions[i2 as usize];
        let uv0 = uvs[i0 as usize];
        let uv1 = uvs[i1 as usize];
        let uv2 = uvs[i2 as usize];

        let edge1 = subtract(&mut gl_matrix::common::Vec3::default(), &p1, &p0);
        let edge2 = subtract(&mut gl_matrix::common::Vec3::default(), &p2, &p0);
        let x1 = uv1[0] - uv0[0];
        let x2 = uv2[0] - uv0[0];
        let y1 = uv1[1] - uv0[1];
        let y2 = uv2[1] - uv0[1];

        let r = 1.0 / (x1 * y2 - x2 * y1);
        let tx = (y2 * edge1[0] - y1 * edge2[0]) * r;
        let ty = (y2 * edge1[1] - y1 * edge2[1]) * r;
        let tz = (y2 * edge1[2] - y1 * edge2[2]) * r;
        let tangent = [tx, ty, tz, 0.0];

        let t0 = add(&mut gl_matrix::common::Vec4::default(), &tangents[i0 as usize], &tangent);
        let t1 = add(&mut gl_matrix::common::Vec4::default(), &tangents[i1 as usize], &tangent);
        let t2 = add(&mut gl_matrix::common::Vec4::default(), &tangents[i2 as usize], &tangent);
        tangents[i0 as usize] = t0;
        tangents[i1 as usize] = t1;
        tangents[i2 as usize] = t2;

        i += 3;
    }

    let mut i = 0;
    while i < positions.len() {
        let t = tangents[i];
        let n = normals[i];
        let mut tangentXYZ = [t[0], t[1], t[2]];
        let mut tangentXYZN = normalize(&mut gl_matrix::common::Vec3::default(), &tangentXYZ);
        let mut dotTN = dot(&tangentXYZN, &n);
        let mut tangent = [tangentXYZN[0], tangentXYZN[1], tangentXYZN[2], dotTN];
        tangents[i] = tangent;
        i += 1;
    }

    tangents
}

impl Drop for Mesh {
    fn drop(&mut self) {
        if self.atomic_ref_count.fetch_sub(1, std::sync::atomic::Ordering::AcqRel) == 1 {
            self.unload();
        }
    }
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
        let mut tangents_array = Vec::new();
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
            let normals = reader.read_normals().ok_or(MeshError::MeshComponentNotFound(MeshComponent::Normals))?;
            let normals = normals.collect::<Vec<_>>();

            // get the tangents
            let tangents = calculate_tangents(&positions, &tex_coords, &normals, &indices);
            tangents_array.extend(tangents.iter().flat_map(|v| vec![v[0], v[1], v[2], v[3]]));

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
        let mut tangent_vbo = 0 as GLuint;
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

            // tangents
            GenBuffers(1, &mut tangent_vbo);
            BindBuffer(ARRAY_BUFFER, tangent_vbo);
            BufferData(ARRAY_BUFFER, (tangents_array.len() * mem::size_of::<GLfloat>()) as GLsizeiptr, tangents_array.as_ptr() as *const GLvoid, STATIC_DRAW);
            // vertex tangents for fragment shader
            let in_tangent_c = CString::new("in_tangent").unwrap();
            let tangent = GetAttribLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, in_tangent_c.as_ptr());
            VertexAttribPointer(tangent as GLuint, 4, FLOAT, FALSE as GLboolean, 0, null());
            EnableVertexAttribArray(7);

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
            tangent_vbo,
            atomic_ref_count: Arc::new(AtomicUsize::new(1)),
        })
    }

    // removes the mesh from the gpu
    pub fn unload(&mut self) {
        unsafe {
            DeleteBuffers(1, &self.vbo);
            DeleteBuffers(1, &self.uvbo);
            DeleteBuffers(1, &self.ebo);
            DeleteBuffers(1, &self.normal_vbo);
            DeleteBuffers(1, &self.tangent_vbo);
            DeleteVertexArrays(1, &self.vao);
        }
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