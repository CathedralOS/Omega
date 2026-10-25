//! Whole-call-graph stack composition for one fixed-stack activation.
//!
//! Checked same-stack calls extend the live frame chain. Sequential sibling
//! calls share capacity, so composition takes their maximum rather than their
//! sum. Opaque same-stack leaves enter only through an exact admitted
//! contribution; a provider-stack or new-activation transfer contributes no
//! child frame to this stack and therefore has no edge in this graph.
//! A live call the producer cannot resolve to a checked callee or an admitted
//! contribution is not dropped: it enters the frame summary's
//! [`UnresolvedCallSite`] roster, the composed demand publishes as partial,
//! and the roster rides the sealed projection so no selected runtime can treat
//! the covered subgraph as the whole call graph.
//! Provider admission covers a sealed site by presenting a
//! [`CallTargetBinding`]: [`cover_unresolved_call_sites`] moves the site to a
//! checked callee edge, merges the bound callee's validated subtree into the
//! demand's frame evidence, and recomposes — a projection whose roster
//! empties publishes exact, while a site no binding names stays unresolved
//! and keeps rejecting. The binding also carries the bound subtree's
//! canonical suspension crossings, which `bind_call_targets` joins into the
//! served plan's roster when it re-seals.
//! Projections spelled `wcsu` carry the worst-case stack usage (WCSU) that the
//! storage contract defines.

use crate::fnv1a::Fnv1a;
use crate::task_plans::identities::commitment_digest;

use crate::{
    AdmittedStackContributionReportId, CanonicalSuspensionCrossing,
    SameStackContributionAdmissionReceiptId, StackPlan, StackPlanProjectionId,
    StackRepresentationId, TaskPlanDiagnostic, TaskStackCompositionId, TaskStackFrameId,
    TaskStackFrameValidationId,
};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests;

/// Untrusted inputs presented for one opaque same-stack contribution.
///
/// Admission must compare the provider-plan report coordinate, strong
/// commitment, and requirement identity with the compiler's authoritative
/// selection. The receipt names the independent evidence that admits this
/// otherwise opaque byte/alignment claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameStackContributionAdmissionCandidate {
    /// Historical compact report coordinate. This is not admission authority.
    pub provider_plan_report_identity: u64,
    /// Strong identity of the exact selected provider plan.
    pub provider_plan_commitment: SameStackProviderPlanCommitment,
    pub requirement_identity: terminal_psi::TerminalIdentityIndex,
    pub receipt: SameStackContributionAdmissionReceiptId,
    pub bytes: u64,
    pub alignment: u64,
}

commitment_digest!(
    /// Domain-separated commitment to the exact selected provider plan that
    /// owns one opaque same-stack contribution.
    SameStackProviderPlanCommitment,
    from:
    /// Wrap a digest issued by canonical provider-plan construction.
);

commitment_digest!(
    /// Domain-separated commitment to the complete admitted same-stack
    /// claim. This remains authoritative when the compact report identity is
    /// projected into later WCSU planning.
    SameStackContributionCommitment,
    from:
    /// Reconstruct a retained non-authoritative commitment projection.
    /// Admission authority remains with [`AdmittedSameStackContribution`].
);

/// Sealed opaque same-stack demand accepted against an exact provider choice.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AdmittedSameStackContribution {
    report_identity: AdmittedStackContributionReportId,
    commitment: SameStackContributionCommitment,
    provider_plan_report_identity: u64,
    provider_plan_commitment: SameStackProviderPlanCommitment,
    requirement_identity: terminal_psi::TerminalIdentityIndex,
    receipt: SameStackContributionAdmissionReceiptId,
    bytes: u64,
    alignment: u64,
}

impl AdmittedSameStackContribution {
    pub const fn report_identity(&self) -> AdmittedStackContributionReportId {
        self.report_identity
    }

    pub const fn commitment(&self) -> SameStackContributionCommitment {
        self.commitment
    }

    pub const fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    pub const fn provider_plan_commitment(&self) -> SameStackProviderPlanCommitment {
        self.provider_plan_commitment
    }

    pub const fn requirement_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.requirement_identity
    }

    pub const fn receipt(&self) -> SameStackContributionAdmissionReceiptId {
        self.receipt
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }
}

pub fn admit_same_stack_contribution(
    candidate: SameStackContributionAdmissionCandidate,
    selected_provider_plan_report_identity: u64,
    selected_provider_plan_commitment: SameStackProviderPlanCommitment,
    selected_requirement_identity: terminal_psi::TerminalIdentityIndex,
) -> Result<AdmittedSameStackContribution, TaskPlanDiagnostic> {
    if selected_provider_plan_report_identity == 0 {
        return Err(TaskPlanDiagnostic(
            "selected provider-plan report identity for same-stack admission cannot be zero".into(),
        ));
    }
    if selected_provider_plan_commitment.is_zero() {
        return Err(TaskPlanDiagnostic(
            "selected provider-plan commitment for same-stack admission cannot be zero".into(),
        ));
    }
    if candidate.provider_plan_report_identity != selected_provider_plan_report_identity {
        return Err(TaskPlanDiagnostic(format!(
            "same-stack admission provider-plan report identity 0x{:016x} does not match selected report identity 0x{selected_provider_plan_report_identity:016x}",
            candidate.provider_plan_report_identity
        )));
    }
    if candidate.provider_plan_commitment.is_zero()
        || candidate.provider_plan_commitment != selected_provider_plan_commitment
    {
        return Err(TaskPlanDiagnostic(
            "same-stack admission provider-plan commitment does not match the exact selected plan"
                .into(),
        ));
    }
    if selected_requirement_identity.is_absent() {
        return Err(TaskPlanDiagnostic(
            "selected requirement identity for same-stack admission cannot be absent".into(),
        ));
    }
    if candidate.requirement_identity != selected_requirement_identity {
        return Err(TaskPlanDiagnostic(
            "same-stack admission requirement identity does not match the selected identity".into(),
        ));
    }
    // A measured frameless body may contribute zero bytes. The retained edge,
    // receipt and exact commitment distinguish that result from missing demand;
    // argument, shadow and return linkage remain in the caller-live frontier.
    validate_alignment(candidate.alignment, "same-stack admission")?;

    let report_identity =
        AdmittedStackContributionReportId(admitted_contribution_report_fingerprint(&candidate));
    let commitment = admitted_contribution_commitment(&candidate);
    Ok(AdmittedSameStackContribution {
        report_identity,
        commitment,
        provider_plan_report_identity: candidate.provider_plan_report_identity,
        provider_plan_commitment: candidate.provider_plan_commitment,
        requirement_identity: candidate.requirement_identity,
        receipt: candidate.receipt,
        bytes: candidate.bytes,
        alignment: candidate.alignment,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StackCallContribution {
    Checked { callee: TaskStackFrameId },
    AdmittedSameStack(AdmittedSameStackContribution),
}

/// Why one live call's same-stack demand is absent from a frame's covered
/// roster. This is the only place an uncovered call is classified; the
/// coordinate plus kind is the admission worklist's evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnresolvedCallKind {
    /// The call target names no checked machine state: a requirement slot,
    /// a machine parameter, or a dynamic descriptor call.
    UnresolvedTarget,
    /// The call target resolves to a machine state supplied by non-checked
    /// means: requirement, top-level requirement, boundary, admission-claim
    /// or external realization supply.
    NonCheckedSupply,
}

/// One live call a frame's whole-call-graph bound could not cover.
///
/// A worst-case bound cannot silently omit a call that may place a callee
/// frame on this stack. Recording the site keeps the composed demand honest:
/// it publishes as partial rather than exact, and each entry names the exact
/// call coordinate provider admission must later cover with an
/// [`AdmittedSameStackContribution`] — or prove transfers off this stack —
/// before the plan can establish exact stack demand. The coordinate mirrors the
/// canonical suspension-crossing coordinate: `frame` supplies the machine
/// and entry, `state` the reachable state inside that frame, and
/// `(statement_index, call_ordinal)` the exact call within that state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnresolvedCallSite {
    /// The frame containing the uncovered call.
    pub frame: TaskStackFrameId,
    /// Name of the reachable state inside `frame` whose flow row owns the
    /// call — the frame spans several states of one machine, so the
    /// statement/call index pair is only exact alongside it.
    pub state: String,
    /// Exact call coordinate inside the state's checked flow row.
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub kind: UnresolvedCallKind,
}

/// Provider admission's binding of one sealed [`UnresolvedCallSite`] to a
/// concrete checked-body callee.
///
/// A requirement slot, machine parameter, or dynamic descriptor the checker
/// could not resolve finally names its machine when the provider binds the
/// call target at admission. The `(frame, state, statement_index,
/// call_ordinal)` coordinate must match a sealed unresolved site exactly —
/// a binding naming no site fails closed rather than covering anything.
/// `callee` names the root frame of the bound callee subtree, and `subtree`
/// carries every validated frame of that subtree, callee root included, as
/// the provider-side whole-call-graph derivation produced them. Covering
/// moves the site into a checked call edge and merges the subtree into the
/// composition's frame evidence, so the bound callee's validated stack frame
/// charges into the demand exactly as if the graph had resolved the call.
/// Unresolved sites inside the presented subtree stay unresolved: binding a
/// target never launders the callee's own partial evidence into exactness.
///
/// `crossings` carries the bound subtree's canonical suspension crossings —
/// the provider-side call-graph derivation's row per checked suspension
/// crossing reachable inside `subtree`. Covering joins them into the plan's
/// canonical roster alongside the containment-subtree rows: an identity the
/// roster already holds must carry the identical row, a conflicting row
/// fails closed, and a novel identity joins so a bound callee that suspends
/// can park at a crossing of its own subtree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallTargetBinding {
    /// Frame owning the unresolved call the provider is binding.
    pub frame: TaskStackFrameId,
    /// Name of the reachable state inside `frame` whose flow row owns the
    /// call — the frame spans several states of one machine, so the
    /// statement/call index pair is only exact alongside it.
    pub state: String,
    /// Exact call coordinate inside the state's checked flow row.
    pub statement_index: usize,
    pub call_ordinal: usize,
    /// Root frame of the bound callee subtree.
    pub callee: TaskStackFrameId,
    /// The bound callee's complete validated subtree: its root frame and
    /// every frame reachable through its own checked calls.
    pub subtree: Vec<ValidatedTaskStackFrameSummary>,
    /// The bound subtree's canonical suspension crossings — one row per
    /// checked suspension crossing reachable inside `subtree`, exactly as
    /// the provider-side call-graph derivation produced them.
    pub crossings: Vec<CanonicalSuspensionCrossing>,
}

/// Compiler-produced local frame facts before whole-graph composition.
/// `local_bytes` includes target calling/entry overhead owned by this frame;
/// every child begins while those bytes remain live. `unresolved_calls`
/// carries the live calls this frame cannot bound; while any frame's roster
/// is non-empty the composed demand is partial, not exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStackFrameSummary {
    pub frame: TaskStackFrameId,
    pub local_bytes: u64,
    pub alignment: u64,
    pub validation: TaskStackFrameValidationId,
    pub calls: Vec<StackCallContribution>,
    pub unresolved_calls: Vec<UnresolvedCallSite>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTaskStackFrameSummary(TaskStackFrameSummary);

impl ValidatedTaskStackFrameSummary {
    pub const fn summary(&self) -> &TaskStackFrameSummary {
        &self.0
    }
}

pub fn validate_task_stack_frame_summary(
    mut summary: TaskStackFrameSummary,
) -> Result<ValidatedTaskStackFrameSummary, TaskPlanDiagnostic> {
    if summary.local_bytes == 0 {
        return Err(TaskPlanDiagnostic(format!(
            "task stack frame 0x{:016x} has zero local WCSU",
            summary.frame.normalized_identity()
        )));
    }
    validate_alignment(
        summary.alignment,
        &format!(
            "task stack frame 0x{:016x}",
            summary.frame.normalized_identity()
        ),
    )?;
    for call in &summary.calls {
        if let StackCallContribution::AdmittedSameStack(contribution) = call {
            validate_alignment(
                contribution.alignment(),
                &format!(
                    "admitted same-stack contribution 0x{:016x}",
                    contribution.report_identity().normalized_identity()
                ),
            )?;
        }
    }
    for site in &summary.unresolved_calls {
        if site.frame != summary.frame {
            return Err(TaskPlanDiagnostic(format!(
                "task stack frame 0x{:016x} carries an unresolved call site attributed to frame \
                 0x{:016x}",
                summary.frame.normalized_identity(),
                site.frame.normalized_identity()
            )));
        }
    }
    summary.calls.sort_unstable();
    summary.calls.dedup();
    summary.unresolved_calls.sort_unstable();
    summary.unresolved_calls.dedup();
    Ok(ValidatedTaskStackFrameSummary(summary))
}

fn validate_alignment(alignment: u64, subject: &str) -> Result<(), TaskPlanDiagnostic> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(TaskPlanDiagnostic(format!(
            "{subject} alignment {alignment} is not a nonzero power of two"
        )));
    }
    Ok(())
}

/// Bind a frame's validated content: its identity, its exact local extent
/// and alignment, the callee roster it can place beneath itself, and the
/// unresolved call sites the bound does not cover. The rosters hash in
/// canonical order, so the identity binds what the frame contains rather
/// than the order a producer presented it — graph derivation and
/// admission-time covering therefore mint the same identity for the same
/// covered content.
pub fn task_stack_frame_validation_identity(
    frame: TaskStackFrameId,
    local_bytes: u64,
    alignment: u64,
    calls: &[StackCallContribution],
    unresolved_calls: &[UnresolvedCallSite],
) -> TaskStackFrameValidationId {
    let mut calls = calls.to_vec();
    calls.sort_unstable();
    let mut unresolved_calls = unresolved_calls.to_vec();
    unresolved_calls.sort_unstable();
    let mut hash = Fnv1a::new();
    hash.byte(0x56);
    hash.u64(frame.normalized_identity());
    hash.u64(local_bytes);
    hash.u64(alignment);
    hash.u64(calls.len() as u64);
    for call in &calls {
        match call {
            StackCallContribution::Checked { callee } => {
                hash.byte(1);
                hash.u64(callee.normalized_identity());
            }
            StackCallContribution::AdmittedSameStack(contribution) => {
                hash.byte(2);
                hash.u64(contribution.report_identity().normalized_identity());
            }
        }
    }
    hash_unresolved_calls(&mut hash, unresolved_calls.iter());
    TaskStackFrameValidationId::from_normalized_identity(hash.finish_or(Fnv1a::OFFSET_BASIS))
        .expect("normalized frame validation identity is never zero")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskStackCompositionEvidence {
    root: TaskStackFrameId,
    frames: BTreeMap<TaskStackFrameId, ValidatedTaskStackFrameSummary>,
}

/// Sealed maximum live stack chain for one fixed-stack activation.
///
/// `unresolved_calls` is the roster of live calls the graph could not bind
/// to a checked or admitted contribution. While it is non-empty the composed
/// bound is partial: `bytes`/`alignment` describe only the covered subgraph,
/// and `is_exact` is the publication gate consumers such as
/// `establish_stack_lease` must require.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedTaskStackDemand {
    identity: TaskStackCompositionId,
    root: TaskStackFrameId,
    bytes: u64,
    alignment: u64,
    contributing_frames: BTreeSet<TaskStackFrameId>,
    frame_validations: BTreeSet<TaskStackFrameValidationId>,
    admitted_contribution_report_identities: BTreeSet<AdmittedStackContributionReportId>,
    admitted_contribution_commitments: BTreeSet<SameStackContributionCommitment>,
    unresolved_calls: BTreeSet<UnresolvedCallSite>,
    evidence: TaskStackCompositionEvidence,
}

impl ComposedTaskStackDemand {
    pub const fn identity(&self) -> TaskStackCompositionId {
        self.identity
    }

    pub const fn root(&self) -> TaskStackFrameId {
        self.root
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    pub const fn contributing_frames(&self) -> &BTreeSet<TaskStackFrameId> {
        &self.contributing_frames
    }

    pub const fn frame_validations(&self) -> &BTreeSet<TaskStackFrameValidationId> {
        &self.frame_validations
    }

    pub const fn admitted_contribution_report_identities(
        &self,
    ) -> &BTreeSet<AdmittedStackContributionReportId> {
        &self.admitted_contribution_report_identities
    }

    pub const fn admitted_contribution_commitments(
        &self,
    ) -> &BTreeSet<SameStackContributionCommitment> {
        &self.admitted_contribution_commitments
    }

    /// The live calls this bound could not cover, deduplicated across the
    /// reachable graph. Empty means the bound is the exact whole-call-graph
    /// WCSU the storage contract names.
    pub const fn unresolved_calls(&self) -> &BTreeSet<UnresolvedCallSite> {
        &self.unresolved_calls
    }
}

/// Sealed projection of one composed WCSU demand into a physical fixed-stack
/// representation.
///
/// The compact composition identity is not used as a substitute for the facts
/// a stack allocator and activation fingerprint rely on. The projection also
/// retains the exact root, composed shape, frame-validation set, admitted
/// contribution set, unresolved-call roster, selected representation, and the
/// composition's frame evidence — admission-time call-target covering
/// recomposes from that retained evidence, never from the published shape
/// alone. A non-empty roster makes the projection partial: the retained
/// shape is the covered subgraph's demand, not the whole-call-graph WCSU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WcsuStackPlanProjection {
    identity: StackPlanProjectionId,
    composition: TaskStackCompositionId,
    root: TaskStackFrameId,
    bytes: u64,
    alignment: u64,
    frame_validations: BTreeSet<TaskStackFrameValidationId>,
    admitted_contribution_report_identities: BTreeSet<AdmittedStackContributionReportId>,
    admitted_contribution_commitments: BTreeSet<SameStackContributionCommitment>,
    unresolved_calls: BTreeSet<UnresolvedCallSite>,
    representation: StackRepresentationId,
    evidence: TaskStackCompositionEvidence,
}

impl WcsuStackPlanProjection {
    pub const fn identity(&self) -> StackPlanProjectionId {
        self.identity
    }

    pub const fn composition(&self) -> TaskStackCompositionId {
        self.composition
    }

    pub const fn root(&self) -> TaskStackFrameId {
        self.root
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    pub const fn frame_validations(&self) -> &BTreeSet<TaskStackFrameValidationId> {
        &self.frame_validations
    }

    pub const fn admitted_contribution_report_identities(
        &self,
    ) -> &BTreeSet<AdmittedStackContributionReportId> {
        &self.admitted_contribution_report_identities
    }

    pub const fn admitted_contribution_commitments(
        &self,
    ) -> &BTreeSet<SameStackContributionCommitment> {
        &self.admitted_contribution_commitments
    }

    /// The live calls the projected bound does not cover. Empty means the
    /// projected shape is the exact whole-call-graph WCSU.
    pub const fn unresolved_calls(&self) -> &BTreeSet<UnresolvedCallSite> {
        &self.unresolved_calls
    }

    pub const fn representation(&self) -> StackRepresentationId {
        self.representation
    }

    pub const fn stack_plan(&self) -> StackPlan {
        StackPlan {
            bytes: self.bytes,
            alignment: self.alignment,
            representation: self.representation,
        }
    }

    pub(crate) fn has_valid_identity(&self) -> bool {
        self.identity
            == StackPlanProjectionId(stack_plan_projection_report_fingerprint(
                self.composition,
                self.root,
                self.bytes,
                self.alignment,
                &self.frame_validations,
                &self.admitted_contribution_report_identities,
                &self.admitted_contribution_commitments,
                &self.unresolved_calls,
                self.representation,
            ))
    }
}

/// Bind one validated whole-call-graph demand to the exact fixed-stack
/// representation that will provision it. The projection inherits the
/// demand's unresolved-call roster: a partial demand projects a partial
/// plan, and the projection identity keeps the two apart.
pub fn project_wcsu_stack_plan(
    demand: &ComposedTaskStackDemand,
    representation: StackRepresentationId,
) -> WcsuStackPlanProjection {
    let composition = demand.identity;
    let root = demand.root;
    let bytes = demand.bytes;
    let alignment = demand.alignment;
    let frame_validations = demand.frame_validations.clone();
    let admitted_contribution_report_identities =
        demand.admitted_contribution_report_identities.clone();
    let admitted_contribution_commitments = demand.admitted_contribution_commitments.clone();
    let unresolved_calls = demand.unresolved_calls.clone();
    let identity = StackPlanProjectionId(stack_plan_projection_report_fingerprint(
        composition,
        root,
        bytes,
        alignment,
        &frame_validations,
        &admitted_contribution_report_identities,
        &admitted_contribution_commitments,
        &unresolved_calls,
        representation,
    ));
    WcsuStackPlanProjection {
        identity,
        composition,
        root,
        bytes,
        alignment,
        frame_validations,
        admitted_contribution_report_identities,
        admitted_contribution_commitments,
        unresolved_calls,
        representation,
        evidence: demand.evidence.clone(),
    }
}

pub fn compose_task_stack_demand(
    root: TaskStackFrameId,
    summaries: impl IntoIterator<Item = ValidatedTaskStackFrameSummary>,
) -> Result<ComposedTaskStackDemand, TaskPlanDiagnostic> {
    let mut frames = BTreeMap::new();
    for summary in summaries {
        let frame = summary.summary().frame;
        if frames.insert(frame, summary).is_some() {
            return Err(TaskPlanDiagnostic(format!(
                "task WCSU graph duplicates frame 0x{:016x}",
                frame.normalized_identity()
            )));
        }
    }
    if !frames.contains_key(&root) {
        return Err(TaskPlanDiagnostic(format!(
            "task WCSU graph has no root frame 0x{:016x}",
            root.normalized_identity()
        )));
    }
    for summary in frames.values() {
        for call in &summary.summary().calls {
            if let StackCallContribution::Checked { callee } = call
                && !frames.contains_key(callee)
            {
                return Err(TaskPlanDiagnostic(format!(
                    "task stack frame 0x{:016x} calls missing checked frame 0x{:016x}",
                    summary.summary().frame.normalized_identity(),
                    callee.normalized_identity()
                )));
            }
        }
    }

    let mut reachable = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    collect_reachable(root, &frames, &mut visiting, &mut reachable)?;
    if reachable.len() != frames.len() {
        let unreachable = frames
            .keys()
            .find(|frame| !reachable.contains(frame))
            .expect("different frame counts imply one unreachable frame");
        return Err(TaskPlanDiagnostic(format!(
            "task WCSU graph contains unreachable frame 0x{:016x}",
            unreachable.normalized_identity()
        )));
    }

    let mut memo = BTreeMap::new();
    let (bytes, alignment) = compose_frame(root, &frames, &mut memo)?;
    let frame_validations = frames
        .values()
        .map(|summary| summary.summary().validation)
        .collect();
    let admitted_contribution_report_identities = frames
        .values()
        .flat_map(|summary| summary.summary().calls.iter())
        .filter_map(|call| match call {
            StackCallContribution::AdmittedSameStack(contribution) => {
                Some(contribution.report_identity())
            }
            StackCallContribution::Checked { .. } => None,
        })
        .collect();
    let admitted_contribution_commitments = frames
        .values()
        .flat_map(|summary| summary.summary().calls.iter())
        .filter_map(|call| match call {
            StackCallContribution::AdmittedSameStack(contribution) => {
                Some(contribution.commitment())
            }
            StackCallContribution::Checked { .. } => None,
        })
        .collect();
    let unresolved_calls = frames
        .values()
        .flat_map(|summary| summary.summary().unresolved_calls.iter().cloned())
        .collect();
    let identity = TaskStackCompositionId(composition_report_fingerprint(
        root, &frames, bytes, alignment,
    ));
    Ok(ComposedTaskStackDemand {
        identity,
        root,
        bytes,
        alignment,
        contributing_frames: reachable,
        frame_validations,
        admitted_contribution_report_identities,
        admitted_contribution_commitments,
        unresolved_calls,
        evidence: TaskStackCompositionEvidence { root, frames },
    })
}

fn collect_reachable(
    frame: TaskStackFrameId,
    frames: &BTreeMap<TaskStackFrameId, ValidatedTaskStackFrameSummary>,
    visiting: &mut BTreeSet<TaskStackFrameId>,
    reachable: &mut BTreeSet<TaskStackFrameId>,
) -> Result<(), TaskPlanDiagnostic> {
    if reachable.contains(&frame) {
        return Ok(());
    }
    if !visiting.insert(frame) {
        return Err(TaskPlanDiagnostic(format!(
            "task WCSU graph contains a non-lowered call cycle through frame 0x{:016x}",
            frame.normalized_identity()
        )));
    }
    for call in &frames
        .get(&frame)
        .expect("checked endpoint validation ran above")
        .summary()
        .calls
    {
        if let StackCallContribution::Checked { callee } = call {
            collect_reachable(*callee, frames, visiting, reachable)?;
        }
    }
    visiting.remove(&frame);
    reachable.insert(frame);
    Ok(())
}

/// Return this frame's peak relative to a zero base. A caller aligns the
/// returned child chain at its own live-byte frontier.
fn compose_frame(
    frame: TaskStackFrameId,
    frames: &BTreeMap<TaskStackFrameId, ValidatedTaskStackFrameSummary>,
    memo: &mut BTreeMap<TaskStackFrameId, (u64, u64)>,
) -> Result<(u64, u64), TaskPlanDiagnostic> {
    if let Some(result) = memo.get(&frame) {
        return Ok(*result);
    }
    let summary = frames
        .get(&frame)
        .expect("checked endpoint validation ran above")
        .summary();
    // `alignment` constrains this frame's base. `local_bytes` is its exact
    // occupied extent; padding is needed only when placing a live child after
    // that extent.
    let local = summary.local_bytes;
    let mut peak = local;
    let mut alignment = summary.alignment;
    for call in &summary.calls {
        let (child_bytes, child_alignment) = match call {
            StackCallContribution::Checked { callee } => compose_frame(*callee, frames, memo)?,
            StackCallContribution::AdmittedSameStack(contribution) => {
                (contribution.bytes(), contribution.alignment())
            }
        };
        let child_base = align_up(local, child_alignment)?;
        let child_peak = child_base.checked_add(child_bytes).ok_or_else(|| {
            TaskPlanDiagnostic("task WCSU composition addition overflowed".into())
        })?;
        peak = peak.max(child_peak);
        alignment = alignment.max(child_alignment);
    }
    let result = (peak, alignment);
    memo.insert(frame, result);
    Ok(result)
}

fn align_up(value: u64, alignment: u64) -> Result<u64, TaskPlanDiagnostic> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum & !(alignment - 1))
        .ok_or_else(|| TaskPlanDiagnostic("task WCSU alignment overflowed".into()))
}

fn composition_report_fingerprint(
    root: TaskStackFrameId,
    frames: &BTreeMap<TaskStackFrameId, ValidatedTaskStackFrameSummary>,
    bytes: u64,
    alignment: u64,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(root.normalized_identity());
    hash.u64(frames.len() as u64);
    for summary in frames.values() {
        let summary = summary.summary();
        hash.u64(summary.frame.normalized_identity());
        hash.u64(summary.local_bytes);
        hash.u64(summary.alignment);
        hash.u64(summary.validation.normalized_identity());
        hash.u64(summary.calls.len() as u64);
        for call in &summary.calls {
            match call {
                StackCallContribution::Checked { callee } => {
                    hash.byte(1);
                    hash.u64(callee.normalized_identity());
                }
                StackCallContribution::AdmittedSameStack(contribution) => {
                    hash.byte(2);
                    hash.u64(contribution.report_identity().normalized_identity());
                    hash.u64(contribution.bytes());
                    hash.u64(contribution.alignment());
                }
            }
        }
        hash_unresolved_calls(&mut hash, summary.unresolved_calls.iter());
    }
    hash.u64(bytes);
    hash.u64(alignment);
    hash.finish_or(Fnv1a::OFFSET_BASIS)
}

fn hash_unresolved_calls<'a>(
    hash: &mut Fnv1a,
    sites: impl ExactSizeIterator<Item = &'a UnresolvedCallSite>,
) {
    hash.u64(sites.len() as u64);
    for site in sites {
        hash.u64(site.frame.normalized_identity());
        for byte in site.state.as_bytes() {
            hash.byte(*byte);
        }
        hash.byte(0);
        hash.u64(site.statement_index as u64);
        hash.u64(site.call_ordinal as u64);
        hash.byte(match site.kind {
            UnresolvedCallKind::UnresolvedTarget => 1,
            UnresolvedCallKind::NonCheckedSupply => 2,
        });
    }
}

fn admitted_contribution_report_fingerprint(
    candidate: &SameStackContributionAdmissionCandidate,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(candidate.provider_plan_report_identity);
    for byte in candidate.provider_plan_commitment.as_bytes() {
        hash.byte(byte);
    }
    // The requirement identity is a table index: hashing the index keeps the
    // fingerprint table-free so admission can re-run at codec decode.
    hash.u64(candidate.requirement_identity.0 as u64);
    hash.u64(candidate.receipt.normalized_identity());
    hash.u64(candidate.bytes);
    hash.u64(candidate.alignment);
    hash.finish_or(Fnv1a::OFFSET_BASIS)
}

fn admitted_contribution_commitment(
    candidate: &SameStackContributionAdmissionCandidate,
) -> SameStackContributionCommitment {
    let mut hash =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega.task.same-stack-contribution.v1\0");
    hash.u64(candidate.provider_plan_report_identity);
    hash.raw(&candidate.provider_plan_commitment.as_bytes());
    hash.raw(&candidate.requirement_identity.0.to_le_bytes());
    hash.raw(&candidate.receipt.normalized_identity().to_le_bytes());
    hash.u64(candidate.bytes);
    hash.u64(candidate.alignment);
    SameStackContributionCommitment(hash.finish().into())
}

fn stack_plan_projection_report_fingerprint(
    composition: TaskStackCompositionId,
    root: TaskStackFrameId,
    bytes: u64,
    alignment: u64,
    frame_validations: &BTreeSet<TaskStackFrameValidationId>,
    admitted_contribution_report_identities: &BTreeSet<AdmittedStackContributionReportId>,
    admitted_contribution_commitments: &BTreeSet<SameStackContributionCommitment>,
    unresolved_calls: &BTreeSet<UnresolvedCallSite>,
    representation: StackRepresentationId,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(composition.normalized_identity());
    hash.u64(root.normalized_identity());
    hash.u64(bytes);
    hash.u64(alignment);
    hash.u64(frame_validations.len() as u64);
    for validation in frame_validations {
        hash.u64(validation.normalized_identity());
    }
    hash.u64(admitted_contribution_report_identities.len() as u64);
    for contribution in admitted_contribution_report_identities {
        hash.u64(contribution.normalized_identity());
    }
    hash.u64(admitted_contribution_commitments.len() as u64);
    for commitment in admitted_contribution_commitments {
        for byte in commitment.as_bytes() {
            hash.byte(byte);
        }
    }
    hash_unresolved_calls(&mut hash, unresolved_calls.iter());
    hash.u64(representation.normalized_identity());
    hash.finish_or(Fnv1a::OFFSET_BASIS)
}
