//! Facts a selected runtime provider publishes about one activation plan:
//! the start operation, the specialization commitment and the plan set.

use crate::task_plans::identities::commitment_digest;
use crate::{TaskRuntimeId, ValidatedActivationPlan};

/// Which ordinary `TaskRuntime` operation requested one concrete activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStartOperation {
    Start,
    TryStart,
}

commitment_digest!(
    /// Domain-separated SHA-256 commitment to the exact checked TaskRuntime
    /// requirement, operation, target entry, and target contract selected by
    /// one task activation.
    TaskSpecializationCommitment
);

/// Exact selected runtime evidence paired with one Omega activation plan.
/// This is post-check provider realization state, not part of Psi checked
/// semantics. Source-local requirement handles remain in derivation; their
/// canonical identities are covered by the activation's specialization commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTaskRuntimeProviderFact {
    pub runtime: TaskRuntimeId,
    pub provider_plan_name: String,
}

/// One target/layout-specific task activation elaborated by Omega after Psi
/// semantic checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskActivationPlanFact {
    pub start_requirement: terminal_psi::TerminalIdentityIndex,
    pub target_machine: terminal_psi::TerminalIdentityIndex,
    pub target_entry: terminal_psi::TerminalIdentityIndex,
    /// Historical compact compatibility/report coordinate.
    pub specialization_report_fingerprint: u64,
    /// Strong identity of the exact checked specialization structure.
    pub specialization_commitment: TaskSpecializationCommitment,
    pub operation: TaskStartOperation,
    pub selected_runtime: SelectedTaskRuntimeProviderFact,
    pub plan: ValidatedActivationPlan,
}

/// Omega-owned task activation sidecar for one checked compilation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskActivationPlanSet {
    pub identities: terminal_psi::TerminalIdentityTable,
    pub activations: Vec<TaskActivationPlanFact>,
}

impl TaskActivationPlanSet {
    pub fn as_slice(&self) -> &[TaskActivationPlanFact] {
        &self.activations
    }

    pub fn is_empty(&self) -> bool {
        self.activations.is_empty()
    }
}
