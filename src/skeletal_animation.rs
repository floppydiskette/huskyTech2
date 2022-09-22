use std::simd::Simd;
use gfx_maths::*;
use gl_matrix::common;
use gl_matrix::mat4::{invert, mul};
use gltf::{animation, Scene};
use gltf::animation::util::ReadOutputs;
use gltf::iter::Animations;
use halfbrown::HashMap;

#[derive(Clone, Debug)]
pub struct SkeletalAnimations {
    pub name: String,
    pub bones: HashMap<usize, SkeletalBone>,
    pub animations: HashMap<String, SkeletalAnimation>,
}

#[derive(Clone, Debug)]
pub struct SkeletalAnimation {
    pub name: String,
    pub time: f32,
    pub current_frame: usize,
    pub last_update: Option<std::time::Instant>,
    pub duration: f32,
    pub animation: Vec<SkeletalKeyframe>,
}

#[derive(Clone, Debug)]
pub struct SkeletalBone {
    pub name: String,
    pub index: usize,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub offset: Mat4,
    pub weights: Vec<SkeletalWeight>,
    pub inverse_bind_matrix: Mat4,
}

#[derive(Clone, Debug)]
pub struct SkeletalWeight {
    pub vertex: usize,
    pub weight: f32,
}

#[derive(Clone, Debug)]
pub struct SkeletalKeyframe {
    pub time: f64,
    pub bone_transforms: Vec<SkeletalBoneTransform>,
}

#[derive(Clone, Debug)]
pub struct SkeletalBoneTransform {
    pub bone_index: i32,
    pub position: Option<Vec3>,
    pub rotation: Option<Quaternion>,
    pub scale: Option<Vec3>,
}

#[derive(Clone, Debug)]
pub enum SkeletalAnimationError {
    WeightLoadingError,
}

fn gen_inverse_bind_pose(bone: &SkeletalBone, bones: &Vec<&SkeletalBone>) -> Mat4 {
    let mut transform = Mat4::identity();
    if let Some(parent_index) = bone.parent {
        transform = transform * gen_inverse_bind_pose(&bones[parent_index as usize], bones);
    }
    transform = transform * bone.offset;
    transform
}

impl SkeletalAnimations {
    pub fn load_skeleton_stuff(skin: &gltf::skin::Skin, mesh: &gltf::Mesh, animations: Animations, buffers: &[gltf::buffer::Data]) -> Result<Self, SkeletalAnimationError> {
        let mut bones_final = HashMap::new();
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
            let offset = joint.transform().matrix();
            bones_final.insert(joint.index(), SkeletalBone {
                name: joint.name().unwrap_or("").to_string(),
                index: joint.index(),
                parent: None,
                children,
                offset: Mat4::from(offset),
                weights,
                inverse_bind_matrix: Mat4::identity(),
            });
        }

        // iterate over each bone and set parents and inverse bind matrices
        for bone in bones_final.values_mut() {
            for (i, parent) in bones_final.values().enumerate() {
                if parent.children.contains(&bone.index) {
                    bone.parent = Some(i);
                }
                bone.inverse_bind_matrix = gen_inverse_bind_pose(bone, &bones_final.values().collect::<Vec<_>>());
            }
        }

        // now, get the animations
        for animation in animations {
            let mut animation_final = Vec::new();
            let mut total_time = 0.0;
            for channel in animation.channels() {
                let sampler = channel.sampler();
                let input = channel.reader(|buffer| Some(&buffers[buffer.index()])).read_inputs().unwrap();
                let output = channel.reader(|buffer| Some(&buffers[buffer.index()])).read_outputs().unwrap();
                let input = input.collect::<Vec<_>>();
                match output {
                    ReadOutputs::Translations(translations) => {
                        let translations = translations.collect::<Vec<_>>();
                        for (i, translation) in translations.iter().enumerate() {
                            let bone_index = channel.target().node().index();
                            let bone_transform = SkeletalBoneTransform {
                                bone_index: bone_index as i32,
                                position: Some(Vec3::new(translation[0], translation[1], translation[2])),
                                rotation: None,
                                scale: None,
                            };
                            if animation_final.len() <= i {
                                animation_final.push(SkeletalKeyframe {
                                    time: input[i] as f64,
                                    bone_transforms: vec![bone_transform],
                                });
                            } else {
                                animation_final[i].bone_transforms.push(bone_transform);
                            }
                        }
                    },
                    ReadOutputs::Rotations(rotations) => {
                        let rotations = match rotations {
                            animation::util::Rotations::U8(rotations) => rotations.map(|r| r.map(|r| r as f32 / 255.0)).collect::<Vec<_>>(),
                            animation::util::Rotations::I8(rotations) => rotations.map(|r| r.map(|r| r as f32 / 127.0)).collect::<Vec<_>>(),
                            animation::util::Rotations::U16(rotations) => rotations.map(|r| r.map(|r| r as f32 / 65535.0)).collect::<Vec<_>>(),
                            animation::util::Rotations::I16(rotations) => rotations.map(|r| r.map(|r| r as f32 / 32767.0)).collect::<Vec<_>>(),
                            animation::util::Rotations::F32(rotations) => rotations.collect::<Vec<_>>(),
                        };
                        for (i, rotation) in rotations.iter().enumerate() {
                            let bone_index = channel.target().node().index();
                            let bone_transform = SkeletalBoneTransform {
                                bone_index: bone_index as i32,
                                position: None,
                                rotation: Some(Quaternion::new(rotation[3], rotation[0], rotation[1], rotation[2])),
                                scale: None,
                            };
                            if animation_final.len() <= i {
                                animation_final.push(SkeletalKeyframe {
                                    time: input[i] as f64,
                                    bone_transforms: vec![bone_transform],
                                });
                            } else {
                                animation_final[i].bone_transforms.push(bone_transform);
                            }
                        }
                    },
                    ReadOutputs::Scales(scales) => {
                        let scales = scales.collect::<Vec<_>>();
                        for (i, scale) in scales.iter().enumerate() {
                            let bone_index = channel.target().node().index();
                            let bone_transform = SkeletalBoneTransform {
                                bone_index: bone_index as i32,
                                position: None,
                                rotation: None,
                                scale: Some(Vec3::new(scale[0], scale[1], scale[2])),
                            };
                            if animation_final.len() <= i {
                                animation_final.push(SkeletalKeyframe {
                                    time: input[i] as f64,
                                    bone_transforms: vec![bone_transform],
                                });
                            } else {
                                animation_final[i].bone_transforms.push(bone_transform);
                            }
                        }
                    },
                    ReadOutputs::MorphTargetWeights(_) => {},
                };

                total_time = input[input.len() - 1] as f64;
            }
            debug!("new animation: {}", animation.name().unwrap_or(""));
            debug!("total time: {}", total_time);
            debug!("total frames: {}", animation_final.len());
            animations_final.insert(animation.name().unwrap_or("").to_string(), SkeletalAnimation {
                name: animation.name().unwrap_or("").to_string(),
                time: 0.0,
                current_frame: 0,
                last_update: None,
                duration: total_time as f32,
                animation: animation_final,
            });
        }

        debug!("total of {} bones", bones_final.len());
        debug!("total of {} animations", animations_final.len());

        Ok(SkeletalAnimations {
            name: skin.name().unwrap_or("").to_string(),
            bones: bones_final,
            animations: animations_final,
        })
    }
}

impl SkeletalAnimation {
    pub fn advance_time(&mut self, delta_time: f32) {
        self.time += delta_time;
        self.current_frame = (self.time / self.duration * self.animation.len() as f32) as usize;
        if self.current_frame >= self.animation.len() {
            self.current_frame = 0;
        }
        if self.time >= self.duration {
            self.time = 0.0;
        }
    }

    // this is called per frame to get the joint matrices for the current frame
    // should take into account each bone offset matrix
    // joint_matrix[j] = inverse(global_transform) * global_transform[j] * bone_offset[j]
    pub fn get_joint_matrices(&self, animations: &SkeletalAnimations) -> Vec<Mat4> {
        let mut joint_matrices = Vec::new();
        let keyframe = &self.animation[self.current_frame];
        for transform in keyframe.bone_transforms {
            let bone = &animations.bones.get(&(transform.bone_index as usize));
            if let Some(bone) = bone {
                let bone_offset = bone.offset;
                let mut bone_transform = Mat4::identity();
                if let Some(position) = transform.position {
                    bone_transform = bone_transform * Mat4::translate(position);
                }
                if let Some(rotation) = transform.rotation {
                    bone_transform = bone_transform * Mat4::rotate(rotation);
                }
                if let Some(scale) = transform.scale {
                    bone_transform = bone_transform * Mat4::scale(scale);
                }
                joint_matrices.push(joint_matrix);
            }
        }
        joint_matrices
    }
}