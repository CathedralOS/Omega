//! Producer-only recognition of the cataloged legalization forms.

mod scalar;
mod structural_unit;

pub(super) use scalar::match_scalar_form;
pub(super) use structural_unit::{MatchedStructuralUnitForm, match_structural_unit_form};
