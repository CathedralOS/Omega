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
pub struct TaskRuntimeInvocationReceiptCandidate {
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
pub struct TaskRuntimeActivationBinding {
    pub specialization_report_fingerprint: u64,
    pub specialization_commitment: TaskSpecializationCommitment,
    pub operation: TaskStartOperation,
    pub selected_runtime: SelectedTaskRuntimeProviderFact,
    pub activation_plan: ActivationPlanId,
}

/// Exact static/dynamic binding retained for one provider-accepted task start.
/// The retained carrier is independent of checked-tree symbol handles: only
/// normalized Omega realization identity crosses into runtime accounting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTaskRuntimeInvocationReceipt {
    identity: TaskRuntimeInvocationBindingId,
    pub(crate) candidate: TaskRuntimeInvocationReceiptCandidate,
    activation: TaskRuntimeActivationBinding,
    executor_selection: ValidatedExecutorSelection,
}

impl ValidatedTaskRuntimeInvocationReceipt {
    pub const fn identity(&self) -> TaskRuntimeInvocationBindingId {
        self.identity
    }

    pub const fn candidate(&self) -> &TaskRuntimeInvocationReceiptCandidate {
        &self.candidate
    }

    pub const fn activation(&self) -> &TaskRuntimeActivationBinding {
        &self.activation
    }

    pub const fn executor_selection(&self) -> &ValidatedExecutorSelection {
        &self.executor_selection
    }
}

/// Bind a provider invocation receipt to one exact post-check activation fact.
/// Static provider/requirement/operation drift rejects before the normalized
/// lifecycle ledger can acquire operational or storage custody.
pub fn validate_task_runtime_invocation_receipt(
    activation: &TaskActivationPlanFact,
    candidate: TaskRuntimeInvocationReceiptCandidate,
) -> Result<ValidatedTaskRuntimeInvocationReceipt, TaskPlanDiagnostic> {
    if activation.specialization_commitment.is_zero() {
        return Err(TaskPlanDiagnostic(
            "task runtime activation has an empty specialization commitment".into(),
        ));
    }
    if candidate.runtime != activation.selected_runtime.runtime {
        return Err(TaskPlanDiagnostic(
            "task runtime invocation receipt names a different selected runtime".into(),
        ));
    }
    if candidate.provider_plan_name != activation.selected_runtime.provider_plan_name {
        return Err(TaskPlanDiagnostic(
            "task runtime invocation receipt names a different selected provider plan".into(),
        ));
    }
    if candidate.requirement_identity != activation.selected_runtime.requirement_identity {
        return Err(TaskPlanDiagnostic(
            "task runtime invocation receipt names a different start requirement".into(),
        ));
    }
    if candidate.operation != activation.operation {
        return Err(TaskPlanDiagnostic(
            "task runtime invocation receipt names a different start operation".into(),
        ));
    }
    if candidate.activation_plan != activation.plan.normalized_identity() {
        return Err(TaskPlanDiagnostic(
            "task runtime invocation receipt names a different activation plan".into(),
        ));
    }
    let executor_selection = validate_executor_selection(
        &activation.plan,
        ExecutorSelectionCandidate {
            runtime: candidate.runtime,
            runtime_instance: candidate.runtime_instance,
            preservation: candidate.preservation.clone(),
        },
    )?;
    Ok(ValidatedTaskRuntimeInvocationReceipt {
        identity: TaskRuntimeInvocationBindingId(runtime_invocation_report_fingerprint(
            activation,
            &candidate,
            &executor_selection,
        )),
        candidate,
        activation: TaskRuntimeActivationBinding {
            specialization_report_fingerprint: activation.specialization_report_fingerprint,
            specialization_commitment: activation.specialization_commitment,
            operation: activation.operation,
            selected_runtime: activation.selected_runtime.clone(),
            activation_plan: activation.plan.normalized_identity(),
        },
        executor_selection,
    })
}

pub(crate) const fn axis_label(axis: ExecutorPreservationAxis) -> &'static str {
    match axis {
        ExecutorPreservationAxis::Cpu => "CPU",
        ExecutorPreservationAxis::HostThread => "host-thread",
    }
}
