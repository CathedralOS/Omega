//! Verifier-local exact cast endpoint remapping.

use psi_core::{IntegerType, Proposition, ScalarTerm};

use super::super::super::cast_custody;

pub(super) fn remap(
    bound: &Proposition,
    source: &ScalarTerm,
    cast_root: &ScalarTerm,
    cast_type: IntegerType,
) -> Option<Proposition> {
    let Proposition::LessOrEqual(left, right) = bound else {
        return None;
    };
    if left == source {
        Some(Proposition::LessOrEqual(
            cast_root.clone(),
            cast_custody::remap_integer_literal(right, cast_type)?,
        ))
    } else if right == source {
        Some(Proposition::LessOrEqual(
            cast_custody::remap_integer_literal(left, cast_type)?,
            cast_root.clone(),
        ))
    } else {
        None
    }
}
