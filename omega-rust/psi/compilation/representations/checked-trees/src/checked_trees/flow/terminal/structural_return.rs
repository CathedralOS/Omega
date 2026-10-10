//! Whole structural completion names its actual source place. Parameter slots
//! and produced binding ordinals remain distinct; returning an input does not
//! manufacture an operation result or erase the source/type correspondence.

use super::{CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan};
use language_semantics::Multiplicity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralReturnPlan {
    pub source: CheckedUnitStructuralArgumentSourcePlan,
    pub completion: CheckedStructuralReturnCompletion,
    pub type_identity: String,
    pub multiplicity: Multiplicity,
    pub reference_sources: Vec<CheckedReferenceResultSourcePlan>,
}

/// Completion is an occurrence, distinct from the source storage's birth.
/// A selected value arm retains its evaluation lane; an immediately forwarded
/// terminal call retains its concrete result, not the callee's origin template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralReturnCompletion {
    Value {
        statement_index: u32,
        expression: typed_trees::expression::ExpressionHandle,
        evaluation: crate::FlowPermissionEvaluation,
    },
    Call {
        coordinate: super::CheckedUnitCallCoordinate,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedReferenceResultSourcePlan {
    pub path: Vec<super::CheckedUnitStructuralPathSegment>,
    pub source: super::CheckedUnitStructuralArgumentPlan,
}

impl CheckedUnitStructuralReturnPlan {
    pub fn from_binding(
        binding: CheckedUnitStructuralResultBindingPlan,
        completion: CheckedStructuralReturnCompletion,
    ) -> Self {
        Self {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: binding.binding_ordinal,
            },
            completion,
            type_identity: binding.type_identity,
            multiplicity: binding.multiplicity,
            reference_sources: Vec::new(),
        }
    }
}
