//! Sealed whole-composition model extracted from a settled
//! [`TaskActivationPlanSet`].
//!
//! `wiki/spec/language/concurrency.md` protocol proofs: when a concrete
//! customer requires whole-composition proof, a sealed erased model must
//! retain activation creation, resource identities, wait/wake edges,
//! priorities, placement, and provider evidence, and ordinary proof machines
//! consume it at composition/deployment. [`SealedCompositionModel`] is that
//! extraction. It is produced only by [`compose_composition_model`] over the
//! settled plan set, is canonicalized so its identity does not depend on
//! elaboration order, and is independently re-derived by
//! [`replay_composition_model`]: a stale or fabricated model rejects rather
//! than standing in for fresh evidence.
//!
//! A dimension the settled vocabulary does not yet carry is recorded as
//! `NotRetained`: an absent dimension is never conflated with an empty
//! retained one. Priorities and inter-activation wait/wake edges (joins,
//! channel handoffs) have no source or plan vocabulary today, so the model
//! publishes their absence explicitly; bounded exploration results supply
//! neither.

use crate::task_plans::identities::CompositionModelId;
use crate::task_plans::report_fingerprints::composition_model_report_fingerprint;
use crate::{
    ActivationCarryObligations, ActivationPlanId, CallingPlanId, MachineContractId, MachineEntryId,
    SelectedTaskRuntimeProviderFact, StackPlan, StackPlanProjectionId, SuspensionCrossingId,
    TaskSpecializationCommitment, TaskStartOperation,
    ValueLayoutId};
use symbols::SymbolHandle;

/// Activation-creation evidence: which checked start operation and exact
/// specialization produced one activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompositionActivationCreation {
    /// The ordinary `TaskRuntime` operation that requested the activation.
    pub operation: TaskStartOperation,
    /// The checked start requirement the operation satisfied.
    pub start_requirement: SymbolHandle,
    /// The machine specialized onto the activation.
    pub target_machine: SymbolHandle,
    /// Its exact selected entry.
    pub target_entry: SymbolHandle,
    /// Strong identity of the exact checked specialization structure.
    pub specialization_commitment: TaskSpecializationCommitment,
    /// Historical compact compatibility/report coordinate.
    pub specialization_report_fingerprint: u64,
}

/// Every normalized resource identity one activation binds: its contract,
/// entry, argument and outcome layouts, calling plan and stack evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompositionResourceIdentities {
    pub machine_contract: MachineContractId,
    pub entry: MachineEntryId,
    pub argument_layout: ValueLayoutId,
    pub terminal_outcome_layout: ValueLayoutId,
    pub calling_plan: CallingPlanId,
    pub stack: StackPlan,
    /// `Some` when the stack shape carries sealed whole-call-graph WCSU
    /// evidence; `None` names the compiler-local layout bridge, which the
    /// model retains as the weaker evidence it is rather than upgrading.
    pub wcsu_stack_projection: Option<StackPlanProjectionId>,
}

/// One intra-activation wait/wake edge: a canonical suspension crossing at
/// which the activation parks (waits) until its runtime resumes (wakes) it.
/// The edge retains the crossing's preservation demands because a woken
/// activation resuming on a different CPU or thread is a different event
/// than the parked continuation bargained for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompositionWaitWakeEdge {
    pub crossing: SuspensionCrossingId,
    pub preserve_cpu: bool,
    pub preserve_host_thread: bool,
}

/// One activation's row in the composition model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompositionActivation {
    /// Stable identity of the whole validated activation plan.
    pub plan: ActivationPlanId,
    pub creation: CompositionActivationCreation,
    pub resources: CompositionResourceIdentities,
    /// Every canonical crossing the activation may park at, in canonical
    /// identity order as validated.
    pub wait_wake_edges: Vec<CompositionWaitWakeEdge>,
    /// Whether outcome settlement requires a safe-point cancellation
    /// observation.
    pub cancellation_required: bool,
    /// The placement the join of this activation's crossings commits:
    /// whether the parked continuation demands its origin CPU and/or host
    /// thread preserved.
    pub placement: ActivationCarryObligations,
    /// Exact selected runtime evidence bound to this activation.
    pub provider_evidence: SelectedTaskRuntimeProviderFact,
}

