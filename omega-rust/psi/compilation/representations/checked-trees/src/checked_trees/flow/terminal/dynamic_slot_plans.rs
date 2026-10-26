//! The call shape of one runtime-selected requirement.
//!
//! A selector call (`Trait::member(selector, data...)`) targets its
//! instantiated slot: the requirement under one `dyn Trait<Args>`
//! application. Every implementation a selector can name realizes this exact
//! shape, so call lowering plans operands against it and the dispatch catalog
//! declares it as the interface slot's call shape, rather than letting one
//! implementation's signature stand for the others.

use crate::checked_trees::flow::terminal::{
    CheckedBoundaryMachineResultPlan, CheckedStructuralScalarParameterPlan,
    CheckedUnitStructuralParameterPlan,
};
use symbols::SymbolHandle;
use typed_trees::type_identity::CallableIdentityId;
use typed_trees::type_identity::TypeIdentityId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicSlotPlan {
    /// The slot symbol selector calls target.
    pub slot: SymbolHandle,
    /// Normalized identity of the selector type (`dyn Shape<Item>`).
    pub interface_identity: TypeIdentityId,
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    /// Normalized requirement identity, the key of the slot's table row.
    pub requirement_identity: CallableIdentityId,
    /// Exact canonical local tuple; empty for a nongeneric row or the raw
    /// runtime-call helper. Only concrete tuple rows enter a family table.
    pub family_tuple: Vec<String>,
    /// Structural formals after the selector, positioned in the call's
    /// operand order (the selector is operand 0).
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Requirement-owned disposal frontier, independent of implementation
    /// claim identities. Indices name the dense structural argument lane.
    pub entry_claim_paths: Vec<CheckedDynamicClaimPath>,
    /// Scalar formals in operand order; the selector is the first, at
    /// position 0, and every implementation receives the rest.
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub result: CheckedBoundaryMachineResultPlan,
    /// Public possible result claim frontier; implementation identities remain private.
    pub result_claim_paths: Vec<Vec<crate::CheckedUnitStructuralPathSegment>>,
    /// Public result/input lifetime regions, indexed only by structural formals.
    pub result_reference_sources: Vec<crate::CheckedReturnedReferencePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicClaimPath {
    pub argument_index: u32,
    pub path: Vec<crate::CheckedUnitStructuralPathSegment>,
}
