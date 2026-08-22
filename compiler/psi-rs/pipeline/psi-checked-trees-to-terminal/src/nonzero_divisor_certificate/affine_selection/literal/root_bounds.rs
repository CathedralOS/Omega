//! Fixed root-bound orientations for affine-literal certificate production.

use psi_core::{Proposition, ScalarTerm};

pub(super) struct OrientedBound {
    pub(super) proposition: Proposition,
    pub(super) substitution_endpoint: usize,
}

pub(super) fn ordered(value: &ScalarTerm, literal: &ScalarTerm) -> [OrientedBound; 2] {
    [
        OrientedBound {
            proposition: Proposition::LessOrEqual(literal.clone(), value.clone()),
            substitution_endpoint: 1,
        },
        OrientedBound {
            proposition: Proposition::LessOrEqual(value.clone(), literal.clone()),
            substitution_endpoint: 0,
        },
    ]
}
