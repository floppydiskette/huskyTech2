use std::ffi::CString;
use std::{mem, thread};
use std::fmt::Debug;
use std::ops::Index;
use std::ptr::null;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;
use gfx_maths::*;
use gl_matrix::vec3::{dot, multiply, normalize, subtract};
use gl_matrix::vec4::add;
use glad_gl::gl::*;
use crate::helpers::{calculate_model_matrix, calculate_normal_matrix, set_shader_if_not_already};
use crate::ht_renderer;
use crate::renderer::MAX_LIGHTS;
use crate::skeletal_animation::{SkeletalAnimation, SkeletalAnimations};
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
    pub animations: Arc<Mutex<Option<SkeletalAnimations>>>,
    pub animation_delta: Arc<Mutex<Option<Instant>>>,
    atomic_ref_count: Arc<AtomicUsize>,
}

pub struct IntermidiaryMesh {
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
    pub uvs: Vec<f32>,
    pub normals: Vec<f32>,
    pub tangents: Vec<f32>,
    pub joints: Option<Vec<i32>>,
    pub weights: Option<Vec<f32>>,
    pub animations: Option<SkeletalAnimations>,
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
    pub fn new(path: &str, mesh_name: &str, renderer: &mut ht_renderer) -> Result<Mesh, MeshError> {
        // load from gltf
        let (document, buffers, images) = gltf::import(path).map_err(|_| MeshError::MeshNotFound)?;

        // get the mesh
        let mesh = document.meshes().find(|m| m.name() == Some(mesh_name)).ok_or(MeshError::MeshNameNotFound)?;

        let skin = document.skins().next();

        let mut animations = None;

        let shader_index = *renderer.shaders.get("gbuffer_anim").unwrap();

        if let Some(skin) = skin {
            animations = Some(SkeletalAnimations::load_skeleton_stuff(&skin, &mesh, document.animations(), &buffers).expect("Failed to load skeleton stuff"));
        }

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
            num_vertices: indices_array.len(),
            num_indices: indices_array.len(),
            animations: Arc::new(Mutex::new(animations)),
            animation_delta: Arc::new(Mutex::new(Some(Instant::now()))),
            normal_vbo,
            tangent_vbo,
            atomic_ref_count: Arc::new(AtomicUsize::new(1)),
        })
    }

    /// begins loading a mesh on a new thread; once complete, the returned atomic bool will be set to true
    /// and the mesh data will be available in the returned Arc<Mutex<Option<IntermidiaryMesh>>>
    /// then, you must call `Mesh::load_from_intermidiary` to load the texture into opengl
    pub fn new_from_name_asynch_begin(path: &str, mesh_name: &str) -> (Arc<AtomicBool>, Arc<Mutex<Option<IntermidiaryMesh>>>) {
        let finished = Arc::new(AtomicBool::new(false));
        let finished_clone = finished.clone();
        let mesh = Arc::new(Mutex::new(None));
        let mesh_clone = mesh.clone();
        let path_clone = path.to_string();
        let mesh_name_clone = mesh_name.to_string();

        thread::spawn(move || {
            fn on_failure(failure: impl Debug) {
                error!("failed to load mesh: {:?}", failure);
            }

            // load from gltf
            let res = gltf::import(path_clone).map_err(|_| MeshError::MeshNotFound);
            if let Err(e) = res {
                on_failure(e);
                return;
            }
            let (document, buffers, _images) = res.unwrap();

            // get the mesh
            let mesh = document.meshes().find(|m| m.name() == Some(&mesh_name_clone)).ok_or(MeshError::MeshNameNotFound);
            if let Err(e) = mesh {
                on_failure(e);
                return;
            }
            let mesh = mesh.unwrap();

            let skin = document.skins().next();

            let mut animations = None;

            if let Some(skin) = skin {
                let skeletal_stuff = SkeletalAnimations::load_skeleton_stuff(&skin, &mesh, document.animations(), &buffers);
                if let Err(e) = skeletal_stuff {
                    on_failure(e);
                    return;
                }
                let skeletal_stuff = skeletal_stuff.unwrap();
                animations = Some(skeletal_stuff);
            }

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
                let positions = reader.read_positions().ok_or(MeshError::MeshComponentNotFound(MeshComponent::Vertices));
                if let Err(e) = positions {
                    on_failure(e);
                    return;
                }
                let positions = positions.unwrap();
                let positions = positions.collect::<Vec<_>>();

                // get the indices
                let indices = reader.read_indices().ok_or(MeshError::MeshComponentNotFound(MeshComponent::Indices));
                if let Err(e) = indices {
                    on_failure(e);
                    return;
                }
                let indices = indices.unwrap();
                let indices = indices.into_u32().collect::<Vec<_>>();

                // get the texture coordinates
                let tex_coords = reader.read_tex_coords(0).ok_or(MeshError::MeshComponentNotFound(MeshComponent::UvSource));
                if let Err(e) = tex_coords {
                    on_failure(e);
                    return;
                }
                let tex_coords = tex_coords.unwrap();
                let tex_coords = tex_coords.into_f32();
                let tex_coords = tex_coords.collect::<Vec<_>>();

                // get the normals
                let normals = reader.read_normals().ok_or(MeshError::MeshComponentNotFound(MeshComponent::Normals));
                if let Err(e) = normals {
                    on_failure(e);
                    return;
                }
                let normals = normals.unwrap();
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
                    let joints = reader.read_joints(0).ok_or(MeshError::MeshComponentNotFound(MeshComponent::Source));
                    if let Err(e) = joints {
                        on_failure(e);
                        return;
                    }
                    let joints = joints.unwrap();
                    let joints = joints.into_u16();
                    let joints = joints.collect::<Vec<_>>();

                    let weights = reader.read_weights(0).ok_or(MeshError::MeshComponentNotFound(MeshComponent::Source));
                    if let Err(e) = weights {
                        on_failure(e);
                        return;
                    }
                    let weights = weights.unwrap();
                    let weights = weights.into_f32();
                    let weights = weights.collect::<Vec<_>>();

                    joint_array.extend(joints.iter().flat_map(|v| vec![v[0] as i32, v[1] as i32, v[2] as i32, v[3] as i32]));
                    weight_array.extend(weights.iter().flat_map(|v|
                        //vec![v[0], v[1], v[2], v[3]]
                        // change zero weights to 1, 0, 0, 0
                        if v[0] == 0.0 && v[1] == 0.0 && v[2] == 0.0 && v[3] == 0.0 {
                            debug!("zero weight found");
                            vec![1.0, 0.0, 0.0, 0.0]
                        } else {
                            vec![v[0], v[1], v[2], v[3]]
                        }
                    ));
                }
            }

            let mesh = IntermidiaryMesh {
                vertices: vertices_array,
                indices: indices_array,
                uvs: uvs_array,
                normals: normals_array,
                tangents: tangents_array,
                joints: if animations.is_some() { Some(joint_array) } else { None },
                weights: if animations.is_some() { Some(weight_array) } else { None },
                animations,
            };

            mesh_clone.lock().unwrap().replace(mesh);
            finished_clone.store(true, Ordering::SeqCst);
        });

        (finished, mesh)
    }

    pub fn load_from_intermidiary(mesh: Option<IntermidiaryMesh>, renderer: &mut ht_renderer) -> Result<Self, MeshError> {
        if mesh.is_none() {
            return Err(MeshError::FunctionNotImplemented);
        }
        let mesh = mesh.unwrap();
        let vertices_array = mesh.vertices;
        let indices_array = mesh.indices;
        let uvs_array = mesh.uvs;
        let normals_array = mesh.normals;
        let tangents_array = mesh.tangents;
        let joint_array = mesh.joints;
        let weight_array = mesh.weights;
        let animations = mesh.animations;

        let shader_index = *renderer.shaders.get("gbuffer_anim").unwrap();

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
                let joint_array = joint_array.unwrap();
                let weight_array = weight_array.unwrap();

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
            num_vertices: indices_array.len(),
            num_indices: indices_array.len(),
            animations: Arc::new(Mutex::new(animations)),
            animation_delta: Arc::new(Mutex::new(Some(Instant::now()))),
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

    pub fn render(&mut self, renderer: &mut ht_renderer, texture: Option<&Texture>, animations_weights: Option<Vec<(String, f64)>>, shadow_pass: Option<(u8, usize)>) {
        if let Some(shadow_pass) = shadow_pass {
            renderer.setup_shadow_pass(shadow_pass.0);
            self.render_inner(renderer, texture, animations_weights, Some(shadow_pass));
        } else {
            self.render_inner(renderer, texture, animations_weights, None);
        }
    }

    fn render_inner(&mut self, renderer: &mut ht_renderer, texture: Option<&Texture>, animations_weights: Option<Vec<(String, f64)>>, shadow_pass: Option<(u8, usize)>) {
        // load the shader
        let gbuffer_shader = if shadow_pass.is_none() { *renderer.shaders.get("gbuffer_anim").unwrap() } else if shadow_pass.unwrap().0 == 1 {
            *renderer.shaders.get("shadow").unwrap()
        } else {
            *renderer.shaders.get("shadow_mask").unwrap()
        };
        set_shader_if_not_already(renderer, gbuffer_shader);
        let mut shader = renderer.backend.shaders.as_mut().unwrap()[gbuffer_shader].clone();
        unsafe {

            EnableVertexAttribArray(0);
            BindVertexArray(self.vao);
            if let Some(texture) = texture {
                if shadow_pass.is_none() {
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
            }

            if let Some(animations) = self.animations.lock().unwrap().as_mut() {
                let current_time = Instant::now();

                let mut animations_weights = animations_weights;
                let mut using_autoanim = false;
                if let Some(anim_weights) = animations_weights.as_mut() {
                    // if it isn't already there, add the "auto" animation (if it exists)
                    //if !anim_weights.iter().any(|(name, _)| name == "auto") {
                    //    if let Some(auto_anim) = animations.animations.get("auto") {
                    //        anim_weights.push(("auto".to_string(), 0.1));
                    //        using_autoanim = true;
                    //    }
                    //}
                } else {
                    // if there are no animations, add the "auto" animation (if it exists)
                    if let Some(_auto_anim) = animations.animations.get("auto") {
                        animations_weights = Some(vec![("auto".to_string(), 0.1)]);
                        using_autoanim = true;
                    }
                }
                if shadow_pass.is_none() || using_autoanim {
                    let delta = current_time.duration_since(self.animation_delta.lock().unwrap().unwrap_or(current_time)).as_secs_f32();
                    animations.advance_time(delta);
                    self.animation_delta.lock().unwrap().replace(current_time);
                }

                if let Some(animations_weights) = animations_weights {
                    // fill bone matrice uniform (this should already have been done if this is a shadow pass)
                    if shadow_pass.is_none() || using_autoanim {
                        let mut anims_weights = animations_weights.iter().map(
                            |(name, weight)| {
                                (Arc::new(animations.animations.get(name).unwrap().clone()), *weight)
                            }
                        ).collect::<Vec<(Arc<SkeletalAnimation>, f64)>>();

                        for bone in animations.root_bones.clone().iter() {
                            animations.apply_poses_i_stole_this_from_reddit_user_a_carotis_interna(*bone, Mat4::identity(), &anims_weights);
                        }
                    }
                    for (i, transform) in animations.get_joint_matrices().iter().enumerate() {
                        let bone_transforms_c = CString::new(format!("joint_matrix[{}]", i)).unwrap();
                        let bone_transforms_loc = GetUniformLocation(shader.program, bone_transforms_c.as_ptr());
                        UniformMatrix4fv(bone_transforms_loc as i32, 1, FALSE, transform.as_ptr());
                    }
                    let care_about_animation_c = CString::new("care_about_animation").unwrap();
                    let care_about_animation_loc = GetUniformLocation(shader.program, care_about_animation_c.as_ptr());
                    Uniform1i(care_about_animation_loc, 1);
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

            // calculate the normal matrix
            let normal_matrix = calculate_normal_matrix(model_matrix, camera_view);

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

            // send the normal matrix to the shader
            let normal_c = CString::new("u_normal_matrix").unwrap();
            let normal_loc = GetUniformLocation(shader.program, normal_c.as_ptr());
            UniformMatrix3fv(normal_loc, 1, FALSE as GLboolean, normal_matrix.as_ptr());

            // send the camera position to the shader
            let camera_pos_c = CString::new("u_camera_pos").unwrap();
            let camera_pos_loc = GetUniformLocation(shader.program, camera_pos_c.as_ptr());
            Uniform3f(camera_pos_loc,
                      renderer.camera.get_position().x,
                      renderer.camera.get_position().y,
                      renderer.camera.get_position().z);

            if let Some((pass, light_num)) = shadow_pass {
                // send iteration to shader
                let pass_c = CString::new("pass").unwrap();
                let pass_loc = GetUniformLocation(shader.program, pass_c.as_ptr() as *const i8);
                Uniform1i(pass_loc, pass as i32);
                // send iteration to shader
                let pass_c = CString::new("facing_angle").unwrap();
                let pass_loc = GetUniformLocation(shader.program, pass_c.as_ptr() as *const i8);
                Uniform1f(pass_loc, *crate::ui::DEBUG_SHADOW_VOLUME_FACE_ANGLE.lock().unwrap());

                // send the light position to the shadow shader
                let light_pos_c = CString::new("light_pos").unwrap();
                let light_pos = GetUniformLocation(shader.program, light_pos_c.as_ptr());
                let light = renderer.lights.get(light_num as usize);
                if let Some(light) = light {
                    let light_position = light.position;
                    Uniform3f(light_pos, light_position.x, light_position.y, light_position.z);
                }
                // send the scene depth buffer to the shadow shader
                let tex = renderer.backend.framebuffers.gbuffer_info2;
                let depth_c = CString::new("scene_depth").unwrap();
                let depth = GetUniformLocation(shader.program, depth_c.as_ptr());
                ActiveTexture(TEXTURE3);
                BindTexture(TEXTURE_2D, tex as GLuint);
                Uniform1i(depth, 3);

                if pass == 2 {
                    // send back buffer to front buffer shader
                    let backface_depth_c = CString::new("backface_depth").unwrap();
                    let backface_depth_loc = GetUniformLocation(shader.program, backface_depth_c.as_ptr() as *const i8);
                    let texture = renderer.backend.framebuffers.shadow_buffer_tex_scratch as GLuint;
                    ActiveTexture(TEXTURE6);
                    BindTexture(TEXTURE_2D, texture);
                    Uniform1i(backface_depth_loc, 6);
                    // send light number to shader
                    let light_num_c = CString::new("light_num_plus_one").unwrap();
                    let light_num_loc = GetUniformLocation(shader.program, light_num_c.as_ptr() as *const i8);
                    Uniform1i(light_num_loc, light_num as i32 + 1);
                }
            }

            // REMOVE
           // if (self.animations.is_none() && shadow_pass.is_some()) || shadow_pass.is_none() {
                DrawElements(TRIANGLES, self.num_indices as GLsizei, UNSIGNED_INT, null());
            //}

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

            // calculate the normal matrix
            let normal_matrix = calculate_normal_matrix(model_matrix, camera_view);

            // send the mvp matrix to the shader
            let mvp_c = CString::new("u_mvp").unwrap();
            let mvp_loc = GetUniformLocation(shader.program, mvp_c.as_ptr());
            UniformMatrix4fv(mvp_loc, 1, FALSE as GLboolean, mvp.as_ptr());

            // send the normal matrix to the shader
            let normal_c = CString::new("u_normal_matrix").unwrap();
            let normal_loc = GetUniformLocation(shader.program, normal_c.as_ptr());
            UniformMatrix3fv(normal_loc, 1, FALSE as GLboolean, normal_matrix.as_ptr());

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