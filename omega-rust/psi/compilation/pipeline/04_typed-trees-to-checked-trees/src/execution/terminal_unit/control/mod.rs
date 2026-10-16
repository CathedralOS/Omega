//! Structural control and boundary-machine construction.
//!
//! This file owns the control plan entry point. `boundary_machine.rs` builds
//! boundary machines and their static requirements, `checked_machine.rs`
//! builds a checked machine (`construction_trace.rs` records where it stopped
//! when it declines a body) and `call_results.rs` binds call results to unit
//! locals.

mod boundary_contracts;
mod boundary_machine;
mod call_occurrences;
mod call_results;
mod checked_machine;
mod construction_trace;
pub(super) mod reference_captures;
pub(in crate::execution::terminal_unit) mod scalar_arrays;
pub(super) mod statement_sequence;
pub(super) mod structural_operands;

pub(crate) use boundary_machine::{
    build_boundary_machine, build_dynamic_slot_plans, build_static_boundary_requirements,
};
pub(super) use call_occurrences::{
    call_owns_statement_computations, outer_calls_in_prefix_traced, terminal_named_call,
};
pub(crate) use call_results::{
    bind_scalar_call_result, bind_structural_call_result, checked_structural_result_type,
};
pub(crate) use checked_machine::build_checked_machine_residual_parts;
pub(crate) use construction_trace::LocalConstructionTrace;

use super::{
    CheckFacts, CheckedBoundaryMachineResultPlan, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, ExpressionNode,
    MachineSupplyMode, Multiplicity, PermissionEventSource, StatementNode, SymbolHandle,
    TypeReferenceHandle,
};
