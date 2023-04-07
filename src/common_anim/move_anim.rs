use gfx_maths::Vec2;
use crate::animgraph::{AnimGraph, AnimGraphNode};

#[derive(Clone, Debug)]
pub struct Features {
    pub backwards: bool,
    pub strafe: bool,
    pub sprint: bool,
    pub crouch: bool,
}

#[derive(Clone, Debug)]
pub struct MoveAnim {
    pub features: Features,
    pub speed: f64,
    pub strafe: f64,
    pub sprint: bool,
    pub crouch: bool,
    pub inner: AnimGraph,
}

impl MoveAnim {
    pub fn new(features: Features) -> Self {
        let mut graph = AnimGraph::new();
        graph.add_node("idle".to_string(), Vec2::new(0.0, 0.0), "idle".to_string());
        graph.add_node("walk".to_string(), Vec2::new(0.0, 1.0), "walk".to_string());
        if features.backwards {
            graph.add_node("walkBack".to_string(), Vec2::new(0.0, -1.0), "walkBack".to_string());
        }
        if features.sprint {
            graph.add_node("run".to_string(), Vec2::new(0.0, 2.0), "sprint".to_string());
        }
        if features.strafe {
            graph.add_node("strafe_left".to_string(), Vec2::new(-1.0, 0.0), "strafe".to_string());
            graph.add_node("strafe_right".to_string(), Vec2::new(1.0, 0.0), "strafe".to_string());
        }

        Self {
            features,
            speed: 0.0,
            strafe: 0.0,
            sprint: false,
            crouch: false,
            inner: graph,
        }
    }

    pub fn from_values(speed: f64, strafe: f64) -> Self {
        let move_anim = MoveAnim {
            features: Features {
                backwards: true,
                strafe: true,
                sprint: false,
                crouch: false,
            },
            speed,
            strafe,
            sprint: false,
            crouch: false,
            inner: AnimGraph {
                nodes: vec![
                    AnimGraphNode {
                        name: "idle".to_string(),
                        position: Vec2::new(0.0, 0.0),
                        animation: "idle".to_string(),
                    },
                    AnimGraphNode {
                        name: "walk".to_string(),
                        position: Vec2::new(0.0, 1.0),
                        animation: "walk".to_string(),
                    },
                    AnimGraphNode {
                        name: "walkBack".to_string(),
                        position: Vec2::new(0.0, -1.0),
                        animation: "walkBack".to_string(),
                    },
                    AnimGraphNode {
                        name: "strafe_left".to_string(),
                        position: Vec2::new(-1.0, 0.0),
                        animation: "strafe".to_string(),
                    },
                    AnimGraphNode {
                        name: "strafe_right".to_string(),
                        position: Vec2::new(1.0, 0.0),
                        animation: "strafe".to_string(),
                    },
                ],
                position: Vec2::new(strafe as f32, speed as f32),
            }
        };
        move_anim
    }

    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed;
        self.inner.position.y = speed as f32;
    }

    pub fn set_strafe(&mut self, strafe: f64) {
        self.strafe = strafe;
        self.inner.position.x = strafe as f32;
    }

    pub fn set_sprint(&mut self, sprint: bool) {
        self.sprint = sprint;
        if sprint {
            self.inner.position.y = 2.0;
        } else {
            self.inner.position.y = self.speed as f32;
        }
    }

    pub fn set_crouch(&mut self, crouch: bool) {
        self.crouch = crouch;
    }

    pub fn weights(&self) -> Vec<(String, f64)> {
        let weights = self.inner.weights();
        //debug!("weights: {:?}", weights);
        weights
    }
}