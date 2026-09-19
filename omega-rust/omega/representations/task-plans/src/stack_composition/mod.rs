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
//! and the roster rides the sealed projection so no `StackLease` can treat
//! the covered subgraph as the whole call graph.
//! Provider admission covers a sealed site by presenting a
//! [`CallTargetBinding`]: [`cover_unresolved_call_sites`] moves the site to a
//! checked callee edge, merges the bound callee's validated subtree into the
//! demand's frame evidence, and recomposes — a projection whose roster
//! empties publishes exact, while a site no binding names stays unresolved
//! and keeps rejecting.
//! Projections spelled `wcsu` carry the worst-case stack usage (WCSU) that the
//! storage contract defines.

use crate::{
    AdmittedStackContributionReportId, SameStackContributionAdmissionReceiptId, StackPlan,
    StackPlanProjectionId, StackRepresentationId, TaskPlanDiagnostic, TaskStackCompositionId,
    TaskStackFrameId, TaskStackFrameValidationId,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

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
    pub requirement_identity: String,
    pub receipt: SameStackContributionAdmissionReceiptId,
    pub bytes: u64,
    pub alignment: u64,
}

/// Domain-separated commitment to the exact selected provider plan that owns
/// one opaque same-stack contribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SameStackProviderPlanCommitment([u8; 32]);

impl SameStackProviderPlanCommitment {
    /// Wrap a digest issued by canonical provider-plan construction.
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

/// Domain-separated commitment to the complete admitted same-stack claim.
/// This remains authoritative when the compact report identity is projected
/// into later WCSU planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SameStackContributionCommitment([u8; 32]);

impl SameStackContributionCommitment {
    /// Reconstruct a retained non-authoritative commitment projection.
    /// Admission authority remains with [`AdmittedSameStackContribution`].
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

/// Sealed opaque same-stack demand accepted against an exact provider choice.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AdmittedSameStackContribution {
    report_identity: AdmittedStackContributionReportId,
    commitment: SameStackContributionCommitment,
    provider_plan_report_identity: u64,
    provider_plan_commitment: SameStackProviderPlanCommitment,
    requirement_identity: String,
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

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
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
    selected_requirement_identity: &str,
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
    if selected_requirement_identity.is_empty() {
        return Err(TaskPlanDiagnostic(
            "selected requirement identity for same-stack admission cannot be empty".into(),
        ));
    }
    if candidate.requirement_identity != selected_requirement_identity {
        return Err(TaskPlanDiagnostic(format!(
            "same-stack admission requirement identity {:?} does not match selected identity {selected_requirement_identity:?}",
            candidate.requirement_identity
        )));
    }
    if candidate.bytes == 0 {
        return Err(TaskPlanDiagnostic(
            "same-stack admission has zero WCSU".into(),
        ));
    }
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
/// before the plan can back a `StackLease`. The coordinate mirrors the
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
            if contribution.bytes() == 0 {
                return Err(TaskPlanDiagnostic(format!(
                    "admitted same-stack contribution 0x{:016x} has zero WCSU",
                    contribution.report_identity().normalized_identity()
                )));
            }
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
    hash.word(frame.normalized_identity());
    hash.word(local_bytes);
    hash.word(alignment);
    hash.word(calls.len() as u64);
    for call in &calls {
        match call {
            StackCallContribution::Checked { callee } => {
                hash.byte(1);
                hash.word(callee.normalized_identity());
            }
            StackCallContribution::AdmittedSameStack(contribution) => {
                hash.byte(2);
                hash.word(contribution.report_identity().normalized_identity());
            }
        }
    }
    hash_unresolved_calls(&mut hash, unresolved_calls.iter());
    TaskStackFrameValidationId::from_normalized_identity(hash.finish())
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

    /// Whether the bound covers every live call in the reachable graph. A
    /// partial bound is sealed evidence of the covered subgraph plus its
    /// unresolved roster — never the whole-call-graph WCSU.
    pub fn is_exact(&self) -> bool {
        self.unresolved_calls.is_empty()
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

    /// Whether the projected bound covers every live call in the reachable
    /// graph. A partial projection cannot back a `StackLease`.
    pub fn is_exact(&self) -> bool {
        self.unresolved_calls.is_empty()
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

/// Cover sealed [`UnresolvedCallSite`] rows with provider-admission call
/// target bindings and re-seal the projection.
///
/// Each [`CallTargetBinding`] names one unresolved site exactly — frame,
/// state, statement index and call ordinal — and carries the bound callee's
/// validated subtree, its root included. Covering first merges every bound
/// subtree into the composition's retained frame evidence: a subtree frame
/// whose validation does not bind its presented content, or whose content
/// conflicts with retained evidence, fails closed. Then each owning frame is
/// edited — the covered site leaves its unresolved roster and the bound
/// callee root enters its checked callee roster — and its validation is
/// re-minted by [`task_stack_frame_validation_identity`], so the covered
/// frame binds exactly what graph derivation would have bound had it
/// resolved the call itself.
///
/// The merged graph then recomposes through [`compose_task_stack_demand`] —
/// checked-edge closure, reachability and acyclicity all re-run, so a
/// binding that introduces a cycle, names a missing frame, or presents an
/// unreachable subtree fails closed — and the covered demand re-projects
/// into the same stack representation. The result publishes exact only when
/// no unresolved site remains: bindings never erase rows they did not name,
/// and a bound subtree's own unresolved sites stay in the roster.
pub fn cover_unresolved_call_sites(
    projection: &WcsuStackPlanProjection,
    bindings: &[CallTargetBinding],
) -> Result<WcsuStackPlanProjection, TaskPlanDiagnostic> {
    if !projection.has_valid_identity() {
        return Err(TaskPlanDiagnostic(
            "call target binding requires a projection whose sealed identity matches its \
             published bound"
                .into(),
        ));
    }
    let mut frames = projection.evidence.frames.clone();
    // Merge every bound subtree before editing any owning frame: a bound
    // subtree legitimately re-presents the caller frame in its pre-cover
    // shape, so site edits must apply to the fully merged evidence.
    for binding in bindings {
        if !binding
            .subtree
            .iter()
            .any(|summary| summary.summary().frame == binding.callee)
        {
            return Err(TaskPlanDiagnostic(format!(
                "call target binding for frame 0x{:016x} state `{}` statement {} call {} does \
                 not contain its callee root frame 0x{:016x}",
                binding.frame.normalized_identity(),
                binding.state,
                binding.statement_index,
                binding.call_ordinal,
                binding.callee.normalized_identity()
            )));
        }
        for subtree_summary in &binding.subtree {
            let presented = subtree_summary.summary();
            if presented.validation
                != task_stack_frame_validation_identity(
                    presented.frame,
                    presented.local_bytes,
                    presented.alignment,
                    &presented.calls,
                    &presented.unresolved_calls,
                )
            {
                return Err(TaskPlanDiagnostic(format!(
                    "call target binding merges frame 0x{:016x} whose validation does not bind \
                     its presented content",
                    presented.frame.normalized_identity()
                )));
            }
            match frames.get(&presented.frame) {
                Some(existing) if existing == subtree_summary => {}
                Some(_) => {
                    return Err(TaskPlanDiagnostic(format!(
                        "call target binding merges frame 0x{:016x} that conflicts with the \
                         composition's retained evidence",
                        presented.frame.normalized_identity()
                    )));
                }
                None => {
                    frames.insert(presented.frame, subtree_summary.clone());
                }
            }
        }
    }
    for binding in bindings {
        let Some(existing) = frames.get(&binding.frame) else {
            return Err(TaskPlanDiagnostic(format!(
                "call target binding names frame 0x{:016x} absent from the composition's \
                 evidence",
                binding.frame.normalized_identity()
            )));
        };
        let mut summary = existing.summary().clone();
        let position = summary
            .unresolved_calls
            .iter()
            .position(|site| {
                site.frame == binding.frame
                    && site.state == binding.state
                    && site.statement_index == binding.statement_index
                    && site.call_ordinal == binding.call_ordinal
            })
            .ok_or_else(|| {
                TaskPlanDiagnostic(format!(
                    "call target binding names no unresolved call site at frame 0x{:016x} \
                     state `{}` statement {} call {}",
                    binding.frame.normalized_identity(),
                    binding.state,
                    binding.statement_index,
                    binding.call_ordinal
                ))
            })?;
        summary.unresolved_calls.remove(position);
        summary.calls.push(StackCallContribution::Checked {
            callee: binding.callee,
        });
        summary.validation = task_stack_frame_validation_identity(
            summary.frame,
            summary.local_bytes,
            summary.alignment,
            &summary.calls,
            &summary.unresolved_calls,
        );
        let summary = validate_task_stack_frame_summary(summary)?;
        frames.insert(binding.frame, summary);
    }
    let covered = compose_task_stack_demand(projection.root, frames.into_values())?;
    Ok(project_wcsu_stack_plan(&covered, projection.representation))
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
    hash.word(root.normalized_identity());
    hash.word(frames.len() as u64);
    for summary in frames.values() {
        let summary = summary.summary();
        hash.word(summary.frame.normalized_identity());
        hash.word(summary.local_bytes);
        hash.word(summary.alignment);
        hash.word(summary.validation.normalized_identity());
        hash.word(summary.calls.len() as u64);
        for call in &summary.calls {
            match call {
                StackCallContribution::Checked { callee } => {
                    hash.byte(1);
                    hash.word(callee.normalized_identity());
                }
                StackCallContribution::AdmittedSameStack(contribution) => {
                    hash.byte(2);
                    hash.word(contribution.report_identity().normalized_identity());
                    hash.word(contribution.bytes());
                    hash.word(contribution.alignment());
                }
            }
        }
        hash_unresolved_calls(&mut hash, summary.unresolved_calls.iter());
    }
    hash.word(bytes);
    hash.word(alignment);
    hash.finish()
}

fn hash_unresolved_calls<'a>(
    hash: &mut Fnv1a,
    sites: impl ExactSizeIterator<Item = &'a UnresolvedCallSite>,
) {
    hash.word(sites.len() as u64);
    for site in sites {
        hash.word(site.frame.normalized_identity());
        for byte in site.state.as_bytes() {
            hash.byte(*byte);
        }
        hash.byte(0);
        hash.word(site.statement_index as u64);
        hash.word(site.call_ordinal as u64);
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
    hash.word(candidate.provider_plan_report_identity);
    for byte in candidate.provider_plan_commitment.as_bytes() {
        hash.byte(byte);
    }
    hash.string(&candidate.requirement_identity);
    hash.word(candidate.receipt.normalized_identity());
    hash.word(candidate.bytes);
    hash.word(candidate.alignment);
    hash.finish()
}

fn admitted_contribution_commitment(
    candidate: &SameStackContributionAdmissionCandidate,
) -> SameStackContributionCommitment {
    let mut hash = Sha256::new();
    hash.update(b"omega.task.same-stack-contribution.v1\0");
    hash.update(candidate.provider_plan_report_identity.to_le_bytes());
    hash.update(candidate.provider_plan_commitment.as_bytes());
    hash.update((candidate.requirement_identity.len() as u64).to_le_bytes());
    hash.update(candidate.requirement_identity.as_bytes());
    hash.update(candidate.receipt.normalized_identity().to_le_bytes());
    hash.update(candidate.bytes.to_le_bytes());
    hash.update(candidate.alignment.to_le_bytes());
    SameStackContributionCommitment(hash.finalize().into())
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
    hash.word(composition.normalized_identity());
    hash.word(root.normalized_identity());
    hash.word(bytes);
    hash.word(alignment);
    hash.word(frame_validations.len() as u64);
    for validation in frame_validations {
        hash.word(validation.normalized_identity());
    }
    hash.word(admitted_contribution_report_identities.len() as u64);
    for contribution in admitted_contribution_report_identities {
        hash.word(contribution.normalized_identity());
    }
    hash.word(admitted_contribution_commitments.len() as u64);
    for commitment in admitted_contribution_commitments {
        for byte in commitment.as_bytes() {
            hash.byte(byte);
        }
    }
    hash_unresolved_calls(&mut hash, unresolved_calls.iter());
    hash.word(representation.normalized_identity());
    hash.finish()
}

struct Fnv1a(u64);

impl Fnv1a {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    const fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn word(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }

    fn string(&mut self, value: &str) {
        self.word(value.len() as u64);
        for byte in value.bytes() {
            self.byte(byte);
        }
    }

    fn finish(self) -> u64 {
        if self.0 == 0 { Self::OFFSET } else { self.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AdmittedSameStackContribution, BTreeSet, CallTargetBinding,
        SameStackContributionAdmissionCandidate, SameStackContributionAdmissionReceiptId,
        SameStackProviderPlanCommitment, StackCallContribution, StackPlan, StackRepresentationId,
        TaskPlanDiagnostic, TaskStackFrameId, TaskStackFrameSummary, TaskStackFrameValidationId,
        UnresolvedCallKind, UnresolvedCallSite, ValidatedTaskStackFrameSummary,
        admit_same_stack_contribution, compose_task_stack_demand, cover_unresolved_call_sites,
        project_wcsu_stack_plan, task_stack_frame_validation_identity,
        validate_task_stack_frame_summary,
    };

    fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, TaskPlanDiagnostic>) -> T {
        constructor(identity).expect("normalized identity")
    }

    fn frame(
        identity: u64,
        bytes: u64,
        alignment: u64,
        calls: Vec<StackCallContribution>,
    ) -> ValidatedTaskStackFrameSummary {
        validate_task_stack_frame_summary(TaskStackFrameSummary {
            frame: id(identity, TaskStackFrameId::from_normalized_identity),
            local_bytes: bytes,
            alignment,
            validation: id(
                identity + 100,
                TaskStackFrameValidationId::from_normalized_identity,
            ),
            calls,
            unresolved_calls: Vec::new(),
        })
        .expect("valid local frame")
    }

    /// A frame whose validation is minted by the canonical derivation — what
    /// whole-call-graph derivation produces and what a call-target binding's
    /// presented subtree must carry.
    fn canonical_frame(
        identity: u64,
        bytes: u64,
        alignment: u64,
        calls: Vec<StackCallContribution>,
        unresolved_calls: Vec<UnresolvedCallSite>,
    ) -> ValidatedTaskStackFrameSummary {
        let frame = id(identity, TaskStackFrameId::from_normalized_identity);
        validate_task_stack_frame_summary(TaskStackFrameSummary {
            frame,
            local_bytes: bytes,
            alignment,
            validation: task_stack_frame_validation_identity(
                frame,
                bytes,
                alignment,
                &calls,
                &unresolved_calls,
            ),
            calls,
            unresolved_calls,
        })
        .expect("valid canonical frame")
    }

    fn provider_plan_commitment(marker: u8) -> SameStackProviderPlanCommitment {
        SameStackProviderPlanCommitment::from_digest([marker; 32])
    }

    fn admission(
        provider_plan_report_identity: u64,
        requirement_identity: &str,
        receipt_identity: u64,
        bytes: u64,
        alignment: u64,
    ) -> AdmittedSameStackContribution {
        let provider_plan_commitment = provider_plan_commitment(0x5a);
        admit_same_stack_contribution(
            SameStackContributionAdmissionCandidate {
                provider_plan_report_identity,
                provider_plan_commitment,
                requirement_identity: requirement_identity.into(),
                receipt: id(
                    receipt_identity,
                    SameStackContributionAdmissionReceiptId::from_normalized_identity,
                ),
                bytes,
                alignment,
            },
            provider_plan_report_identity,
            provider_plan_commitment,
            requirement_identity,
        )
        .expect("valid same-stack admission")
    }

    #[test]
    fn maximum_live_chain_uses_alignment_and_not_sibling_sum() {
        let root = id(1, TaskStackFrameId::from_normalized_identity);
        let left = id(2, TaskStackFrameId::from_normalized_identity);
        let right = id(3, TaskStackFrameId::from_normalized_identity);
        let demand = compose_task_stack_demand(
            root,
            [
                frame(
                    1,
                    24,
                    8,
                    vec![
                        StackCallContribution::Checked { callee: left },
                        StackCallContribution::Checked { callee: right },
                    ],
                ),
                frame(2, 32, 16, Vec::new()),
                frame(3, 80, 32, Vec::new()),
            ],
        )
        .expect("acyclic stack graph");

        assert_eq!(demand.bytes(), 112, "32-byte root base + max 80-byte child");
        assert_eq!(demand.alignment(), 32);
        assert_eq!(demand.contributing_frames().len(), 3);
        assert_eq!(demand.frame_validations().len(), 3);
    }

    #[test]
    fn admitted_same_stack_leaf_is_explicit_and_composed() {
        let root = id(10, TaskStackFrameId::from_normalized_identity);
        let admission = admission(0x100, "Codec::decode", 11, 48, 16);
        let admission_report_identity = admission.report_identity();
        let admission_commitment = admission.commitment();
        let demand = compose_task_stack_demand(
            root,
            [frame(
                10,
                24,
                8,
                vec![StackCallContribution::AdmittedSameStack(admission)],
            )],
        )
        .expect("admitted foreign leaf");

        assert_eq!(demand.bytes(), 80);
        assert_eq!(demand.alignment(), 16);
        assert_eq!(
            demand.admitted_contribution_report_identities(),
            &BTreeSet::from([admission_report_identity])
        );
        assert_eq!(
            demand.admitted_contribution_commitments(),
            &BTreeSet::from([admission_commitment])
        );
    }

    #[test]
    fn stack_plan_projection_binds_composition_evidence_and_representation() {
        let root = id(12, TaskStackFrameId::from_normalized_identity);
        let admission = admission(0x120, "Codec::decode", 13, 48, 16);
        let admission_report_identity = admission.report_identity();
        let admission_commitment = admission.commitment();
        let demand = compose_task_stack_demand(
            root,
            [frame(
                12,
                24,
                8,
                vec![StackCallContribution::AdmittedSameStack(admission)],
            )],
        )
        .expect("composed stack demand");
        let representation = id(14, StackRepresentationId::from_normalized_identity);
        let projection = project_wcsu_stack_plan(&demand, representation);

        assert_eq!(projection.composition(), demand.identity());
        assert_eq!(projection.root(), root);
        assert_eq!(projection.bytes(), 80);
        assert_eq!(projection.alignment(), 16);
        assert_eq!(projection.frame_validations(), demand.frame_validations());
        assert_eq!(
            projection.admitted_contribution_report_identities(),
            &BTreeSet::from([admission_report_identity])
        );
        assert_eq!(
            projection.admitted_contribution_commitments(),
            &BTreeSet::from([admission_commitment])
        );
        assert_eq!(projection.representation(), representation);
        assert_eq!(
            projection.stack_plan(),
            StackPlan {
                bytes: 80,
                alignment: 16,
                representation,
            }
        );
        assert_ne!(projection.identity().normalized_identity(), 0);

        let other_representation = project_wcsu_stack_plan(
            &demand,
            id(15, StackRepresentationId::from_normalized_identity),
        );
        assert_ne!(projection.identity(), other_representation.identity());
    }

    #[test]
    fn stack_plan_projection_identity_rejects_evidence_substitution() {
        let root = id(16, TaskStackFrameId::from_normalized_identity);
        let first = compose_task_stack_demand(root, [frame(16, 32, 16, Vec::new())])
            .expect("first composed demand");
        let mut changed_validation = frame(16, 32, 16, Vec::new());
        changed_validation.0.validation =
            id(0x1600, TaskStackFrameValidationId::from_normalized_identity);
        let second =
            compose_task_stack_demand(root, [changed_validation]).expect("second composed demand");
        let representation = id(17, StackRepresentationId::from_normalized_identity);
        let first_projection = project_wcsu_stack_plan(&first, representation);
        let mut substituted = project_wcsu_stack_plan(&second, representation);

        assert_eq!(first_projection.stack_plan(), substituted.stack_plan());
        assert_ne!(first_projection.identity(), substituted.identity());
        substituted.identity = first_projection.identity;
        assert!(
            !substituted.has_valid_identity(),
            "an identity from equal-shaped but different WCSU evidence must not validate"
        );
        let candidate = crate::ActivationPlanCandidate {
            machine_contract: id(0x1610, crate::MachineContractId::from_normalized_identity),
            entry: id(0x1611, crate::MachineEntryId::from_normalized_identity),
            argument_layout: crate::TaskArgumentLayout::new(
                id(0x1612, crate::ValueLayoutId::from_normalized_identity),
                &[],
            )
            .expect("empty canonical argument layout"),
            terminal_outcome_layout: id(0x1613, crate::ValueLayoutId::from_normalized_identity),
            calling_plan: id(0x1614, crate::CallingPlanId::from_normalized_identity),
            stack_plan: substituted.stack_plan(),
            may_suspend: false,
            may_block: false,
            canonical_suspension_crossings: Vec::new(),
            carry_obligations: crate::ActivationCarryObligations::none(),
            cancellation_required: false,
        };
        assert!(
            crate::validate_wcsu_activation_plan(candidate, substituted)
                .expect_err("substituted projection identity")
                .0
                .contains("projection identity")
        );
    }

    #[test]
    fn unresolved_call_sites_publish_a_partial_bound_through_the_projection() {
        let root = id(50, TaskStackFrameId::from_normalized_identity);
        let child = id(51, TaskStackFrameId::from_normalized_identity);
        let site = UnresolvedCallSite {
            frame: root,
            state: "run".into(),
            statement_index: 3,
            call_ordinal: 1,
            kind: UnresolvedCallKind::UnresolvedTarget,
        };
        let mut partial_root = frame(
            50,
            24,
            8,
            vec![StackCallContribution::Checked { callee: child }],
        );
        partial_root.0.unresolved_calls.push(site.clone());
        let demand = compose_task_stack_demand(root, [partial_root, frame(51, 32, 16, Vec::new())])
            .expect("the covered subgraph still composes");

        // 24-byte root extent, child aligned to 16 -> base 32, plus 32 bytes.
        assert_eq!(demand.bytes(), 64);
        assert!(!demand.is_exact());
        assert_eq!(demand.unresolved_calls(), &BTreeSet::from([site.clone()]));

        let representation = id(52, StackRepresentationId::from_normalized_identity);
        let projection = project_wcsu_stack_plan(&demand, representation);
        assert!(!projection.is_exact());
        assert_eq!(projection.unresolved_calls(), &BTreeSet::from([site]));

        let exact_demand = compose_task_stack_demand(
            root,
            [
                frame(
                    50,
                    24,
                    8,
                    vec![StackCallContribution::Checked { callee: child }],
                ),
                frame(51, 32, 16, Vec::new()),
            ],
        )
        .expect("the same subgraph without unresolved calls composes exactly");
        let exact_projection = project_wcsu_stack_plan(&exact_demand, representation);
        assert!(exact_projection.is_exact());
        assert_eq!(exact_projection.stack_plan(), projection.stack_plan());
        assert_ne!(
            exact_projection.identity(),
            projection.identity(),
            "a partial bound and an exact bound of equal shape must carry different identities"
        );
    }

    #[test]
    fn unresolved_call_site_must_name_its_owning_frame() {
        let root = id(60, TaskStackFrameId::from_normalized_identity);
        let error = validate_task_stack_frame_summary(TaskStackFrameSummary {
            frame: root,
            local_bytes: 24,
            alignment: 8,
            validation: id(160, TaskStackFrameValidationId::from_normalized_identity),
            calls: Vec::new(),
            unresolved_calls: vec![UnresolvedCallSite {
                frame: id(61, TaskStackFrameId::from_normalized_identity),
                state: "run".into(),
                statement_index: 0,
                call_ordinal: 0,
                kind: UnresolvedCallKind::NonCheckedSupply,
            }],
        })
        .expect_err("a site attributed to another frame must not validate");
        assert!(error.0.contains("unresolved call site attributed to frame"));
    }

    #[test]
    fn same_stack_admission_binds_selected_provider_requirement_and_receipt() {
        let provider_plan_commitment = provider_plan_commitment(0x20);
        let candidate = SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: 0x200,
            provider_plan_commitment,
            requirement_identity: "Codec::decode".into(),
            receipt: id(
                30,
                SameStackContributionAdmissionReceiptId::from_normalized_identity,
            ),
            bytes: 64,
            alignment: 16,
        };
        let admitted = admit_same_stack_contribution(
            candidate.clone(),
            0x200,
            provider_plan_commitment,
            "Codec::decode",
        )
        .expect("exact selection matches");

        assert_eq!(admitted.provider_plan_report_identity(), 0x200);
        assert_eq!(
            admitted.provider_plan_commitment(),
            provider_plan_commitment
        );
        assert_eq!(admitted.requirement_identity(), "Codec::decode");
        assert_eq!(admitted.receipt(), candidate.receipt);
        assert_eq!(admitted.bytes(), 64);
        assert_eq!(admitted.alignment(), 16);
        assert!(
            admit_same_stack_contribution(
                candidate.clone(),
                0x201,
                provider_plan_commitment,
                "Codec::decode",
            )
            .expect_err("provider-plan drift")
            .0
            .contains("does not match selected report identity")
        );
        assert!(
            admit_same_stack_contribution(
                candidate,
                0x200,
                provider_plan_commitment,
                "Codec::encode",
            )
            .expect_err("requirement drift")
            .0
            .contains("requirement identity")
        );
    }

    #[test]
    fn same_stack_admission_validates_shape_and_fingerprints_exact_evidence() {
        let first = admission(0x300, "Codec::decode", 40, 64, 16);
        let second_receipt = admission(0x300, "Codec::decode", 41, 64, 16);
        let second_requirement = admission(0x300, "Codec::decode.fast", 40, 64, 16);
        assert_ne!(first.report_identity(), second_receipt.report_identity());
        assert_ne!(
            first.report_identity(),
            second_requirement.report_identity()
        );
        assert_ne!(first.commitment(), second_receipt.commitment());
        assert_ne!(first.commitment(), second_requirement.commitment());

        let candidate = SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: 0x300,
            provider_plan_commitment: provider_plan_commitment(0x30),
            requirement_identity: "Codec::decode".into(),
            receipt: id(
                42,
                SameStackContributionAdmissionReceiptId::from_normalized_identity,
            ),
            bytes: 0,
            alignment: 16,
        };
        assert!(
            admit_same_stack_contribution(
                candidate.clone(),
                0x300,
                provider_plan_commitment(0x30),
                "Codec::decode",
            )
            .expect_err("zero demand")
            .0
            .contains("zero WCSU")
        );
        assert!(
            admit_same_stack_contribution(
                SameStackContributionAdmissionCandidate {
                    bytes: 1,
                    alignment: 24,
                    ..candidate
                },
                0x300,
                provider_plan_commitment(0x30),
                "Codec::decode",
            )
            .expect_err("invalid alignment")
            .0
            .contains("nonzero power of two")
        );
    }

    #[test]
    fn same_stack_admission_rejects_compact_equal_provider_plan_substitution() {
        let selected_commitment = provider_plan_commitment(0x31);
        let mut selected_candidate = SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: 0x300,
            provider_plan_commitment: selected_commitment,
            requirement_identity: "Codec::decode".into(),
            receipt: id(
                43,
                SameStackContributionAdmissionReceiptId::from_normalized_identity,
            ),
            bytes: 64,
            alignment: 16,
        };
        let selected = admit_same_stack_contribution(
            selected_candidate.clone(),
            0x300,
            selected_commitment,
            "Codec::decode",
        )
        .expect("canonical selected provider contribution");

        let substituted_commitment = provider_plan_commitment(0x32);
        selected_candidate.provider_plan_commitment = substituted_commitment;
        let substituted = admit_same_stack_contribution(
            selected_candidate.clone(),
            0x300,
            substituted_commitment,
            "Codec::decode",
        )
        .expect("the second exact provider plan is independently admissible");
        assert_eq!(
            selected.provider_plan_report_identity(),
            substituted.provider_plan_report_identity(),
            "the adversary deliberately holds the compact report coordinate equal",
        );
        assert_ne!(
            selected.commitment(),
            substituted.commitment(),
            "the strong provider-plan subject enters the admitted-contribution commitment",
        );

        let error = admit_same_stack_contribution(
            selected_candidate.clone(),
            selected_candidate.provider_plan_report_identity,
            selected_commitment,
            "Codec::decode",
        )
        .expect_err("compact-equal foreign provider plan must not be admitted");
        assert!(error.0.contains("commitment does not match"));

        let zero = SameStackProviderPlanCommitment::from_digest([0; 32]);
        let error = admit_same_stack_contribution(selected_candidate, 0x300, zero, "Codec::decode")
            .expect_err("an absent authoritative provider-plan subject must reject");
        assert!(error.0.contains("commitment") && error.0.contains("cannot be zero"));
    }

    #[test]
    fn missing_cycles_unreachable_and_overflow_fail_closed() {
        let root = id(20, TaskStackFrameId::from_normalized_identity);
        let missing = id(21, TaskStackFrameId::from_normalized_identity);
        assert!(
            compose_task_stack_demand(
                root,
                [frame(
                    20,
                    8,
                    8,
                    vec![StackCallContribution::Checked { callee: missing }],
                )],
            )
            .expect_err("missing callee")
            .0
            .contains("missing checked frame")
        );

        assert!(
            compose_task_stack_demand(
                root,
                [
                    frame(
                        20,
                        8,
                        8,
                        vec![StackCallContribution::Checked { callee: missing }],
                    ),
                    frame(
                        21,
                        8,
                        8,
                        vec![StackCallContribution::Checked { callee: root }],
                    ),
                ],
            )
            .expect_err("non-lowered cycle")
            .0
            .contains("non-lowered call cycle")
        );

        assert!(
            compose_task_stack_demand(
                root,
                [frame(20, 8, 8, Vec::new()), frame(22, 8, 8, Vec::new())]
            )
            .expect_err("unreachable frame")
            .0
            .contains("unreachable frame")
        );

        assert!(
            compose_task_stack_demand(
                root,
                [frame(
                    20,
                    u64::MAX,
                    8,
                    vec![StackCallContribution::AdmittedSameStack(admission(
                        0x400,
                        "Codec::decode",
                        23,
                        1,
                        8
                    ),)],
                )],
            )
            .expect_err("alignment overflow")
            .0
            .contains("alignment overflow")
        );
    }

    /// A partial projection whose root frame seals `sites` — the canonical
    /// validation derivation mints over the unresolved roster exactly as
    /// whole-call-graph derivation does.
    fn partial_projection(
        root: u64,
        sites: Vec<UnresolvedCallSite>,
        representation: u64,
    ) -> super::WcsuStackPlanProjection {
        let root_frame = id(root, TaskStackFrameId::from_normalized_identity);
        let partial_root = canonical_frame(root, 24, 8, Vec::new(), sites);
        let demand = compose_task_stack_demand(root_frame, [partial_root])
            .expect("the covered subgraph composes");
        assert!(!demand.is_exact());
        let projection = project_wcsu_stack_plan(
            &demand,
            id(
                representation,
                StackRepresentationId::from_normalized_identity,
            ),
        );
        assert!(!projection.is_exact());
        projection
    }

    fn site(
        frame: u64,
        state: &str,
        statement_index: usize,
        call_ordinal: usize,
    ) -> UnresolvedCallSite {
        UnresolvedCallSite {
            frame: id(frame, TaskStackFrameId::from_normalized_identity),
            state: state.into(),
            statement_index,
            call_ordinal,
            kind: UnresolvedCallKind::UnresolvedTarget,
        }
    }

    fn binding(
        site: &UnresolvedCallSite,
        subtree: Vec<ValidatedTaskStackFrameSummary>,
        callee: u64,
    ) -> CallTargetBinding {
        CallTargetBinding {
            frame: site.frame,
            state: site.state.clone(),
            statement_index: site.statement_index,
            call_ordinal: site.call_ordinal,
            callee: id(callee, TaskStackFrameId::from_normalized_identity),
            subtree,
        }
    }

    #[test]
    fn call_target_binding_covers_a_sealed_site_into_an_exact_bound() {
        let callee = id(71, TaskStackFrameId::from_normalized_identity);
        let unresolved = site(70, "run", 1, 0);
        let projection = partial_projection(70, vec![unresolved.clone()], 72);

        let covered = cover_unresolved_call_sites(
            &projection,
            &[binding(
                &unresolved,
                vec![canonical_frame(71, 32, 16, Vec::new(), Vec::new())],
                71,
            )],
        )
        .expect("the bound callee subtree covers the sealed site");

        // The bound callee charges beneath the root's 24-byte extent: its
        // base aligns to 16 -> 32, then its 32 bytes compose on top.
        assert_eq!(covered.bytes(), 64);
        assert_eq!(covered.alignment(), 16);
        assert!(covered.is_exact());
        assert!(covered.unresolved_calls().is_empty());
        assert!(covered.has_valid_identity());
        assert_ne!(
            covered.identity(),
            projection.identity(),
            "covering re-seals a new projection identity"
        );
        assert_ne!(
            covered.composition(),
            projection.composition(),
            "charging the bound callee composes a different demand"
        );

        // The covered projection is identical to the projection the graph
        // would have sealed had the call resolved to this callee state
        // directly — same shape, same frame evidence, same identity.
        let root = id(70, TaskStackFrameId::from_normalized_identity);
        let known = compose_task_stack_demand(
            root,
            [
                canonical_frame(
                    70,
                    24,
                    8,
                    vec![StackCallContribution::Checked { callee }],
                    Vec::new(),
                ),
                canonical_frame(71, 32, 16, Vec::new(), Vec::new()),
            ],
        )
        .expect("the graph-known shape composes");
        let known_projection = project_wcsu_stack_plan(
            &known,
            id(72, StackRepresentationId::from_normalized_identity),
        );
        assert_eq!(covered, known_projection);
    }

    #[test]
    fn call_target_binding_rejects_sites_and_subtrees_it_cannot_prove() {
        let unresolved = site(70, "run", 1, 0);
        let projection = partial_projection(70, vec![unresolved.clone()], 72);

        // A coordinate that names no sealed site covers nothing.
        let wrong_call = UnresolvedCallSite {
            call_ordinal: 1,
            ..unresolved.clone()
        };
        assert!(
            cover_unresolved_call_sites(
                &projection,
                &[binding(
                    &wrong_call,
                    vec![canonical_frame(71, 32, 16, Vec::new(), Vec::new())],
                    71,
                )],
            )
            .expect_err("a binding must name a sealed unresolved site")
            .0
            .contains("names no unresolved call site")
        );

        // A frame absent from the composition's evidence covers nothing.
        let absent_frame = site(90, "run", 0, 0);
        assert!(
            cover_unresolved_call_sites(
                &projection,
                &[binding(
                    &absent_frame,
                    vec![canonical_frame(71, 32, 16, Vec::new(), Vec::new())],
                    71,
                )],
            )
            .expect_err("a binding must name a frame in the composition")
            .0
            .contains("absent from the composition")
        );

        // A subtree that omits its callee root cannot charge the edge it
        // would create.
        assert!(
            cover_unresolved_call_sites(
                &projection,
                &[binding(
                    &unresolved,
                    vec![canonical_frame(73, 32, 16, Vec::new(), Vec::new())],
                    71,
                )],
            )
            .expect_err("a subtree must contain its callee root frame")
            .0
            .contains("does not contain its callee root")
        );

        // A subtree frame whose validation does not bind its content is
        // drifted evidence, not a derivation product.
        let mut drifted = canonical_frame(71, 32, 16, Vec::new(), Vec::new());
        drifted.0.validation = id(0x7fff, TaskStackFrameValidationId::from_normalized_identity);
        assert!(
            cover_unresolved_call_sites(&projection, &[binding(&unresolved, vec![drifted], 71)])
                .expect_err("subtree validation must bind its presented content")
                .0
                .contains("validation does not bind")
        );

        // A subtree frame that conflicts with retained evidence is a
        // collision, not a shared callee.
        let conflicting = canonical_frame(70, 64, 8, Vec::new(), Vec::new());
        assert!(
            cover_unresolved_call_sites(
                &projection,
                &[binding(&unresolved, vec![conflicting], 70)],
            )
            .expect_err("a subtree frame conflicting with retained evidence rejects")
            .0
            .contains("conflicts with the composition's retained evidence")
        );

        // Binding the site to a callee that calls back into the caller
        // discovers the cycle the unresolved edge had hidden.
        let root = id(70, TaskStackFrameId::from_normalized_identity);
        let cyclic_subtree = vec![canonical_frame(
            71,
            32,
            16,
            vec![StackCallContribution::Checked { callee: root }],
            Vec::new(),
        )];
        assert!(
            cover_unresolved_call_sites(&projection, &[binding(&unresolved, cyclic_subtree, 71)])
                .expect_err("a binding must not introduce a same-stack call cycle")
                .0
                .contains("non-lowered call cycle")
        );
    }

    #[test]
    fn call_target_binding_leaves_unbound_sites_partial() {
        let covered_site = site(70, "run", 1, 0);
        let still_unbound = site(70, "run", 5, 0);
        let projection =
            partial_projection(70, vec![covered_site.clone(), still_unbound.clone()], 72);

        let covered = cover_unresolved_call_sites(
            &projection,
            &[binding(
                &covered_site,
                vec![canonical_frame(71, 32, 16, Vec::new(), Vec::new())],
                71,
            )],
        )
        .expect("the named site covers");
        assert!(
            !covered.is_exact(),
            "a site no binding names stays unresolved"
        );
        assert_eq!(
            covered.unresolved_calls(),
            &BTreeSet::from([still_unbound]),
            "the unbound row keeps the projection partial"
        );

        // A bound subtree's own unresolved sites stay unresolved too: the
        // covered edge charges what the subtree bounds and keeps what it
        // cannot.
        let nested_site = site(73, "run", 0, 0);
        let projection = partial_projection(70, vec![covered_site.clone()], 72);
        let covered = cover_unresolved_call_sites(
            &projection,
            &[binding(
                &covered_site,
                vec![canonical_frame(
                    73,
                    32,
                    16,
                    Vec::new(),
                    vec![nested_site.clone()],
                )],
                73,
            )],
        )
        .expect("the bound subtree merges");
        assert!(!covered.is_exact());
        assert_eq!(covered.unresolved_calls(), &BTreeSet::from([nested_site]));
    }
}
