//! Whole structural completion names its actual source place. Parameter slots
//! and produced binding ordinals remain distinct; returning an input does not
//! manufacture an operation result or erase the source/type correspondence.

use super::{CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan};
use language_semantics::Multiplicity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralReturnPlan {
    pub source: CheckedUnitStructuralArgumentSourcePlan,
    pub type_identity: String,
    pub multiplicity: Multiplicity,
}

impl From<CheckedUnitStructuralResultBindingPlan> for CheckedUnitStructuralReturnPlan {
    fn from(binding: CheckedUnitStructuralResultBindingPlan) -> Self {
        Self {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: binding.binding_ordinal,
            },
            type_identity: binding.type_identity,
            multiplicity: binding.multiplicity,
        }
    }
}
