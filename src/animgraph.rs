use gfx_maths::Vec2;
use crate::helpers::distance2d;

#[derive(Clone, Debug)]
pub struct AnimGraph {
    pub nodes: Vec<AnimGraphNode>,
    pub position: Vec2,
}

#[derive(Clone, Debug)]
pub struct AnimGraphNode {
    pub name: String,
    pub position: Vec2,
    pub animation: String,
}

impl AnimGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            position: Vec2::new(0.0, 0.0),
        }
    }

    pub fn add_node(&mut self, name: String, position: Vec2, animation: String) {
        self.nodes.push(AnimGraphNode {
            name,
            position,
            animation,
        });
    }

    pub fn weights(&self) -> Vec<(String, f64)> {
        let mut weights = Vec::new();
        let position = self.position;
        // weights are 0 if no influence, 1 if full influence
        // nodes further than or equal to 2.0 away have no influence
        // if we are directly on a node, we get 1.0 weight and no other nodes
        let mut max_weight = 0.0;
        for node in &self.nodes {
            let distance = distance2d(position, node.position) as f64;
            let weight = 1.0 - (distance / 2.0).min(1.0);
            if weight > 0.0 {
                weights.push((node.animation.clone(), weight));
            }
            if weight > max_weight {
                max_weight = weight;
            }
            if weight == 1.0 {
                // only one node can have full influence
                weights.retain(|(_, w)| *w == max_weight);
                break;
            }
        }

        weights
    }
}