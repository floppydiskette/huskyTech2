use gfx_maths::*;

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
    pub parent: i32,
    pub position: Vec3,
    pub rotation: Quaternion,
    pub scale: Vec3,
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

impl SkeletalAnimation {
    pub fn from_gltf(animation: gltf::animation::Animation) -> Option<Self> {
        let mut skeletal_animation = SkeletalAnimation {
            name: animation.name().unwrap_or("default").to_string(),
            time: 0.0,
            duration: 0.0,
            animation: Vec::new(),
        };

        let mut iter = animation.samplers();
        while let Some(sampler) = iter.next() {
            let accessor = sampler.input();
        }

        None
    }
}