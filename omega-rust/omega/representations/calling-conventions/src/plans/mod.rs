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
    evaluate_darwin_aapcs64_variadic_boundary_entry_plan, evaluate_freestanding_program_entry_plan,
    evaluate_ordinary_boundary_entry_plan, validate_boundary_entry_plan,
    validate_boundary_entry_plan_with_callback_materializations, validate_boundary_plan_result,
};
pub use call_plan_evaluation::{
    evaluate_call_plan, evaluate_darwin_aapcs64_variadic_call_plan, validate_call_plan,
};
pub use diagnostics::{BoundaryPlanDiagnostic, PlanDiagnostic};
pub use state_footprints::{
    ProviderExitRealization, StateFootprintEvidence, compose_state_footprints,
    validate_call_return_mechanics_footprint, validate_composed_state_footprint,
    validate_outbound_call_footprint, validate_provider_exit_realization,
    validate_runtime_value_guard_footprint, validate_state_footprint,
};
pub use vocabulary::{
    CallPlan, CallSignature, CallingPolicy, ConcreteVariadicCallSignature, EntryControl,
    EntryStack, IndirectPointerLocation, MachineRegime, MachineRegister, MachineState,
    MachineStateSet, Preemption, RegisterSet, StatePlan, SystemVEightbyteClass, ValueClass,
    ValueLocation, ValuePlacement, ValueShape, encode_state_plan_identity,
};
