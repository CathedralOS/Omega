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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicSlotPlan {
    /// The slot symbol selector calls target.
    pub slot: SymbolHandle,
    /// Normalized identity of the selector type (`dyn Shape<Item>`).
    pub interface_identity: String,
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    /// Normalized requirement identity, the key of the slot's table row.
    pub requirement_identity: String,
    /// Structural formals after the selector, positioned in the call's
    /// operand order (the selector is operand 0).
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Scalar formals in operand order; the selector is the first, at
    /// position 0, and every implementation receives the rest.
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub result: CheckedBoundaryMachineResultPlan,
}
