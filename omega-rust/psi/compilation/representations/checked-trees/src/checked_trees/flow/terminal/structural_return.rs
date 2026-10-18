//! Whole structural completion names its actual source place. Parameter slots
//! and produced binding ordinals remain distinct; returning an input does not
//! manufacture an operation result or erase the source/type correspondence.

use super::{CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan};
use language_semantics::Multiplicity;
use typed_trees::type_identity::TypeIdentityId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralReturnPlan {
    pub source: CheckedUnitStructuralArgumentSourcePlan,
    pub completion: CheckedStructuralReturnCompletion,
    pub type_identity: TypeIdentityId,
    pub multiplicity: Multiplicity,
    pub reference_sources: Vec<CheckedReturnedReferencePlan>,
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

/// A function's returned provenance is a relation to ingress, not a recipe
/// for forming a loan. Runtime element choices have no caller-side selector;
/// their actual address and permission remain in the captured returned value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedReturnedReferencePlan {
    pub path: Vec<super::CheckedUnitStructuralPathSegment>,
    pub source: CheckedReferenceOriginSourcePlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedReferenceOriginSourcePlan {
    pub root: CheckedReferenceOriginRoot,
    pub path: Vec<CheckedReferenceOriginProjection>,
    pub extent: semantic_vocabulary::ReferenceOriginExtent,
    /// Type of the selected source endpoint. For a region relation this need
    /// not equal the result referent's type.
    pub type_identity: TypeIdentityId,
    pub access: super::CheckedStructuralAccess,
}

/// A normal result either retains a formal's permission or publishes a
/// permission established by the performed invocation. Executable body replay
/// still authenticates its actual supplier; this declaration forms no pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedReferenceOriginRoot {
    Parameter { index: u32 },
    Invocation { carrier_type: TypeIdentityId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedReferenceOriginProjection {
    Exact(super::CheckedUnitStructuralPathSegment),
    Element,
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
