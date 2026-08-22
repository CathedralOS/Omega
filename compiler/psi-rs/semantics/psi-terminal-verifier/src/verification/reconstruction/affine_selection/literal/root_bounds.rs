//! Fixed root-bound orientations for independent affine-literal reconstruction.

use psi_core::{Proposition, ScalarTerm};

pub(super) struct OrientedBound {
    pub(super) proposition: Proposition,
}

pub(super) fn ordered(value: &ScalarTerm, literal: &ScalarTerm) -> [OrientedBound; 2] {
    [
        OrientedBound {
            proposition: Proposition::LessOrEqual(literal.clone(), value.clone()),
        },
        OrientedBound {
            proposition: Proposition::LessOrEqual(value.clone(), literal.clone()),
        },
    ]
}
