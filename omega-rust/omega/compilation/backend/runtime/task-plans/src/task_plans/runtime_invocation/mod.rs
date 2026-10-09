//! Runtime invocation receipts: binding one activation to the provider
//! operation and plan that started it.

use crate::task_plans::report_fingerprints::runtime_invocation_report_fingerprint;
use crate::{
    ActivationPlanId, ExecutorPreservationAxis, ExecutorPreservationEvidence,
    ExecutorSelectionCandidate, SelectedTaskRuntimeProviderFact, TaskActivationPlanFact,
    TaskPlanDiagnostic, TaskRuntimeId, TaskRuntimeInstanceId, TaskRuntimeInvocationBindingId,
    TaskRuntimeInvocationId, TaskRuntimeInvocationReceiptId, TaskSpecializationCommitment,
    TaskStartOperation, ValidatedExecutorSelection, validate_executor_selection,
};

/// Provider-authored evidence for one dynamic invocation of an already
/// selected `TaskRuntime::{start,try_start}<M>` specialization.
///
/// This is deliberately a candidate rather than authority by construction.
/// Validation binds every copied field back to the Omega-owned static
/// activation fact before lifecycle accounting may consume the receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskRuntimeInvocationReceiptCandidate {
    pub receipt: TaskRuntimeInvocationReceiptId,
    pub invocation: TaskRuntimeInvocationId,
    pub runtime: TaskRuntimeId,
    pub runtime_instance: TaskRuntimeInstanceId,
    pub operation: TaskStartOperation,
    pub provider_plan_name: String,
    pub requirement_identity: String,
    pub activation_plan: ActivationPlanId,
    pub preservation: Vec<ExecutorPreservationEvidence>,
}

/// Normalized projection of the static activation fact retained at runtime.
/// Checked-tree symbol handles remain in the compilation sidecar; runtime
/// accounting needs only the specialization, operation, provider, and plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskRuntimeActivationBinding {
    pub specialization_report_fingerprint: u64,
    pub specialization_commitment: TaskSpecializationCommitment,
    pub operation: TaskStartOperation,
    pub selected_runtime: SelectedTaskRuntimeProviderFact,
    pub activation_plan: ActivationPlanId,
}

pub(crate) const fn axis_label(axis: ExecutorPreservationAxis) -> &'static str {
    match axis {
        ExecutorPreservationAxis::Cpu => "CPU",
        ExecutorPreservationAxis::HostThread => "host-thread",
    }
}
