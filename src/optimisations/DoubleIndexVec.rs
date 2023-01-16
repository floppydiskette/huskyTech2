use std::slice::Iter;
use halfbrown::HashMap;

/// vec but you can index it with two different options
/// used mainly cause bones need to be indexed by both bone index and bone send to gpu order
#[derive(Clone, Debug)]
pub struct DoubleIndexVec<T> {
    vec: Vec<T>,
    index: HashMap<usize, usize>,
}

impl<T> DoubleIndexVec<T> {
    pub fn new() -> Self {
        Self {
            vec: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub fn push(&mut self, value: T, b_index: usize) {
        self.vec.push(value);
        self.index.insert(b_index, self.vec.len() - 1);
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.vec.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.vec.get_mut(index)
    }

    pub fn get_by_b_index(&self, b_index: usize) -> Option<&T> {
        self.index.get(&b_index).and_then(|index| self.vec.get(*index))
    }

    pub fn get_by_b_index_mut(&mut self, b_index: usize) -> Option<&mut T> {
        self.index.get(&b_index).and_then(|index| self.vec.get_mut(*index))
    }

    pub fn values(&self) -> std::slice::Iter<T> {
        self.vec.iter()
    }

    pub fn iter(&self) -> Iter<T> {
        self.vec.iter()
    }
}