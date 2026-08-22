//! Affine frontier cursor custody for independent reconstruction.

use psi_core::ScalarTerm;

pub(in crate::verification::reconstruction::affine_custody::frontier) struct Entry {
    pub(super) word: Vec<usize>,
    pub(super) start: usize,
    pub(super) current: ScalarTerm,
}

impl Entry {
    pub(in crate::verification::reconstruction::affine_custody::frontier) fn root(
        root: &ScalarTerm,
    ) -> Self {
        Self {
            word: Vec::new(),
            start: 0,
            current: root.clone(),
        }
    }
}
