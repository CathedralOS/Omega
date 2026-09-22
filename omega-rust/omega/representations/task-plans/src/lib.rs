//! Provider-independent task activation plans and lifecycle accounting.
//!
//! An activation plan describes one fixed, nonmoving stack, the canonical
//! semantic suspension crossings — each retaining the exact live frontier
//! of places, claims and four-axis demands the parked continuation must
//! keep — and only the CPU/thread preservation those crossings demand.
//! Executor selection consumes exact per-axis checked or
//! admitted evidence; this crate deliberately does not publish a generalized
//! runtime behavior record.
//!
//! Start at `task_plans.rs`: the activation plan, its exact marshalling
//! argument layout and its validators, leading into the provider facts,
//! executor selection, invocation receipts, stack leases, lifecycle ledger,
//! provider admission, stack composition, composition model, identities and
//! report fingerprints beneath it.

mod task_plans;
#[cfg(test)]
mod tests;

pub use semantic_vocabulary::{ClaimId, SuspensionCrossingId};
pub use task_plans::activation_plan_facts::{
    SelectedTaskRuntimeProviderFact, TaskActivationPlanFact, TaskActivationPlanSet,
    TaskSpecializationCommitment, TaskStartOperation,
};
pub use task_plans::composition_model::{
    CompositionActivation, CompositionActivationCreation, CompositionCrossActivationEdges,
    CompositionPriorities, CompositionResourceIdentities, CompositionWaitWakeEdge,
    SealedCompositionModel, compose_composition_model, replay_composition_model,
};
pub use task_plans::diagnostic::TaskPlanDiagnostic;
pub use task_plans::executor_selection::{
    ExecutorPreservationAxis, ExecutorPreservationEvidence, ExecutorSelectionCandidate,
    ValidatedExecutorSelection, validate_executor_selection,
};
pub use task_plans::identities::{
    ActivationInstanceId, ActivationPlanId, AdmittedStackContributionReportId, CallingPlanId,
    CompositionModelId, ExecutorPreservationEvidenceId, ExecutorSelectionId, LiveCarryPlaceId,
    LiveCarryTypeId, MachineContractId, MachineEntryId, SameStackContributionAdmissionReceiptId,
    StackPlanProjectionId, StackRepresentationId, TaskArgumentCustodyId, TaskLifecycleClaimId,
    TaskRuntimeId, TaskRuntimeInstanceId, TaskRuntimeInvocationBindingId, TaskRuntimeInvocationId,
    TaskRuntimeInvocationReceiptId, TaskStackCompositionId, TaskStackFrameId,
    TaskStackFrameValidationId, TaskStorageLeaseId, TaskStorageOwnerId, ValueLayoutId,
};
pub use task_plans::lifecycle_ledger::{
    ClosedTaskRuntime, MovedTaskArguments, SettledTaskLifecycle, TaskClaimRoute,
    TaskDependencyRecord, TaskLifecycleClaim, TaskLifecycleLedger, TaskRouteSettlementError,
    TaskRuntimeCloseError, TaskSettlementError, TaskSettlementOutcome, TaskStartRejection,
    TaskStartStorage, TaskStorageBinding,
};
pub use task_plans::provider_admission::{
    TaskAdmissionCloseError, TaskAdmissionRejection, TaskRuntimeAdmission,
};
pub use task_plans::runtime_invocation::{
    TaskRuntimeActivationBinding, TaskRuntimeInvocationReceiptCandidate,
    ValidatedTaskRuntimeInvocationReceipt, validate_task_runtime_invocation_receipt,
};
pub use task_plans::stack_composition::{
    AdmittedSameStackContribution, CallTargetBinding, ComposedTaskStackDemand,
    SameStackContributionAdmissionCandidate, SameStackContributionCommitment,
    SameStackProviderPlanCommitment, StackCallContribution, TaskStackFrameSummary,
    UnresolvedCallKind, UnresolvedCallSite, ValidatedTaskStackFrameSummary,
    WcsuStackPlanProjection, admit_same_stack_contribution, compose_task_stack_demand,
    cover_unresolved_call_sites, project_wcsu_stack_plan, task_stack_frame_validation_identity,
    validate_task_stack_frame_summary,
};
pub use task_plans::stack_leases::{
    StackLease, StackLeaseBacking, TaskStorageProvenance, establish_stack_lease,
};
pub use task_plans::{
    ActivationCarryObligations, ActivationPlanCandidate, CanonicalSuspensionCrossing,
    LiveCarryDemand, LiveCarryStorage, StackPlan, TaskArgumentExtent, TaskArgumentLayout,
    ValidatedActivationPlan, validate_activation_plan, validate_wcsu_activation_plan,
};
