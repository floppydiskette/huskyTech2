use gfx_maths::Vec3;
use crate::worldmachine::components::COMPONENT_TYPE_LIGHT;
use crate::worldmachine::ecs::{Component, ParameterValue};

#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
}

impl Light {
    pub fn from_component(component: Component) -> Option<Light> {
        if component.get_type() == COMPONENT_TYPE_LIGHT.clone() {
            let position = component.get_parameter("position").unwrap();
            let position = match position.value {
                ParameterValue::Vec3(position) => position,
                _ => panic!("Invalid parameter type for position"),
            };
            let color = component.get_parameter("colour").unwrap();
            let color = match color.value {
                ParameterValue::Vec3(color) => color,
                _ => panic!("Invalid parameter type for colour"),
            };
            let intensity = component.get_parameter("intensity").unwrap();
            let intensity = match intensity.value {
                ParameterValue::Float(intensity) => intensity as f32,
                _ => panic!("Invalid parameter type for intensity"),
            };
            Some(Light {
                position,
                color,
                intensity,
            })
        } else {
            None
        }
    }
}