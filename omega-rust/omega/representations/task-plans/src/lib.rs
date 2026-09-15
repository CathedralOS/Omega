//! Provider-independent task activation plans and lifecycle accounting.
//!
//! An activation plan describes one fixed, nonmoving stack, the canonical
//! semantic suspension crossings, and only the CPU/thread preservation those
//! crossings demand. Executor selection consumes exact per-axis checked or
//! admitted evidence; this crate deliberately does not publish a generalized
//! runtime behavior record.
//!
//! `activation_plans.rs` is the root: the plan and its validators.
//! `activation_plan_facts.rs` carries what a provider publishes about a plan,
//! `executor_selection.rs` binds an executor to a plan's preservation axes,
//! `runtime_invocation.rs` receipts one activation, `lifecycle_ledger.rs`
//! accounts for claims until settlement, `stack_composition.rs` projects
//! WCSU stack plans, `identities.rs` holds every coordinate,
//! `report_fingerprints.rs` the compact report values and `diagnostic.rs`
//! the failure type.

mod activation_plan_facts;
mod activation_plans;
mod diagnostic;
mod executor_selection;
mod identities;
mod lifecycle_ledger;
mod report_fingerprints;
mod runtime_invocation;
mod stack_composition;
#[cfg(test)]
mod tests;

pub use activation_plan_facts::{
    SelectedTaskRuntimeProviderFact, TaskActivationPlanFact, TaskActivationPlanSet,
    TaskSpecializationCommitment, TaskStartOperation,
};
pub use activation_plans::{
    ActivationCarryObligations, ActivationPlanCandidate, CanonicalSuspensionCrossing, StackPlan,
    ValidatedActivationPlan, validate_activation_plan, validate_wcsu_activation_plan,
};
pub use diagnostic::TaskPlanDiagnostic;
pub use executor_selection::{
    ExecutorPreservationAxis, ExecutorPreservationEvidence, ExecutorSelectionCandidate,
    ValidatedExecutorSelection, validate_executor_selection,
};
pub use identities::{
    ActivationInstanceId, ActivationPlanId, AdmittedStackContributionReportId, CallingPlanId,
    ExecutorPreservationEvidenceId, ExecutorSelectionId, MachineContractId, MachineEntryId,
    SameStackContributionAdmissionReceiptId, StackPlanProjectionId, StackRepresentationId,
    TaskLifecycleClaimId, TaskRuntimeId, TaskRuntimeInstanceId, TaskRuntimeInvocationBindingId,
    TaskRuntimeInvocationId, TaskRuntimeInvocationReceiptId, TaskStackCompositionId,
    TaskStackFrameId, TaskStackFrameValidationId, TaskStorageLeaseId, TaskStorageOwnerId,
    ValueLayoutId,
};
pub use lifecycle_ledger::{
    ClosedTaskRuntime, SettledTaskLifecycle, TaskDependencyRecord, TaskLifecycleClaim,
    TaskLifecycleLedger, TaskRuntimeCloseError, TaskSettlementError, TaskStorageBinding,
    TaskStorageProvenance,
};
pub use runtime_invocation::{
    TaskRuntimeActivationBinding, TaskRuntimeInvocationReceiptCandidate,
    ValidatedTaskRuntimeInvocationReceipt, validate_task_runtime_invocation_receipt,
};
pub use semantic_vocabulary::SuspensionCrossingId;
pub use stack_composition::{
    AdmittedSameStackContribution, ComposedTaskStackDemand,
    SameStackContributionAdmissionCandidate, SameStackContributionCommitment,
    SameStackProviderPlanCommitment, StackCallContribution, TaskStackFrameSummary,
    ValidatedTaskStackFrameSummary, WcsuStackPlanProjection, admit_same_stack_contribution,
    compose_task_stack_demand, project_wcsu_stack_plan, validate_task_stack_frame_summary,
};
