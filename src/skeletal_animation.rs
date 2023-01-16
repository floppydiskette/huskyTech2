use std::collections::{BTreeMap, HashSet, VecDeque};
use std::ptr::slice_from_raw_parts;
use gfx_maths::*;
use gl_matrix::common;
use gl_matrix::mat4::{invert, mul};
use gltf::{Accessor, animation, Scene};
use gltf::animation::util::{ReadOutputs, Rotations};
use gltf::iter::Animations;
use halfbrown::HashMap;
use tokio::io::AsyncReadExt;
use crate::helpers::{gfx_maths_mat4_to_glmatrix_mat4, glmatrix_mat4_to_gfx_maths_mat4, gltf_matrix_to_gfx_maths_mat4, interpolate_mats};

#[derive(Clone, Debug)]
pub struct SkeletalAnimations {
    pub name: String,
    pub root_bones: Vec<usize>,
    pub bones: Vec<SkeletalBone>,
    pub animations: HashMap<String, SkeletalAnimation>,
}

#[derive(Clone, Debug)]
pub struct SkeletalAnimation {
    pub name: String,
    pub time: f32,
    pub max_time: f32,
    pub last_update: Option<std::time::Instant>,
    pub framecount: usize,
    pub frames: Vec<HashMap<usize, SkeletalKeyframe>>, // Vec of HashMaps, each HashMap is a frame, each HashMap contains a bone index and a keyframe
}

#[derive(Clone, Debug)]
pub struct SkeletalKeyframe {
    pub time: f32,
    pub bone: usize,
    pub translate: Option<Mat4>,
    pub rotate: Option<Mat4>,
    pub scale: Option<Mat4>,
}

#[derive(Clone, Debug)]
pub struct SkeletalBone {
    pub name: String,
    pub index: usize,
    pub children: Vec<usize>,
    pub inverse_bind_matrix: Mat4,
    pub animated_transform: Mat4,
}

#[derive(Clone, Debug)]
pub struct SkeletalWeight {
    pub vertex: usize,
    pub weight: f32,
}

#[derive(Clone, Debug)]
pub enum SkeletalAnimationError {
    WeightLoadingError,
}

impl SkeletalBone {
    fn get_two_closest_keyframes_and_interpolate(&self, animation: &SkeletalAnimation) -> Mat4 {
        let mut min = (None, None, None); // (translate, rotate, scale)
        let mut max = (None, None, None);
        let mut stop_min_mode = false;
        for i in 0..animation.framecount {
            if !animation.frames[i].contains_key(&self.index) {
                continue;
            }
            let keyframe = &animation.frames[i][&self.index];
            if keyframe.time > animation.time {
                stop_min_mode = true;
                if max.0.is_none() {
                    max.0 = keyframe.translate;
                   // debug!("found max translate at {}", keyframe.time);
                }
                if max.1.is_none() {
                    max.1 = keyframe.rotate;
                    //debug!("found max rotate at {}", keyframe.time);
                }
                if max.2.is_none() {
                    max.2 = keyframe.scale;
                    //debug!("found max scale at {}", keyframe.time);
                }
                // if all three are set, break
                if max.0.is_some() && max.1.is_some() && max.2.is_some() {
                    break;
                }
            }
            if !stop_min_mode {
                if let Some(translate) = keyframe.translate {
                    min.0 = Some(translate);
                   // debug!("found min translate at {}", keyframe.time);
                }
                if let Some(rotate) = keyframe.rotate {
                    min.1 = Some(rotate);
                    //debug!("found min rotate at {}", keyframe.time);
                }
                if let Some(scale) = keyframe.scale {
                    min.2 = Some(scale);
                    //debug!("found min scale at {}", keyframe.time);
                }
            }
        }

        if (min.0.is_none() && min.1.is_none() && min.2.is_none()) || (max.0.is_none() && max.1.is_none() && max.2.is_none()) {
            // can we find at least one frame?
            return if animation.frames.iter().any(|x| x.contains_key(&self.index)) {
                // if so, return the first frame
                let mut mat = Mat4::identity();
                let working_mat = 0;
                if let Some(translate) = animation.frames[working_mat][&self.index].translate {
                    mat = mat * translate;
                }
                if let Some(rotate) = animation.frames[working_mat][&self.index].rotate {
                    mat = mat * rotate;
                }
                if let Some(scale) = animation.frames[working_mat][&self.index].scale {
                    mat = mat * scale;
                }
                mat
            } else {
                // if not, return the identity matrix
                Mat4::identity()
            }
        }

        let (min_translate, min_rotate, min_scale) = min;
        let (max_translate, max_rotate, max_scale) = max;

        let mat_a = {
            let mut mat = Mat4::identity();
            if let Some(translate) = &min_translate {
                mat = mat * translate;
            }
            if let Some(rotate) = &min_rotate {
                mat = mat * rotate;
            }
            if let Some(scale) = &min_scale {
                mat = mat * scale;
            }
            mat
        };
        let mat_b = {
            let mut mat = Mat4::identity();
            if let Some(translate) = &max_translate {
                mat = mat * translate;
            }
            if let Some(rotate) = &max_rotate {
                mat = mat * rotate;
            }
            if let Some(scale) = &max_scale {
                mat = mat * scale;
            }
            mat
        };

        interpolate_mats(mat_a, mat_b, animation.time as f64)
        //mat_b
    }
}

impl SkeletalAnimations {
    pub fn load_skeleton_stuff(skin: &gltf::Skin, mesh: &gltf::Mesh, animations: gltf::iter::Animations, buffers: &[gltf::buffer::Data]) -> Result<SkeletalAnimations, SkeletalAnimationError> {
        let mut bones_final = HashMap::new();
        let mut bone_order = Vec::new();
        let mut root_bones = Vec::new();
        let mut animations_final = HashMap::new();

        // first, get the weights from the mesh
        let mut weights_final = Vec::new();
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let weights = reader.read_weights(0).ok_or(SkeletalAnimationError::WeightLoadingError)?;
            let weights = weights.into_f32().collect::<Vec<_>>();
            weights_final.push(weights);
        }

        // for bones, they are layed out as an array with each bone specifying the index of its children
        for joint in skin.joints() {
            let mut children = Vec::new();
            for child in joint.children() {
                children.push(child.index());
            }
            let mut weights = Vec::new();
            for (i, weight) in weights_final.iter().enumerate() {
                let weight = weight[joint.index()];
                if weight[0] != 0.0 {
                    weights.push(SkeletalWeight {
                        vertex: i,
                        weight: weight[0],
                    });
                }
            }
            // we'll fill in the inverse_bind_matrix in a second, just insert the bones first
            bones_final.insert(joint.index(), SkeletalBone {
                name: joint.name().unwrap_or("unnamed").to_string(),
                index: joint.index(),
                children,
                inverse_bind_matrix: Mat4::identity(),
                animated_transform: Mat4::identity(),
            });
            bone_order.push(joint.index());
        }

        let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));
        let inverse_bind_matrices = reader.read_inverse_bind_matrices().unwrap();
        for (i, inverse_bind_matrix) in inverse_bind_matrices.enumerate() {
            // ahh lovely, we're given a [[f32; 4]; 4] matrix, but we need a [f32; 16] matrix
            let mat = gltf_matrix_to_gfx_maths_mat4(inverse_bind_matrix);
            // this should be good, so add it to the bone
            bones_final.get_mut(&i).unwrap().inverse_bind_matrix = mat;
        }

        // iterate one more time to find the root bones (bones with no parents)
        // todo: this is a really dumb way to do it, please find a better way
        let mut no_parents = HashSet::new();
        for bone in bones_final.values() {
            no_parents.insert(bone.index);
        }
        for bone in bones_final.values() {
            for child in bone.children.iter() {
                no_parents.remove(child);
            }
        }
        for bone in no_parents {
            root_bones.push(bone);
        }

        // now, get the animations
        for animation in animations {
            let mut keyframes: HashMap<usize, HashMap<usize, SkeletalKeyframe>> = HashMap::new(); // outer hashmap is a frame, inner hashmap is a bone
            let mut highest_time = 0.0;
            for channel in animation.channels() {
                let sampler = channel.sampler();
                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
                let times = reader.read_inputs().unwrap();
                let mut matrices = reader.read_outputs().unwrap();
                for (i, time) in times.enumerate() {
                    if time > highest_time {
                        highest_time = time;
                    }
                    match &matrices {
                        ReadOutputs::Translations(translations) => {
                            let translation = translations.clone().nth(i).unwrap();
                            let matrix = Mat4::translate(Vec3::new(translation[0], translation[1], translation[2]));
                            let bone = channel.target().node().index();
                            if !keyframes.contains_key(&i) {
                                keyframes.insert(i, HashMap::new());
                            }
                            if !keyframes[&i].contains_key(&bone) {
                                keyframes.get_mut(&i).unwrap().insert(bone, SkeletalKeyframe {
                                    time,
                                    bone,
                                    translate: Some(matrix),
                                    rotate: None,
                                    scale: None,
                                });
                            } else {
                                keyframes.get_mut(&i).unwrap().get_mut(&bone).unwrap().translate = Some(matrix);
                            }
                        },
                        ReadOutputs::Rotations(rotations) => {
                            let matrix = match &rotations {
                                Rotations::I8(i8s) => {
                                    let rotation = i8s.clone().nth(i).unwrap();
                                    Mat4::rotate(Quaternion::new(rotation[0] as f32, rotation[1] as f32, rotation[2] as f32, rotation[3] as f32))
                                },
                                Rotations::U8(u8s) => {
                                    let rotation = u8s.clone().nth(i).unwrap();
                                    Mat4::rotate(Quaternion::new(rotation[0] as f32, rotation[1] as f32, rotation[2] as f32, rotation[3] as f32))
                                },
                                Rotations::I16(i16s) => {
                                    let rotation = i16s.clone().nth(i).unwrap();
                                    Mat4::rotate(Quaternion::new(rotation[0] as f32, rotation[1] as f32, rotation[2] as f32, rotation[3] as f32))
                                },
                                Rotations::U16(u16s) => {
                                    let rotation = u16s.clone().nth(i).unwrap();
                                    Mat4::rotate(Quaternion::new(rotation[0] as f32, rotation[1] as f32, rotation[2] as f32, rotation[3] as f32))
                                },
                                Rotations::F32(f32s) => {
                                    let rotation = f32s.clone().nth(i).unwrap();
                                    Mat4::rotate(Quaternion::new(rotation[0], rotation[1], rotation[2], rotation[3]))
                                },
                            };
                            let bone = channel.target().node().index();
                            if !keyframes.contains_key(&i) {
                                keyframes.insert(i, HashMap::new());
                            }
                            if !keyframes[&i].contains_key(&bone) {
                                keyframes.get_mut(&i).unwrap().insert(bone, SkeletalKeyframe {
                                    time,
                                    bone,
                                    translate: None,
                                    rotate: Some(matrix),
                                    scale: None,
                                });
                            } else {
                                keyframes.get_mut(&i).unwrap().get_mut(&bone).unwrap().rotate = Some(matrix);
                            }
                        },
                        ReadOutputs::Scales(scales) => {
                            let scale = scales.clone().nth(i).unwrap();
                            let matrix = Mat4::scale(Vec3::new(scale[0], scale[1], scale[2]));
                            let bone = channel.target().node().index();
                            if !keyframes.contains_key(&i) {
                                keyframes.insert(i, HashMap::new());
                            }
                            if !keyframes[&i].contains_key(&bone) {
                                keyframes.get_mut(&i).unwrap().insert(bone, SkeletalKeyframe {
                                    time,
                                    bone,
                                    translate: None,
                                    rotate: None,
                                    scale: Some(matrix),
                                });
                            } else {
                                keyframes.get_mut(&i).unwrap().get_mut(&bone).unwrap().scale = Some(matrix);
                            }
                        },
                        ReadOutputs::MorphTargetWeights(yeah) => {
                            debug!("morph target weights");
                        }
                    }
                }
            }

            // order keyframes by time
            let mut keyframes_ordered: Vec<HashMap<usize, SkeletalKeyframe>> = keyframes.values().cloned().collect();
            keyframes_ordered.sort_by(|a, b| a[&0].time.partial_cmp(&b[&0].time).unwrap());

            animations_final.insert(animation.name().unwrap_or("").to_string(), SkeletalAnimation {
                name: animation.name().unwrap_or("").to_string(),
                time: 0.0,
                max_time: highest_time,
                last_update: None,
                framecount: keyframes.len(),
                frames: keyframes_ordered,
            });
        }

        // last step: order bones by index
        let mut bones_ordered: Vec<SkeletalBone> = Vec::new();
        for i in bone_order {
            bones_ordered.push(bones_final[&i].clone());
        }

        Ok(SkeletalAnimations {
            name: skin.name().unwrap_or("").to_string(),
            root_bones,
            bones: bones_ordered,
            animations: animations_final,
        })
    }

    pub fn apply_poses_i_stole_this_from_reddit_user_a_carotis_interna(&mut self, joint: usize, parent: Mat4, animation: &SkeletalAnimation) {
        let bone = self.bones[joint].clone();
        let pose = bone.get_two_closest_keyframes_and_interpolate(animation);
        let mut pose = parent * pose;
        for child in &bone.children {
            self.apply_poses_i_stole_this_from_reddit_user_a_carotis_interna(*child, pose, animation);
        }
        pose = pose * bone.inverse_bind_matrix;
        self.bones[joint].animated_transform = pose;
    }
}

impl SkeletalAnimation {
    pub fn advance_time(&mut self, delta_time: f32) {
        const SCALE: f32 = 1.0;
        self.time += (delta_time * SCALE);
        if self.time > self.max_time {
            self.time = 0.0;
        }
    }

    // this is called per frame to get the joint matrices for the current frame
    // should take into account each bone offset matrix
    // joint_matrix[j] = inverse(global_transform) * global_transform[j] * bone_offset[j]
    pub fn get_joint_matrices(&self, animations: &SkeletalAnimations) -> Vec<Mat4> {
        // assume that "apply_poses" has been called
        let mut joint_matrices = Vec::new();
        for bone in &animations.bones {
            joint_matrices.push(bone.animated_transform);
        }
        joint_matrices
    }
}