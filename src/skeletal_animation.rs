use gfx_maths::*;
use gltf::animation;
use gltf::animation::util::ReadOutputs;
use gltf::iter::Animations;

#[derive(Clone, Debug)]
pub struct SkeletalAnimations {
    pub name: String,
    pub bones: Vec<SkeletalBone>,
    pub animations: Vec<SkeletalAnimation>,
}

#[derive(Clone, Debug)]
pub struct SkeletalAnimation {
    pub name: String,
    pub time: f32,
    pub duration: f32,
    pub animation: Vec<SkeletalKeyframe>,
}

#[derive(Clone, Debug)]
pub struct SkeletalBone {
    pub name: String,
    pub children: Vec<usize>,
    pub position: Vec3,
    pub rotation: Quaternion,
    pub scale: Vec3,
    pub weights: Vec<SkeletalWeight>,
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
    pub position: Vec3,
    pub rotation: Quaternion,
    pub scale: Vec3,
}

#[derive(Clone, Debug)]
pub enum SkeletalAnimationError {
    WeightLoadingError,
}

impl SkeletalAnimations {
    pub fn load_skeleton_stuff(skin: &gltf::skin::Skin, mesh: &gltf::Mesh, animations: Animations, buffers: &Vec<gltf::buffer::Data>) -> Result<Self, SkeletalAnimationError> {
        let mut bones_final = Vec::new();
        let mut animations_final = Vec::new();

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
            bones_final.push(SkeletalBone {
                name: joint.name().unwrap_or("").to_string(),
                children,
                position: Vec3::new(0.0, 0.0, 0.0),
                rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
                scale: Vec3::new(1.0, 1.0, 1.0),
                weights,
            });
        }

        // now, get the animations
        for animation in animations {
            let mut animation_final = Vec::new();
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
                                position: Vec3::new(translation[0], translation[1], translation[2]),
                                rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
                                scale: Vec3::new(1.0, 1.0, 1.0),
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
                                position: Vec3::new(0.0, 0.0, 0.0),
                                rotation: Quaternion::new(rotation[0], rotation[1], rotation[2], rotation[3]),
                                scale: Vec3::new(1.0, 1.0, 1.0),
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
                                position: Vec3::new(0.0, 0.0, 0.0),
                                rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
                                scale: Vec3::new(scale[0], scale[1], scale[2]),
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
            }
            animations_final.push(SkeletalAnimation {
                name: animation.name().unwrap_or("").to_string(),
                time: 0.0,
                duration: animation.
                animation: animation_final,
            });
        }

        Ok(SkeletalAnimations {
            name: skin.name().unwrap_or("").to_string(),
            bones: bones_final,
            animations: animations_final,
        })
    }
}