//! Normalized boundary calling and machine-state plans.
//!
//! The existing encoders still realize these policies directly. This module is
//! the semantic seam they are migrating toward: policy + signature produces a
//! deterministic `CallPlan`; inbound roots pair it with a `StatePlan`.
//! Backend footprint evidence is deliberately a different artifact.
//!
//! `vocabulary.rs` names the plan parts, `call_plan_evaluation.rs` derives a
//! call plan per convention, `boundary_entry.rs` pairs it with a state plan,
//! `state_footprints.rs` validates what an entry may touch,
//! `plan_identity.rs` fingerprints plans and `diagnostics.rs` reports failures.

mod boundary_entry;
mod call_plan_evaluation;
mod diagnostics;
mod plan_identity;
mod state_footprints;
#[cfg(test)]
mod tests;
mod vocabulary;

pub use boundary_entry::{
    BoundaryEntryPlan, BoundaryPlanResult, CallingPolicyRejection, ValidatedBoundaryEntryPlan,
    evaluate_ordinary_boundary_entry_plan, validate_boundary_entry_plan,
    validate_boundary_entry_plan_with_callback_materializations, validate_boundary_plan_result,
};
pub use call_plan_evaluation::{
    evaluate_call_plan,
};
pub(crate) use call_plan_evaluation::{ validate_call_plan};
pub use diagnostics::{ PlanDiagnostic};
pub(crate) use diagnostics::{BoundaryPlanDiagnostic};
pub use state_footprints::{ StateFootprintEvidence,
};
pub(crate) use state_footprints::{
    ProviderExitRealization};
pub use vocabulary::{
    CallPlan, CallSignature, CallingPolicy, EntryControl,
    EntryStack, IndirectPointerLocation, MachineRegime, MachineRegister, MachineState,
    MachineStateSet, Preemption, RegisterSet, StatePlan, SystemVEightbyteClass, ValueClass,
    ValueLocation, ValuePlacement, ValueShape, byte_size_at_offset_zero,
    encode_state_plan_identity, gpr_code,
};
