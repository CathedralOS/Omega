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
    TaskActivationPlanSet, TaskPlanDiagnostic, TaskSpecializationCommitment, TaskStartOperation,
    ValueLayoutId,
};
use symbols::SymbolHandle;

/// Activation-creation evidence: which checked start operation and exact
/// specialization produced one activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionActivationCreation {
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
pub struct CompositionResourceIdentities {
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
pub struct CompositionWaitWakeEdge {
    pub crossing: SuspensionCrossingId,
    pub preserve_cpu: bool,
    pub preserve_host_thread: bool,
}

/// One activation's row in the composition model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionActivation {
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

/// Priority rows for the whole composition.
///
/// No priority surface exists in the source language or the settled plan
/// vocabulary today, so the model records the dimension's absence
/// explicitly instead of publishing an empty set that a proof machine could
/// mistake for "all activations unprioritized".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionPriorities {
    /// The composition carries no retained priority evidence.
    NotRetained,
}

/// Wait/wake edges between distinct activations: joins, channel handoffs
/// and other cross-activation unblocking.
///
/// The settled vocabulary retains only intra-activation suspension
/// crossings; no inter-activation waits-for relation is extractable yet,
/// so the model records the absence explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionCrossActivationEdges {
    /// No inter-activation wait/wake evidence is retained.
    NotRetained,
}

/// The sealed erased whole-composition model of one settled task activation
/// plan set. Constructible only through [`compose_composition_model`]; its
/// `identity` commits to every retained row and to the explicit absence
/// markers, so a fabricated or drifted model cannot share it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedCompositionModel {
    identity: CompositionModelId,
    /// One row per settled activation, in canonical [`ActivationPlanId`]
    /// order.
    activations: Vec<CompositionActivation>,
    priorities: CompositionPriorities,
    cross_activation_edges: CompositionCrossActivationEdges,
}

impl SealedCompositionModel {
    /// Normalized identity committing to the complete retained model.
    pub const fn identity(&self) -> CompositionModelId {
        self.identity
    }

    /// The composition's activation rows in canonical order.
    pub fn activations(&self) -> &[CompositionActivation] {
        &self.activations
    }

    /// Priority evidence for the whole composition.
    pub const fn priorities(&self) -> CompositionPriorities {
        self.priorities
    }

    /// Inter-activation wait/wake evidence for the whole composition.
    pub const fn cross_activation_edges(&self) -> CompositionCrossActivationEdges {
        self.cross_activation_edges
    }
}

/// Extract the sealed erased whole-composition model of a settled task
/// activation plan set.
///
/// Every retained row is a projection of a validated plan fact; nothing is
/// authored and nothing is inferred from names. The extraction canonicalizes
/// row order by [`ActivationPlanId`], so the model's identity depends on
/// what was settled, not on the order the compiler happened to visit it.
/// Two activations settling to the same plan identity would answer one
/// coordinate with two rows, so the composition rejects rather than merge
/// them.
pub fn compose_composition_model(
    plan_set: &TaskActivationPlanSet,
) -> Result<SealedCompositionModel, TaskPlanDiagnostic> {
    let mut activations: Vec<CompositionActivation> = plan_set
        .as_slice()
        .iter()
        .map(|fact| {
            let candidate = fact.plan.candidate();
            CompositionActivation {
                plan: fact.plan.normalized_identity(),
                creation: CompositionActivationCreation {
                    operation: fact.operation,
                    start_requirement: fact.start_requirement,
                    target_machine: fact.target_machine,
                    target_entry: fact.target_entry,
                    specialization_commitment: fact.specialization_commitment,
                    specialization_report_fingerprint: fact.specialization_report_fingerprint,
                },
                resources: CompositionResourceIdentities {
                    machine_contract: candidate.machine_contract,
                    entry: candidate.entry,
                    argument_layout: candidate.argument_layout.identity,
                    terminal_outcome_layout: candidate.terminal_outcome_layout,
                    calling_plan: candidate.calling_plan,
                    stack: candidate.stack_plan,
                    wcsu_stack_projection: fact
                        .plan
                        .wcsu_stack_projection()
                        .map(|projection| projection.identity()),
                },
                wait_wake_edges: candidate
                    .canonical_suspension_crossings
                    .iter()
                    .map(|crossing| CompositionWaitWakeEdge {
                        crossing: crossing.identity,
                        preserve_cpu: crossing.preserve_cpu,
                        preserve_host_thread: crossing.preserve_host_thread,
                    })
                    .collect(),
                cancellation_required: candidate.cancellation_required,
                placement: candidate.carry_obligations,
                provider_evidence: fact.selected_runtime.clone(),
            }
        })
        .collect();
    activations.sort_by_key(|activation| activation.plan.normalized_identity());
    for window in activations.windows(2) {
        if window[0].plan == window[1].plan {
            return Err(TaskPlanDiagnostic(
                "composition carries two activations with the same plan identity".into(),
            ));
        }
    }
    Ok(SealedCompositionModel {
        identity: CompositionModelId(composition_model_report_fingerprint(&activations)),
        activations,
        priorities: CompositionPriorities::NotRetained,
        cross_activation_edges: CompositionCrossActivationEdges::NotRetained,
    })
}

/// Independently re-derive the composition model of `plan_set` and compare
/// it against `presented` in full. A forged, stale or truncated model —
/// including one whose identity matches but whose rows do not — rejects.
pub fn replay_composition_model(
    plan_set: &TaskActivationPlanSet,
    presented: &SealedCompositionModel,
) -> Result<(), TaskPlanDiagnostic> {
    let fresh = compose_composition_model(plan_set)?;
    if &fresh != presented {
        return Err(TaskPlanDiagnostic(
            "presented composition model does not match a fresh extraction of the settled plan \
             set"
            .into(),
        ));
    }
    Ok(())
}
