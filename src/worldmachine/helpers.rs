use gfx_maths::{Quaternion, Vec3};
use crate::worldmachine::components::Transform;
use crate::worldmachine::ecs::Component;

pub fn serialize_vec3(vec: &Vec3) -> String {
    format!("{},{},{}", vec.x, vec.y, vec.z)
}

pub fn deserialize_vec3(serialization: &str) -> Result<Vec3, String> {
    debug!("deserializing vec3 from {}", serialization);
    let mut split = serialization.split(',');
    let x = split.next().unwrap().parse::<f32>().map_err(|e| {
        format!("failed to parse x value: {}", e)
    })?;
    let y = split.next().unwrap().parse::<f32>().map_err(|e| {
        format!("failed to parse y value: {}", e)
    })?;
    let z = split.next().unwrap().parse::<f32>().map_err(|e| {
        format!("failed to parse z value: {}", e)
    })?;
    Ok(Vec3::new(x, y, z))
}

pub fn serialize_quaternion(quat: &Quaternion) -> String {
    format!("{},{},{},{}", quat.x, quat.y, quat.z, quat.w)
}

pub fn deserialize_quaternion(serialization: &str) -> Result<Quaternion, String> {
    let mut split = serialization.split(',');
    let x = split.next().unwrap().parse::<f32>().map_err(|e| {
        format!("failed to parse x value: {}", e)
    })?;
    let y = split.next().unwrap().parse::<f32>().map_err(|e| {
        format!("failed to parse y value: {}", e)
    })?;
    let z = split.next().unwrap().parse::<f32>().map_err(|e| {
        format!("failed to parse z value: {}", e)
    })?;
    let w = split.next().unwrap().parse::<f32>().map_err(|e| {
        format!("failed to parse w value: {}", e)
    })?;
    Ok(Quaternion::new(x, y, z, w))
}