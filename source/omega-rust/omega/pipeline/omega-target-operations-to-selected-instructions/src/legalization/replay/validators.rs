//! Independent recognition of proposed catalog recipes.

mod scalar;
mod structural_unit;
mod unit;

pub(super) use scalar::scalar_validator_accepts;
pub(super) use structural_unit::{ValidatedStructuralUnitForm, validate_structural_unit_form};
pub(super) use unit::validate_unit_form;
