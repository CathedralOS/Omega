//! Executor selection: the preservation axes an activation demands, the
//! evidence a provider supplies for each, and the validator that binds them.

use crate::task_plans::report_fingerprints::executor_selection_report_fingerprint;
use crate::task_plans::runtime_invocation::axis_label;
use crate::{
    ExecutorPreservationEvidenceId, ExecutorSelectionId, TaskPlanDiagnostic, TaskRuntimeId,
    TaskRuntimeInstanceId, ValidatedActivationPlan,
};

/// One independent affinity axis an executor can prove it preserves.
///
/// Suspension and address stability are deliberately absent. Suspension is a
/// local liveness judgment, while address stability follows from the selected
/// fixed nonmoving stack lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExecutorPreservationAxis {
    Cpu,
    HostThread,
}

/// Exact checked-conformance or admission-receipt evidence for one axis.
///
/// The normalized identity is produced by provider selection from the
/// conformance/receipt it validated. This is evidence identity, not a freely
/// authored runtime behavior bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutorPreservationEvidence {
    axis: ExecutorPreservationAxis,
    identity: ExecutorPreservationEvidenceId,
}

impl ExecutorPreservationEvidence {
    pub const fn new(
        axis: ExecutorPreservationAxis,
        identity: ExecutorPreservationEvidenceId,
    ) -> Self {
        Self { axis, identity }
    }

    pub const fn axis(self) -> ExecutorPreservationAxis {
        self.axis
    }

    pub const fn identity(self) -> ExecutorPreservationEvidenceId {
        self.identity
    }
}

/// Exact runtime instance and preservation evidence selected for one
/// provider-independent activation plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorSelectionCandidate {
    pub runtime: TaskRuntimeId,
    pub runtime_instance: TaskRuntimeInstanceId,
    pub preservation: Vec<ExecutorPreservationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedExecutorSelection {
    identity: ExecutorSelectionId,
    candidate: ExecutorSelectionCandidate,
    plan: ValidatedActivationPlan,
}

impl ValidatedExecutorSelection {
    pub const fn identity(&self) -> ExecutorSelectionId {
        self.identity
    }

    pub const fn candidate(&self) -> &ExecutorSelectionCandidate {
        &self.candidate
    }

    pub const fn plan(&self) -> &ValidatedActivationPlan {
        &self.plan
    }
}

/// Validate one already resolved executor selection against the activation's
/// demanded CPU/thread preservation. Checked providers and opaque admitted
/// providers both arrive as exact per-axis evidence identities; absence fails
/// closed, and multiple identities for one axis reject rather than guessing.
pub fn validate_executor_selection(
    plan: &ValidatedActivationPlan,
    mut candidate: ExecutorSelectionCandidate,
) -> Result<ValidatedExecutorSelection, TaskPlanDiagnostic> {
    candidate
        .preservation
        .sort_by_key(|evidence| evidence.axis());
    for duplicate in candidate.preservation.windows(2) {
        if duplicate[0].axis() == duplicate[1].axis() {
            return Err(TaskPlanDiagnostic(format!(
                "selected executor supplies more than one {} preservation identity; selection must retain one exact checked conformance or admission receipt",
                axis_label(duplicate[0].axis()),
            )));
        }
    }

    let establishes = |axis| {
        candidate
            .preservation
            .iter()
            .any(|evidence| evidence.axis() == axis)
    };
    let obligations = plan.candidate().carry_obligations;
    if obligations.preserve_cpu && !establishes(ExecutorPreservationAxis::Cpu) {
        return Err(TaskPlanDiagnostic(
            "selected executor does not establish CPU preservation required by the activation"
                .into(),
        ));
    }
    if obligations.preserve_host_thread && !establishes(ExecutorPreservationAxis::HostThread) {
        return Err(TaskPlanDiagnostic(
            "selected executor does not establish host-thread preservation required by the activation"
                .into(),
        ));
    }

    Ok(ValidatedExecutorSelection {
        identity: ExecutorSelectionId(executor_selection_report_fingerprint(plan, &candidate)),
        candidate,
        plan: plan.clone(),
    })
}
