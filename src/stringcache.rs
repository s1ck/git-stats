use fxhash::FxBuildHasher;
use indexmap::set::IndexSet;
use std::ops::Index;

pub struct StringCache {
    index_set: IndexSet<String, FxBuildHasher>,
}

impl StringCache {
    pub(crate) fn new() -> Self {
        StringCache {
            index_set: IndexSet::default(),
        }
    }

    pub(crate) fn intern<T>(&mut self, k: T) -> usize
    where
        T: AsRef<str> + Into<String>,
    {
        match self.index_set.get_index_of(k.as_ref()) {
            Some(idx) => idx,
            None => self.index_set.insert_full(k.into()).0,
        }
    }

    fn get(&self, idx: usize) -> Option<&str> {
        self.index_set.get_index(idx).map(|s| s.as_str())
    }
}

impl Index<usize> for StringCache {
    type Output = str;

    fn index(&self, index: usize) -> &Self::Output {
        match self.get(index) {
            Some(item) => item,
            None => panic!("No entry for index {}", index),
        }
    }
}
