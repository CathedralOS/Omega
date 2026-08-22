//! Independent ordered root bounds for one directly landed literal.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn retained(root: &ScalarTerm, literal: &ScalarTerm) -> [Proposition; 2] {
    [
        Proposition::LessOrEqual(literal.clone(), root.clone()),
        Proposition::LessOrEqual(root.clone(), literal.clone()),
    ]
}
