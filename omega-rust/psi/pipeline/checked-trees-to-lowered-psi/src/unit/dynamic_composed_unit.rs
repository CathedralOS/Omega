//! Source-free lowering for bounded local dynamic calls.
//!
//! A never-rebound value lowers to a direct call. A value rebound exactly once
//! retains two selections and an indirect descriptor call. A checked forwarded
//! call additionally preserves its dynamic parameter, caller argument, and
//! parameter dispatch instead of composing the helper away. Every lane retains
//! the exact conformance application, source field subloan, requirement row,
//! and realization callable in Terminal custody.
//!
//! This file owns the per-lane entry points. `dynamic_lanes.rs` carries the
//! lane shapes and the shared lowering, `plan_validation.rs` validates the
//! exact plans, `source_lowering.rs` lowers sources and call custody,
//! `forwarded_helpers.rs` materializes forwarded helper chains,
//! `structural_types.rs` lowers structural types, `realizations.rs`
//! collects and materializes realizations, `applications.rs` lowers the
//! conformance applications and `store_operations.rs` lowers caller and
//! realization stores; `continuation.rs`, `join.rs`, `unit.rs` and
//! `unit_join.rs` carry the lane bodies.

mod applications;
mod continuation;
mod dynamic_lanes;
mod forwarded_helpers;
mod join;
mod plan_validation;
mod realizations;
mod source_lowering;
mod store_operations;
mod structural_types;
mod unit;
mod unit_join;

use super::{
    CheckedTrees, LoweredPsi, LoweredSourceCallOccurrence, LoweringError, PrimitiveType,
    ProofBundle, allocate_dense, block_id, edge_id, evidence_lowering, lookup_type_id,
    lower_installation_machine_service_ceiling, lower_root_service_reach, machine_id, operation_id,
    place_id, terminal_scalar_type, unsupported, value_id,
};
use crate::unit::dynamic_composed_unit::dynamic_lanes::{
    DynamicLoweringLane, lower_dynamic_composed_unit_machine,
};
use checked_trees::{
    CheckedBooleanExpression, CheckedDynamicScalarCallPlan, CheckedReboundDynamicScalarCallPlan,
    CheckedScalarExpression, CheckedStructuralAccess, CheckedUnitStructuralPathSegment,
};
use language_semantics::Multiplicity;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{
    Block, ClosedConformanceApplication, ClosedConformanceCallableResult, ClosedConformanceRow,
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, TerminalDirectDynamicDispatch,
    TerminalDynamicConformanceSelection, TerminalDynamicDescriptorArgument,
    TerminalDynamicDescriptorParameter, TerminalDynamicDescriptorSource,
    TerminalDynamicDispatchCatalog, TerminalIndirectDynamicDispatch, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalParameterDynamicDispatch,
    TerminalReboundDynamicDescriptor, Terminator, ValueDeclaration, VocabularyMarker,
};

pub(crate) fn lower_joined_dynamic_composed_unit_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedJoinedDynamicScalarCallPlan,
) -> Result<LoweredPsi, LoweringError> {
    join::lower(checked, plan)
}

pub(crate) fn lower_joined_dynamic_unit_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedJoinedDynamicUnitCallPlan,
) -> Result<LoweredPsi, LoweringError> {
    unit_join::lower(checked, plan)
}

pub(crate) fn lower_direct_dynamic_unit_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedDynamicUnitCallPlan,
) -> Result<LoweredPsi, LoweringError> {
    unit::lower_direct_dynamic_unit_machine(checked, plan)
}

pub(crate) fn lower_rebound_dynamic_unit_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedReboundDynamicUnitCallPlan,
) -> Result<LoweredPsi, LoweringError> {
    unit::lower_rebound_dynamic_unit_machine(checked, plan)
}

pub(crate) fn lower_direct_dynamic_composed_unit_machine(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    lower_dynamic_composed_unit_machine(checked, plan, DynamicLoweringLane::Direct)
}

pub(crate) fn lower_rebound_dynamic_composed_unit_machine(
    checked: &CheckedTrees,
    plan: &CheckedReboundDynamicScalarCallPlan,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    lower_dynamic_composed_unit_machine(
        checked,
        &plan.latest,
        DynamicLoweringLane::Rebound(&plan.initial),
    )
}

pub(crate) fn lower_stored_dynamic_composed_unit_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedStoredDynamicScalarCallPlan,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    lower_dynamic_composed_unit_machine(checked, &plan.call, DynamicLoweringLane::Stored(plan))
}
