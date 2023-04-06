use gl_matrix::common::Vec2;

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