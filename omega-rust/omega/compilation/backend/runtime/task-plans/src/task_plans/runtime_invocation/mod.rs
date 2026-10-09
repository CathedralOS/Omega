//! Runtime invocation receipts: binding one activation to the provider
//! operation and plan that started it.

use crate::{
    ActivationPlanId, ExecutorPreservationAxis, ExecutorPreservationEvidence,
    TaskActivationPlanFact,
    TaskPlanDiagnostic, TaskRuntimeId, TaskRuntimeInstanceId, TaskRuntimeInvocationBindingId,
    TaskRuntimeInvocationId, TaskRuntimeInvocationReceiptId, TaskSpecializationCommitment,
    TaskStartOperation, ValidatedExecutorSelection, validate_executor_selection};

pub(crate) const fn axis_label(axis: ExecutorPreservationAxis) -> &'static str {
    match axis {
        ExecutorPreservationAxis::Cpu => "CPU",
        ExecutorPreservationAxis::HostThread => "host-thread"}
}
