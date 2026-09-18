//! Provider-independent task activation plans and lifecycle accounting.
//!
//! An activation plan describes one fixed, nonmoving stack, the canonical
//! semantic suspension crossings, and only the CPU/thread preservation those
//! crossings demand. Executor selection consumes exact per-axis checked or
//! admitted evidence; this crate deliberately does not publish a generalized
//! runtime behavior record.
//!
//! Start at `activation_plans.rs`: the plan, its validators, its diagnostic
//! and the facts a provider publishes about a plan.
//! `executor_selection` binds an executor to a plan's preservation axes,
//! `runtime_invocation` receipts one activation, `stack_leases` issues the
//! nonmoving stack authority, `lifecycle_ledger` runs the transactional
//! start and accounts for claims until settlement, `provider_admission`
//! is the provider-side gate consuming those carriers for one admitted
//! runtime instance, `stack_composition` projects WCSU stack plans,
//! `identities` holds every coordinate, `report_fingerprints` the compact
//! report values and `diagnostic` the failure type.

mod activation_plans;
mod executor_selection;
mod identities;
mod lifecycle_ledger;
mod provider_admission;
mod report_fingerprints;
mod runtime_invocation;
mod stack_composition;
mod stack_leases;
#[cfg(test)]
mod tests;

pub use activation_plans::activation_plan_facts::{
    SelectedTaskRuntimeProviderFact, TaskActivationPlanFact, TaskActivationPlanSet,
    TaskSpecializationCommitment, TaskStartOperation,
};
pub use activation_plans::diagnostic::TaskPlanDiagnostic;
pub use activation_plans::{
    ActivationCarryObligations, ActivationPlanCandidate, CanonicalSuspensionCrossing, StackPlan,
    ValidatedActivationPlan, validate_activation_plan, validate_wcsu_activation_plan,
};
pub use executor_selection::{
    ExecutorPreservationAxis, ExecutorPreservationEvidence, ExecutorSelectionCandidate,
    ValidatedExecutorSelection, validate_executor_selection,
};
pub use identities::{
    ActivationInstanceId, ActivationPlanId, AdmittedStackContributionReportId, CallingPlanId,
    ExecutorPreservationEvidenceId, ExecutorSelectionId, MachineContractId, MachineEntryId,
    SameStackContributionAdmissionReceiptId, StackPlanProjectionId, StackRepresentationId,
    TaskArgumentCustodyId, TaskLifecycleClaimId, TaskRuntimeId, TaskRuntimeInstanceId,
    TaskRuntimeInvocationBindingId, TaskRuntimeInvocationId, TaskRuntimeInvocationReceiptId,
    TaskStackCompositionId, TaskStackFrameId, TaskStackFrameValidationId, TaskStorageLeaseId,
    TaskStorageOwnerId, ValueLayoutId,
};
pub use lifecycle_ledger::{
    ClosedTaskRuntime, MovedTaskArguments, SettledTaskLifecycle, TaskDependencyRecord,
    TaskLifecycleClaim, TaskLifecycleLedger, TaskRuntimeCloseError, TaskSettlementError,
    TaskStartRejection, TaskStartStorage, TaskStorageBinding,
};
pub use provider_admission::{
    TaskAdmissionCloseError, TaskAdmissionRejection, TaskRuntimeAdmission,
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
pub use stack_leases::{
    StackLease, StackLeaseBacking, TaskStorageProvenance, establish_stack_lease,
};
