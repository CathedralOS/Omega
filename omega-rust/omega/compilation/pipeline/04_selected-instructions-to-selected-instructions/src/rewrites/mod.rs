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

/// The one row of a family's declared-pair table that names a given
/// (producer, consumer) kind combination: several matching rows is a catalog
/// defect and refuses rather than preferring table order. One row per family
/// expands its own lookup over its own table.
macro_rules! declared_pair_lookup {
    ($lookup:ident, $table:ident, $rule:ty) => {
        /// The declared pair for one (producer, consumer) kind combination, or
        /// none when the family declares no such relationship — several rules
        /// matching one combination is a catalog defect and refuses rather
        /// than preferring table order.
        pub(super) fn $lookup(
            producer: MachineSemanticKind,
            consumer: MachineSemanticKind,
        ) -> Option<&'static $rule> {
            let mut matches = $table
                .iter()
                .filter(|rule| rule.producer() == producer && rule.consumer() == consumer);
            let rule = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            Some(rule)
        }
    };
}

mod address_fold;
mod allocation_recovery;
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
mod place_storage;
mod pre_allocation;
mod redundant_extension;
mod runtime_rematerialization;
mod runtime_spill;
mod runtime_split;
mod selected_lowering;
mod settlement_gaps;
pub mod unexecuted;
mod window_hazards;

pub(crate) use address_fold::{AddressFoldError, ValidatedAddressFold};
pub(crate) use address_fold::{
    fold_selected_address, measured_steps as address_fold_measured_steps,
};
pub(crate) use allocation_recovery::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogEntry, FixedViewCopyError,
    FixedViewCopyValidationReceipt, FunctionPressureRematerialization,
    PressureRematerializationAction, PressureRematerializationPlan,
    PressureRematerializationRewrite, PressureRematerializationValidationReceipt,
    materialize_fixed_view_copies, pressure_rematerialization_identity,
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
pub(crate) use copied_call_operand::{
    CopiedCallOperandError, ValidatedCopiedCallOperand, copied_call_operand_measured_steps,
    fold_selected_copied_call_operand,
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
    spill_selected_runtime_value_with_crossing_status,
    spill_selected_runtime_value_with_span_policy,
};
pub use runtime_split::{RuntimeSplitError, ValidatedRuntimeSplit, split_selected_runtime_value};
pub(crate) use selected_lowering::{
    enabled_pair_rules, fold_selected_incoming_literal, literal_fold_identity,
};
// Reached as `crate::rewrites::validate_literal_fold` by the literal-fold
// test corpus.
pub(crate) use selected_lowering::{
    FunctionLiteralFold, LiteralFoldAction, LiteralFoldError, LiteralFoldPlan, LiteralFoldPolicy,
    LiteralFoldValidationReceipt, PairResultDisposition, PairUnitDefRelation,
    SELECTED_LOWERING_RULE_CATALOG, SelectedInstructionPairRule, SelectedLoweringRuleCatalogEntry,
    SelectedLoweringRuleCatalogError, resolve_selected_lowering_rules,
};
pub use selected_lowering::{LiteralFoldIdentity, ValidatedLiteralFold};

// The primitives allocation recovery shares live in the analyses package.
pub(crate) use selected_instruction_analyses::rewriting::*;
pub(crate) use selected_instruction_analyses::sealed;

pub(crate) use load_forwarding::{
    StoredLoadForwardingError, ValidatedStoredLoadForwarding, forward_selected_stored_load,
};

use crate::rewrites::catalog::selected_stage_catalog_contains;
use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationPhaseMismatch,
    OptimizationPhaseSelections, OptimizationSelections,
};
use selected_instructions::SelectedFunction;

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
