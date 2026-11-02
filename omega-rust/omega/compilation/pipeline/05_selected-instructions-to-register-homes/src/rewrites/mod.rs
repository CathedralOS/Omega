//! Optimizer module role: stage group. Selected-instruction rewrites and retained replay evidence.

mod rematerialization;

// Allocation recovery: the rewrites register assignment replays, and the
// fixed-view and pressure-rematerialization slice it executes. They build on
// the selected-program rewrite primitives the analyses package shares with
// selected optimization.
pub(crate) use selected_instruction_analyses::rewriting::*;
pub(crate) use selected_instruction_analyses::sealed;

mod allocation_recovery;
mod fixed_occurrences;
mod fixed_view;
mod runtime_rematerialization;
mod runtime_spill;
mod runtime_split;
mod settlement_gaps;

pub(crate) use allocation_recovery::FixedViewCopyDestination;
pub use allocation_recovery::{
    FixedViewCopy, FixedViewCopyPlan, FixedViewCopyPolicy, FixedViewCopySourceEvidence,
    PressureRematerializationError, PressureRematerializationPolicy, ValidatedFixedViewCopies,
    ValidatedPressureRematerialization, fixed_view_copy_identity,
    rematerialize_selected_active_resident,
};
pub(crate) use allocation_recovery::{
    FixedViewCopyError, FixedViewCopyValidationReceipt, FunctionPressureRematerialization,
    PressureRematerializationAction, PressureRematerializationPlan,
    PressureRematerializationRewrite, PressureRematerializationValidationReceipt,
    materialize_fixed_view_copies, pressure_rematerialization_identity,
};
pub use fixed_occurrences::{
    FixedOccurrencesError, ValidatedFixedOccurrences, materialize_selected_fixed_occurrences,
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

pub(crate) use rematerialization::complete_optimized_active_resident_rematerialization;

pub(crate) use rematerialization::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedActiveResidentRematerializationPressure,
    stage_optimized_active_resident_rematerialization_pressure,
};
