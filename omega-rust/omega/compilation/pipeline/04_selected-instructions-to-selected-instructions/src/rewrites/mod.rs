//! Optimizer module role: stage group. Selected-CFG rewrites and their replay
//! evidence, split by whether the stage executes them.
//!
//! The executed set is what `optimize_selected_instructions` reaches:
//! `catalog` is the stage rule catalog it admits selections through;
//! `selected_lowering` and `literal_folds` execute the selected-lowering
//! slice (literal folds under the pair-rule descriptors); `allocation_recovery`
//! executes the allocation-recovery slice through `fixed_view` (fixed-view
//! copies and fixed precolored segment homes) and the pressure
//! rematerialization it owns; `pre_allocation` executes the pre-allocation
//! slice through `copy_removal` (same-block copy removal),
//! `redundant_extension` (carrier extensions whose producer already
//! normalizes), `address_fold` (displacement consumers reading an
//! `AddressOffset` result), `constant_boolean` (a `MaterializeBoolean*`
//! whose reached compare is compile-time constant), and `constant_branch`
//! (a conditional-branch terminator whose reached compare is compile-time
//! constant), `dead_compare` (a flag-publisher compare whose
//! publication dies unread, or repeats a shadow's equivalent
//! publication), and `copied_call_operand` (a direct internal call whose
//! `Use` arguments read a retained `CopyI64` destination, rebound to the
//! copy's source), and `load_forwarding` (a byte-exact retained store whose
//! independently reconstructed alias and path facts authorize a later read); `runtime_split`, `runtime_rematerialization` and
//! `runtime_spill` are the
//! recovery rewrites register assignment replays, in cost order. `block_edges`,
//! `condition_state` and `window_hazards` are the block-boundary,
//! flag-state and hazard vocabulary the executed set shares.
//!
//! `unexecuted` holds the design record of every other rewrite family,
//! rostered against the board row that owns its execution, reached by no
//! production route, and compiled nowhere. See its module doc for the
//! disposition roster.

/// Every validated rewrite exposes the same three accessors: the
/// transformed plan borrowed and shared, and the receipt the rewrite
/// replayed. One declarative row per validated type expands that impl.
macro_rules! validated_transformed_receipt {
    ($name:ident, $receipt:ident) => {
        impl $name {
            pub fn transformed(&self) -> &SelectedInstructionPlan {
                &self.transformed
            }

            pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
                Arc::clone(&self.transformed)
            }

            pub const fn receipt(&self) -> &$receipt {
                &self.receipt
            }
        }
    };
}

/// Every selected-rewrite receipt exposes the same four identities: the
/// selected plan before and after, plus the optimization-unit and fuel
/// identities the rewrite carried. One declarative row per receipt expands
/// those accessors; receipts with extra accessors keep them in their own
/// impl.
macro_rules! receipt_selected_identities {
    ($name:ident) => {
        impl $name {
            /// Identity of the selected plan the rewrite consumed.
            pub const fn source_selected(&self) -> SelectedInstructionPlanIdentity {
                self.source_selected
            }
            /// Identity of the selected plan the rewrite produced.
            pub const fn transformed_selected(&self) -> SelectedInstructionPlanIdentity {
                self.transformed_selected
            }
            /// Optimization unit the rewrite's evidence belongs to.
            pub const fn optimization_unit(&self) -> OptimizationUnitIdentity {
                self.optimization_unit
            }
            /// Fuel schedule the rewrite ran under.
            pub const fn fuel_schedule(&self) -> FuelScheduleIdentity {
                self.fuel_schedule
            }
        }
    };
}

/// A rule that declares a consumer and a producer `MachineSemanticKind`
/// answers the same kind-probe twice: whether an instruction kind is the
/// declared consumer or producer. One row per rule expands both probes.
macro_rules! declared_kind_matchers {
    ($name:ident) => {
        impl $name {
            /// Whether `kind` is the consumer this rule declares.
            pub(super) fn matches_consumer(&self, kind: SelectedInstructionKind) -> bool {
                machine_semantic_kind(kind) == self.consumer
            }

            /// Whether `kind` is the producer this rule declares.
            pub(super) fn matches_producer(&self, kind: SelectedInstructionKind) -> bool {
                machine_semantic_kind(kind) == self.producer
            }
        }
    };
}

mod address_fold;
mod allocation_recovery;
mod block_edges;
mod catalog;
mod condition_state;
mod constant_boolean;
mod constant_branch;
mod copied_call_operand;
mod copy_removal;
mod dead_compare;
mod fixed_view;
mod literal_folds;
mod load_forwarding;
mod operand_probes;
mod place_storage;
mod pre_allocation;
mod redundant_extension;
mod runtime_rematerialization;
mod runtime_spill;
mod runtime_split;
mod selected_lowering;
mod selected_operands;
pub mod unexecuted;
mod window_hazards;

pub(crate) use address_fold::{AddressFoldError, ValidatedAddressFold};
pub(crate) use address_fold::{
    fold_selected_address, measured_steps as address_fold_measured_steps,
};
pub(crate) use allocation_recovery::materialize_fixed_view_copies;
pub(crate) use allocation_recovery::pressure_rematerialization_identity;
pub(crate) use allocation_recovery::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogEntry, FixedViewCopyError,
    FixedViewCopyValidationReceipt, FunctionPressureRematerialization,
    PressureRematerializationAction, PressureRematerializationPlan,
    PressureRematerializationRewrite, PressureRematerializationValidationReceipt,
};
pub use allocation_recovery::{
    AllocationRecoveryRuleCatalogError, FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan,
    FixedViewCopyPolicy, FixedViewCopySourceEvidence, PressureRematerializationError,
    PressureRematerializationPolicy, ValidatedFixedViewCopies, ValidatedPressureRematerialization,
    fixed_view_copy_identity, rematerialize_selected_active_resident,
    selected_allocation_recovery_rule,
};
pub(crate) use constant_boolean::{ConstantBooleanError, ValidatedConstantBoolean};
pub(crate) use constant_boolean::{
    fold_selected_constant_boolean, measured_steps as constant_boolean_measured_steps,
    surface_sizes as constant_boolean_surface_sizes,
};
pub(crate) use constant_branch::{ConstantBranchError, ValidatedConstantBranch};
pub(crate) use constant_branch::{
    fold_selected_constant_branch, measured_steps as constant_branch_measured_steps,
    surface_sizes as constant_branch_surface_sizes,
};
pub(crate) use copied_call_operand::{CopiedCallOperandError, ValidatedCopiedCallOperand};
pub(crate) use copied_call_operand::{
    copied_call_operand_measured_steps, fold_selected_copied_call_operand,
};
// Reached as `crate::rewrites::<name>` by the catalog's own tests and by
// `selected_optimization`'s admission decision.
pub(crate) use catalog::{SELECTED_STAGE_RULE_CATALOG, SelectedStageRuleRows};
pub(crate) use copy_removal::{CopyRemovalError, ValidatedCopyRemoval};
pub(crate) use copy_removal::{
    measured_steps as copy_removal_measured_steps, remove_selected_copy,
};
pub(crate) use dead_compare::{
    DeadCompareError, EquivalentCompareError, RedundantCompareError, ValidatedDeadCompare,
    ValidatedEquivalentCompare, ValidatedRedundantCompare,
};
pub(crate) use dead_compare::{
    equivalent_compare_measured_steps, measured_steps as dead_compare_measured_steps,
    redundant_compare_measured_steps, remove_dead_compare, remove_equivalent_compare,
    remove_redundant_compare,
};
pub use fixed_view::{
    OptimizedFixedPrecoloredSegmentHomeCustodyError, OptimizedFixedViewCopyCustodyError,
    probe_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_precolored_segment_homes,
    stage_optimized_fixed_view_copies,
};
pub(crate) use fixed_view::{
    StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
    StagedOptimizedFixedPrecoloredSegmentHomes, StagedOptimizedFixedViewCopies,
    StagedOptimizedFixedViewCopyCustodyReceipt,
};
pub use literal_folds::{
    OptimizedLiteralFoldCustodyError, StagedOptimizedLiteralFoldCustodyReceipt,
    StagedOptimizedLiteralFolds, StagedSelectedLoweringOptimizationCustodyReceipt,
    StagedSelectedLoweringOptimizationRun,
};
pub(crate) use literal_folds::{
    StagedOptimizedLiteralFoldAttempt, StagedOptimizedLiteralFoldAttemptReceipt,
    StagedOptimizedLiteralFoldIterationReceipt, StagedOptimizedLiteralFoldStep,
    run_selected_lowering_optimizations, validate_selected_lowering_optimization_custody,
};
pub use pre_allocation::{
    OptimizedPreAllocationCustodyError, StagedPreAllocationOptimizationCustodyReceipt,
    StagedPreAllocationOptimizationRun,
};
pub(crate) use pre_allocation::{
    PRE_ALLOCATION_RULE_CATALOG, PreAllocationPolicy, PreAllocationRuleCatalogEntry,
    PreAllocationTransformationIdentity, StagedOptimizedPreAllocationAttempt,
    StagedOptimizedPreAllocationAttemptReceipt, StagedOptimizedPreAllocationIterationReceipt,
    StagedOptimizedPreAllocationStep, ValidatedPreAllocationTransformation,
    resolve_pre_allocation_rules, run_pre_allocation_optimizations,
    validate_pre_allocation_optimization_custody,
};
pub(crate) use redundant_extension::{RedundantExtensionError, ValidatedRedundantExtension};
pub(crate) use redundant_extension::{
    measured_steps as redundant_extension_measured_steps, remove_selected_redundant_extension,
};
pub use runtime_rematerialization::{
    RuntimeRematerializationError, ValidatedRuntimeRematerialization,
    rematerialize_selected_runtime_value,
};

pub use runtime_spill::{
    RuntimeSpillError, RuntimeSpillSpanPolicy, ValidatedRuntimeSpill, spill_selected_runtime_value,
    spill_selected_runtime_value_with_crossing_status, spill_selected_runtime_value_with_span_policy,
};
pub use runtime_split::{RuntimeSplitError, ValidatedRuntimeSplit, split_selected_runtime_value};
pub(crate) use selected_lowering::enabled_pair_rules;
pub(crate) use selected_lowering::fold_selected_incoming_literal;
pub(crate) use selected_lowering::literal_fold_identity;
// Reached as `crate::rewrites::validate_literal_fold` by the literal-fold
// test corpus.
pub(crate) use selected_lowering::{
    FunctionLiteralFold, LiteralFoldAction, LiteralFoldError, LiteralFoldPlan, LiteralFoldPolicy,
    LiteralFoldValidationReceipt, PairOperandShape, PairResultDisposition, PairUnitDefRelation,
    SELECTED_LOWERING_RULE_CATALOG, SelectedInstructionPairRule, SelectedLoweringRuleCatalogEntry,
    SelectedLoweringRuleCatalogError, resolve_selected_lowering_rules,
};
pub use selected_lowering::{LiteralFoldIdentity, ValidatedLiteralFold};

/// Explicit applicability of the currently architecture-independent rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegisterAllocationRuleTargetApplicability {
    TargetIndependent,
}

pub(crate) use load_forwarding::{
    StoredLoadForwardingError, ValidatedStoredLoadForwarding, forward_selected_stored_load,
};

use crate::rewrites::block_edges::function_at;
use crate::rewrites::catalog::selected_stage_catalog_contains;
use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationPhaseMismatch,
    OptimizationPhaseSelections, OptimizationSelections, OptimizationWorkBudget,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedOperand, VirtualRegisterId,
};

/// The scan surface every single-function rewrite's measured-step contract
/// charges first: one step per block plus one per instruction of the
/// rewritten function, and one step per plan function. Function storage is
/// shared copy-on-write, so the proposal's plan clone, the transformed plan's
/// identity and replay's restored-plan comparison each visit an unchanged
/// function's shared entry once — they rehash and compare only the detached
/// rewritten body. Charging every instruction of the plan instead made a
/// required rewrite of a twelve-block function exceed the pass budget once
/// the program passed a million instructions. Replay over separately
/// allocated storage still compares the unshared bodies by content; that
/// comparison is linear in the input it was handed and is not rewrite work.
pub(crate) fn rewrite_scan(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
) -> Option<usize> {
    function_scan(function)?.checked_add(plan.functions.len())
}

/// The ceilings for a rewrite recovery cannot do without — a spill, split or
/// rematerialization of one value. They grow with the rewritten function's
/// extent: the rewrite scan plus every instruction and terminator operand,
/// memory access and successor-edge transport the audit reads
/// (`OptimizationWorkBudget::for_extent`). The audit's work is linear in that
/// extent, so no function is too large to relieve, while work superlinear in
/// it still exhausts the ceilings. Leaving the transports out capped the
/// functions a required rewrite could process by their edge surface: the
/// product typing harness's largest function carried 33,342 transports
/// against 14,703 operands, and the fixed-occurrence audit's per-transport
/// charge alone exceeded one reference extent's ceiling.
pub(crate) fn required_rewrite_budget(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    budget: OptimizationWorkBudget,
) -> Option<OptimizationWorkBudget> {
    let operands = function.blocks.total_operand_count()?;
    let extent = rewrite_scan(plan, function)?
        .checked_add(operands)?
        .checked_add(function.memory_accesses.len())?
        .checked_add(function_transports(function)?)?;
    Some(budget.for_extent(u64::try_from(extent).ok()?))
}

/// Every value a function's successor edges transport: each register and
/// structural binding, and a structural case's tag with each of its
/// payloads. An audit that reads the edges reads each of these once.
pub(crate) fn function_transports(function: &SelectedFunction) -> Option<usize> {
    function.blocks.total_transport_count()
}

/// One step per block plus one per instruction in one function — the
/// scan term every measured-step contract charges before its own audit.
pub(crate) fn function_scan(function: &SelectedFunction) -> Option<usize> {
    function.blocks.iter().try_fold(0usize, |total, block| {
        total.checked_add(block.instructions.len())?.checked_add(1)
    })
}

/// Charge a measured-step total against the budget: `overflow` when the
/// count cannot fit the measured-step width, `exceeded` when it passes the
/// family's own validation cap.
pub(crate) fn charge_steps<E>(
    steps: usize,
    budget: OptimizationWorkBudget,
    overflow: impl FnOnce() -> E,
    exceeded: impl FnOnce() -> E,
) -> Result<(), E> {
    if u64::try_from(steps).map_err(|_| overflow())? > budget.validation_steps() {
        return Err(exceeded());
    }
    Ok(())
}

/// The cone-propagation bound every walked family's measured-step contract
/// derives from a function and its successor edges — the audits share this
/// arithmetic verbatim and then add their own per-unit and setup terms.
pub(crate) struct ConePropagation {
    pub(crate) function_scan: usize,
    pub(crate) edge_count: usize,
    pub(crate) elements: usize,
    pub(crate) per_unit_base: usize,
    pub(crate) walk_setup_base: usize,
}

/// Bound the shared entry-set propagation a cone walk performs: elements
/// cover every event site plus the unknown marker, pops stay under
/// `blocks × (elements + 1)`, and each pop visits its out-edges at the
/// widest terminator's union cost.
pub(crate) fn cone_propagation<E>(
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    overflow: impl Fn() -> E,
) -> Result<ConePropagation, E> {
    let function_scan = function_scan(function).ok_or_else(|| overflow())?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or_else(|| overflow())?;
    let block_count = function.blocks.len();
    let elements = function_scan.checked_add(1).ok_or_else(|| overflow())?;
    let pops = block_count
        .checked_mul(elements.checked_add(1).ok_or_else(|| overflow())?)
        .ok_or_else(|| overflow())?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit_base = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .ok_or_else(|| overflow())?;
    let walk_setup_base = edge_count
        .checked_mul(block_count.checked_add(1).ok_or_else(|| overflow())?)
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .ok_or_else(|| overflow())?;
    Ok(ConePropagation {
        function_scan,
        edge_count,
        elements,
        per_unit_base,
        walk_setup_base,
    })
}

/// The plan and the function a family audits, once the plan's target has
/// matched the environment's — both call sites bind the same pair, with the
/// family's own `SourceMismatch` marking either refusal.
pub(crate) fn plan_function<'source, E>(
    source: &'source impl crate::ValidatedSelectedAnalysis,
    function_index: usize,
    environment: &ValidatedTargetRegisterEnvironment,
    mismatch: impl Fn() -> E + Copy,
) -> Result<(&'source SelectedInstructionPlan, &'source SelectedFunction), E> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(mismatch());
    }
    Ok((plan, function_at(plan, function_index, mismatch())?))
}

/// The four receipt identities every family seals the same way: source,
/// transformed, optimization unit, and fuel schedule, followed by the
/// family's own recorded fields.
macro_rules! receipt {
    ($receipt:ident, $source:expr, $proposed:expr $(, $field:ident : $value:expr)* $(,)?) => {
        $receipt {
            source_selected: $source.selected_identity(),
            transformed_selected: selected_instructions::selected_instruction_plan_identity(&$proposed),
            optimization_unit: $source.optimization_unit_identity(),
            fuel_schedule: $source.fuel_schedule_identity(),
            $($field: $value,)*
        }
    };
}
pub(crate) use receipt;

/// The whole seal body every family ends with: a receipt of the four shared
/// identities plus the family's recorded fields, beside the proposed plan in
/// its arc.
macro_rules! sealed {
    ($validated:ident, $receipt:ident, $source:expr, $proposed:expr $(, $field:ident : $value:expr)* $(,)?) => {
        $validated {
            receipt: crate::rewrites::receipt!($receipt, $source, $proposed $(, $field : $value)*),
            transformed: std::sync::Arc::new($proposed)}
    };
}
pub(crate) use sealed;

pub(crate) fn instruction(
    id: SelectedInstructionId,
    kind: SelectedInstructionKind,
    row: &RegisterInstructionConstraint,
    registers: &[VirtualRegisterId],
) -> SelectedInstruction {
    SelectedInstruction {
        id,
        kind,
        constraint: row.key,
        operands: row
            .operands
            .iter()
            .zip(registers)
            .map(|(operand, register)| SelectedOperand {
                operand: operand.operand,
                virtual_register: *register,
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: Default::default(),
    }
}

/// Whether `register`'s only occurrence in `function` is one operand —
/// the custody a dropped `Def` requires across producer rewrites: the
/// operand itself is that one occurrence, and any other operand position,
/// terminator operand, or successor transport naming the register leaves a
/// surviving read or second definition.
pub(crate) fn register_occurs_once(
    function: &selected_instructions::SelectedFunction,
    register: selected_instructions::VirtualRegisterId,
) -> bool {
    use crate::rewrites::block_edges::terminator_successors;
    use selected_instructions::{SelectedStructuralTransport, terminator_instruction};

    let mut occurrences = 0_usize;
    for block in &function.blocks {
        for instruction in block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(&block.terminator)))
        {
            occurrences += instruction
                .operands
                .iter()
                .filter(|operand| operand.virtual_register == register)
                .count();
        }
        for successor in terminator_successors(&block.terminator) {
            occurrences += successor
                .structural_bindings
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.transport,
                        SelectedStructuralTransport::WholeValue { argument, .. }
                        | SelectedStructuralTransport::ScalarReference { base: selected_instructions::SelectedAddressBase::Register(argument), .. }
                        | SelectedStructuralTransport::Address { base: selected_instructions::SelectedAddressBase::Register(argument), .. }
                            if argument == register
                    )
                })
                .count();
            if let Some(case) = &successor.structural_case {
                occurrences +=
                    case.payloads
                        .iter()
                        .filter(|payload| {
                            match &payload.transport {
                        selected_instructions::SelectedCasePayloadTransport::Unused => false,
                        selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                            parameter} => *parameter == register,
                        selected_instructions::SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter} => *argument == register || *parameter == register}
                        })
                        .count();
            }
            occurrences += successor
                .bindings
                .iter()
                .filter(|binding| match &binding.transport {
                    selected_instructions::SelectedValueTransport::Unused => false,
                    selected_instructions::SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } => *argument == register || *parameter == register,
                })
                .count();
        }
    }
    occurrences == 1
}

/// Shared byte-envelope for the selected-rewrite receipt identities: the
/// domain tag, the source and transformed fingerprints, the optimization-unit
/// fingerprint, then the per-rewrite tail feeds.
pub(crate) fn rewrite_bytes(
    tag: &'static [u8],
    source_selected: impl AsRef<[u8]>,
    transformed_selected: impl AsRef<[u8]>,
    optimization_unit: impl AsRef<[u8]>,
    tail: impl FnOnce(&mut Vec<u8>),
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(tag);
    bytes.extend_from_slice(source_selected.as_ref());
    bytes.extend_from_slice(transformed_selected.as_ref());
    bytes.extend_from_slice(optimization_unit.as_ref());
    tail(&mut bytes);
    bytes
}

/// Shared boundary-settlement shift for rewrites that remove one instruction:
/// every settlement past the removed index in the same block shifts down one.
pub(crate) fn shifted_boundary_settlements<E>(
    function: &selected_instructions::SelectedFunction,
    block: selected_instructions::SelectedBlockId,
    removed: usize,
    source_mismatch: impl Fn() -> E,
    overflow: impl Fn() -> E,
) -> Result<Vec<selected_instructions::SelectedBoundarySettlement>, E> {
    let body = function
        .block(block)
        .ok_or_else(|| source_mismatch())?
        .instructions
        .len();
    let mut shifted = function.boundary_settlements.clone();
    for settlement in &mut shifted {
        if settlement.block != block {
            continue;
        }
        let position = settlement.instruction_index as usize;
        if position > body {
            return Err(source_mismatch());
        }
        if position > removed {
            settlement.instruction_index = u32::try_from(position - 1).map_err(|_| overflow())?;
        }
    }
    Ok(shifted)
}

/// Shared cone-audited step measurement for the constant folds: the
/// cone-propagation bases plus the per-unit surface, the reach scan and the
/// rewrite scan, all saturating through the caller's overflow diagnostic.
/// Both fold admissions charge this total and both fold validators publish
/// it, so the fold's measured-step contract is derived once.
pub(crate) fn constant_fold_steps<E>(
    plan: &selected_instructions::SelectedInstructionPlan,
    function: &selected_instructions::SelectedFunction,
    successors: &[Vec<usize>],
    implicit_uses: usize,
    flag_universe: usize,
    surface: usize,
    defs: usize,
    overflow: impl Fn() -> E + Copy,
) -> Result<usize, E> {
    let cone = crate::rewrites::cone_propagation(function, successors, overflow)?;
    let per_unit = Some(cone.per_unit_base)
        .and_then(|total| total.checked_add(flag_universe.checked_add(surface)?.checked_add(defs)?))
        .ok_or_else(overflow)?;
    let walk_setup = Some(cone.walk_setup_base)
        .and_then(|total| total.checked_add(flag_universe))
        .and_then(|total| total.checked_add(surface))
        .ok_or_else(overflow)?;
    let function_scan = cone.function_scan;
    let reach_scan = implicit_uses
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or_else(overflow)?;
    crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| {
            total
                .checked_add(function_scan.checked_mul(2)?)?
                .checked_add(reach_scan)
        })
        .ok_or_else(overflow)
}

/// Shared phase validation for rewrite-stage rule catalogs: the required
/// phase must be present, non-empty, and every selected optimization must
/// belong to the stage's catalog.
pub(crate) fn required_stage_phase<E>(
    selections: &OptimizationPhaseSelections,
    phase_kind: OptimizationExecutionPhase,
    wrong_phase: impl FnOnce(OptimizationPhaseMismatch) -> E,
    missing_selection: impl FnOnce() -> E,
    unsupported: impl FnOnce(Optimization) -> E,
) -> Result<OptimizationSelections, E> {
    let phase = selections.require_phase(phase_kind).map_err(wrong_phase)?;
    if phase.is_empty() {
        return Err(missing_selection());
    }
    if let Some(unsupported_selection) = phase
        .as_slice()
        .iter()
        .find(|selected| !selected_stage_catalog_contains(phase_kind, **selected))
    {
        return Err(unsupported(*unsupported_selection));
    }
    Ok(phase.clone())
}

pub use fixed_occurrences::{
    FixedOccurrencesError, ValidatedFixedOccurrences, materialize_selected_fixed_occurrences,
};
mod fixed_occurrences;
