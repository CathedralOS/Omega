//! Producer-only recognition of the cataloged legalization forms.

mod scalar;
mod scalar_call_unit;
mod structural_unit;
mod unit;

pub(super) use scalar::match_scalar_form;
pub(super) use scalar_call_unit::match_scalar_call_unit_form;
pub(super) use structural_unit::{MatchedStructuralUnitForm, match_structural_unit_form};
pub(super) use unit::match_unit_form;
