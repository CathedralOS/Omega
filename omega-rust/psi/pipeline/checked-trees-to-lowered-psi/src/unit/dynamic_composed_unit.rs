//! Source-free lowering for bounded local dynamic calls.
//!
//! A never-rebound value lowers to a direct call. A value rebound exactly once
//! retains two selections and an indirect descriptor call. A checked forwarded
//! call additionally preserves its dynamic parameter, caller argument, and
//! parameter dispatch instead of composing the helper away. Every lane retains
//! the exact conformance application, source field subloan, requirement row,
//! and realization callable in Terminal custody.
//!
//! This file owns the one entry point, `lower_dynamic_dispatch_machine`: it
//! resolves the checked binding kind (direct, rebound, stored or joined) into
//! a lowering route and hands that route to the body for the plan's result
//! shape. `dynamic_lanes.rs` carries the lane shapes and the scalar-composed
//! single-call lowering, `unit.rs` its Unit-result counterpart, `join.rs` and
//! `unit_join.rs` the two-branch joins, and `continuation.rs` the scalar
//! result that immediately selects Unit control. `plan_validation.rs`
//! validates the exact plans, `source_lowering.rs` lowers sources and call
//! custody, `forwarded_helpers.rs` materializes forwarded helper chains,
//! `structural_types.rs` lowers structural types, `realizations.rs` collects
//! and materializes realizations, `applications.rs` lowers the conformance
//! applications and `store_operations.rs` lowers caller and realization
//! stores.

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
    CheckedBooleanExpression, CheckedDynamicScalarCallPlan, CheckedDynamicUnitCallPlan,
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

/// What one dynamic dispatch lowering retains about its source machines.
pub(crate) enum LoweredDynamicDispatch {
    /// A scalar-composed single-call binding retains the exact catalog its
    /// caller, realizations and forwarded helpers map onto.
    SourceMapped(crate::producer_result::SourceMappedLowered),
    /// Unit-result and joined bindings retain the caller and the realizations
    /// it selects without a per-machine catalog.
    EntryOnly {
        terminal: LoweredPsi,
        source_machines: Vec<symbols::SymbolHandle>,
    },
}

/// How one binding reaches its call(s) once the checked kind is resolved:
/// one call under a lowering lane, or the two branch calls of a join.
enum DynamicDispatchRoute<'a, Call> {
    Single {
        call: &'a Call,
        lane: DynamicLoweringLane<'a>,
    },
    Joined {
        control: &'a checked_trees::CheckedDynamicJoinControlPlan,
        when_true: &'a checked_trees::CheckedDynamicJoinBranchPlan<Call>,
        when_false: &'a checked_trees::CheckedDynamicJoinBranchPlan<Call>,
    },
}

fn dynamic_dispatch_route<Call>(
    binding: &checked_trees::CheckedDynamicBinding<Call>,
) -> DynamicDispatchRoute<'_, Call> {
    use checked_trees::CheckedDynamicBinding;
    match binding {
        CheckedDynamicBinding::Direct(call) => DynamicDispatchRoute::Single {
            call,
            lane: DynamicLoweringLane::Direct,
        },
        CheckedDynamicBinding::Rebound { initial, latest } => DynamicDispatchRoute::Single {
            call: latest,
            lane: DynamicLoweringLane::Rebound(initial),
        },
        CheckedDynamicBinding::Stored { descriptor, call } => DynamicDispatchRoute::Single {
            call,
            lane: DynamicLoweringLane::Stored(descriptor),
        },
        CheckedDynamicBinding::Joined {
            control,
            when_true,
            when_false,
        } => DynamicDispatchRoute::Joined {
            control,
            when_true,
            when_false,
        },
    }
}

/// Lower one checked dynamic dispatch: resolve the binding kind into its
/// lowering route, then lower that route under the plan's result shape.
pub(crate) fn lower_dynamic_dispatch_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedDynamicDispatchPlan,
) -> Result<LoweredDynamicDispatch, LoweringError> {
    use checked_trees::{CheckedDynamicBindingKind, CheckedDynamicDispatchPlan};
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .calls
        .iter()
        .filter(|candidate| *candidate == plan)
        .count()
        != 1
    {
        return unsupported(match (plan, plan.binding_kind()) {
            (_, CheckedDynamicBindingKind::Stored) => {
                "stored dynamic descriptor drifted from checked aggregate custody"
            }
            (CheckedDynamicDispatchPlan::Scalar(_), CheckedDynamicBindingKind::Joined) => {
                "joined dynamic control plan drifted from checked custody"
            }
            (CheckedDynamicDispatchPlan::Unit(_), CheckedDynamicBindingKind::Joined) => {
                "joined dynamic Unit control plan drifted from checked custody"
            }
            _ => "dynamic dispatch plan drifted from checked custody",
        });
    }
    let caller = plan.caller_machine();
    Ok(match plan {
        CheckedDynamicDispatchPlan::Scalar(binding) => match dynamic_dispatch_route(binding) {
            DynamicDispatchRoute::Single { call, lane } => LoweredDynamicDispatch::SourceMapped(
                lower_dynamic_composed_unit_machine(checked, call, lane)?,
            ),
            DynamicDispatchRoute::Joined {
                control,
                when_true,
                when_false,
            } => LoweredDynamicDispatch::EntryOnly {
                terminal: join::lower(checked, control, when_true, when_false)?,
                source_machines: joined_source_machines(
                    caller,
                    [
                        when_true.call.realization_machine,
                        when_false.call.realization_machine,
                    ],
                ),
            },
        },
        CheckedDynamicDispatchPlan::Unit(binding) => match dynamic_dispatch_route(binding) {
            DynamicDispatchRoute::Single { call, lane } => LoweredDynamicDispatch::EntryOnly {
                terminal: unit::lower_dynamic_unit_machine(checked, call, lane)?,
                source_machines: vec![caller, call.realization_machine],
            },
            DynamicDispatchRoute::Joined {
                control,
                when_true,
                when_false,
            } => LoweredDynamicDispatch::EntryOnly {
                terminal: unit_join::lower(checked, control, when_true, when_false)?,
                source_machines: joined_source_machines(
                    caller,
                    [
                        when_true.call.realization_machine,
                        when_false.call.realization_machine,
                    ],
                ),
            },
        },
    })
}

fn joined_source_machines(
    caller: symbols::SymbolHandle,
    realizations: [symbols::SymbolHandle; 2],
) -> Vec<symbols::SymbolHandle> {
    let mut sources = vec![caller];
    sources.extend(realizations);
    sources.sort_by_key(|machine| (machine.arena_index(), machine.generation()));
    sources.dedup();
    sources
}
