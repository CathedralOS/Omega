//! Ordered function storage shared across selected rewrites. A spill changes one
//! function; retaining its replay history must not copy every unrelated body.
//! Mutation detaches only the selected function. Equality and serialization
//! remain content-based; allocation identity is never serialized.

use std::ops::{Index, IndexMut};
use std::sync::Arc;

use super::SelectedFunction;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectedFunctions(Vec<Arc<SelectedFunction>>);

impl SelectedFunctions {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&SelectedFunction> {
        self.0.get(index).map(Arc::as_ref)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut SelectedFunction> {
        self.0.get_mut(index).map(Arc::make_mut)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SelectedFunction> + DoubleEndedIterator {
        self.0.iter().map(Arc::as_ref)
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = &mut SelectedFunction> + DoubleEndedIterator {
        self.0.iter_mut().map(Arc::make_mut)
    }

    pub fn push(&mut self, function: SelectedFunction) {
        self.0.push(Arc::new(function));
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Reports storage reuse for diagnostics and allocation regression controls.
    pub fn shares_function_storage(&self, other: &Self, index: usize) -> bool {
        self.0
            .get(index)
            .zip(other.0.get(index))
            .is_some_and(|(left, right)| Arc::ptr_eq(left, right))
    }
}

impl From<Vec<SelectedFunction>> for SelectedFunctions {
    fn from(functions: Vec<SelectedFunction>) -> Self {
        Self(functions.into_iter().map(Arc::new).collect())
    }
}

impl FromIterator<SelectedFunction> for SelectedFunctions {
    fn from_iter<T: IntoIterator<Item = SelectedFunction>>(functions: T) -> Self {
        Self(functions.into_iter().map(Arc::new).collect())
    }
}

impl Index<usize> for SelectedFunctions {
    type Output = SelectedFunction;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for SelectedFunctions {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        Arc::make_mut(&mut self.0[index])
    }
}

impl<'storage> IntoIterator for &'storage SelectedFunctions {
    type Item = &'storage SelectedFunction;
    type IntoIter = std::iter::Map<
        std::slice::Iter<'storage, Arc<SelectedFunction>>,
        fn(&'storage Arc<SelectedFunction>) -> &'storage SelectedFunction,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(Arc::as_ref)
    }
}

impl<'storage> IntoIterator for &'storage mut SelectedFunctions {
    type Item = &'storage mut SelectedFunction;
    type IntoIter = std::iter::Map<
        std::slice::IterMut<'storage, Arc<SelectedFunction>>,
        fn(&'storage mut Arc<SelectedFunction>) -> &'storage mut SelectedFunction,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut().map(Arc::make_mut)
    }
}
