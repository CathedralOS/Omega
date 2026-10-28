//! Provider-independent task activation plans and invocation evidence.
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
//! executor selection, invocation receipts, stack composition, composition
//! model, identities and report fingerprints beneath it.

pub(crate) use optimization_core::digest_encoder;
mod activation;
mod fnv1a;
mod task_plans;
pub use activation::realize_task_activations;

pub use semantic_vocabulary::{ClaimId, SuspensionCrossingId};
pub(crate) use task_plans::StackPlan;
pub use task_plans::activation_plan_facts::{
    SelectedTaskRuntimeProviderFact, TaskActivationPlanFact, TaskActivationPlanSet,
    TaskSpecializationCommitment, TaskStartOperation,
};
pub use task_plans::diagnostic::TaskPlanDiagnostic;
pub use task_plans::identities::{
    ActivationPlanId, AdmittedStackContributionReportId, CallingPlanId, CompositionModelId,
    ExecutorPreservationEvidenceId, ExecutorSelectionId, LiveCarryPlaceId, LiveCarryTypeId,
    MachineContractId, MachineEntryId, SameStackContributionAdmissionReceiptId,
    StackPlanProjectionId, StackRepresentationId, TaskRuntimeId, TaskRuntimeInstanceId,
    TaskRuntimeInvocationBindingId, TaskRuntimeInvocationId, TaskRuntimeInvocationReceiptId,
    TaskStackCompositionId, TaskStackFrameId, TaskStackFrameValidationId, ValueLayoutId,
};
pub use task_plans::stack_composition::{
    AdmittedSameStackContribution, CallTargetBinding, SameStackContributionAdmissionCandidate,
    SameStackContributionCommitment, SameStackProviderPlanCommitment, StackCallContribution,
    TaskStackFrameSummary, UnresolvedCallKind, UnresolvedCallSite, ValidatedTaskStackFrameSummary,
    admit_same_stack_contribution, compose_task_stack_demand, project_wcsu_stack_plan,
    task_stack_frame_validation_identity, validate_task_stack_frame_summary,
};
pub use task_plans::{
    ActivationCarryObligations, ActivationPlanCandidate, CanonicalSuspensionCrossing,
    LiveCarryDemand, LiveCarryStorage, TaskArgumentLayout, ValidatedActivationPlan,
    validate_wcsu_activation_plan,
};
mod source_guards;
