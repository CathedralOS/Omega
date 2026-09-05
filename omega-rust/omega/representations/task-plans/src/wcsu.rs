//! Whole-call-graph stack composition for one fixed-stack activation.
//!
//! Checked same-stack calls extend the live frame chain. Sequential sibling
//! calls share capacity, so composition takes their maximum rather than their
//! sum. Opaque same-stack leaves enter only through an exact admitted
//! contribution; a provider-stack or new-activation transfer contributes no
//! child frame to this stack and therefore has no edge in this graph.

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

/// Compiler-produced local frame facts before whole-graph composition.
/// `local_bytes` includes target calling/entry overhead owned by this frame;
/// every child begins while those bytes remain live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStackFrameSummary {
    pub frame: TaskStackFrameId,
    pub local_bytes: u64,
    pub alignment: u64,
    pub validation: TaskStackFrameValidationId,
    pub calls: Vec<StackCallContribution>,
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
    summary.calls.sort_unstable();
    summary.calls.dedup();
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskStackCompositionEvidence {
    root: TaskStackFrameId,
    frames: BTreeMap<TaskStackFrameId, ValidatedTaskStackFrameSummary>,
}

/// Sealed maximum live stack chain for one fixed-stack activation.
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
}

/// Sealed projection of one composed WCSU demand into a physical fixed-stack
/// representation.
///
/// The compact composition identity is not used as a substitute for the facts
/// a stack allocator and activation fingerprint rely on. The projection also
/// retains the exact root, composed shape, frame-validation set, admitted
/// contribution set, and selected representation.
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
    representation: StackRepresentationId,
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
                self.representation,
            ))
    }
}

/// Bind one validated whole-call-graph demand to the exact fixed-stack
/// representation that will provision it.
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
    let identity = StackPlanProjectionId(stack_plan_projection_report_fingerprint(
        composition,
        root,
        bytes,
        alignment,
        &frame_validations,
        &admitted_contribution_report_identities,
        &admitted_contribution_commitments,
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
        representation,
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
    }
    hash.word(bytes);
    hash.word(alignment);
    hash.finish()
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
    use super::*;

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
        })
        .expect("valid local frame")
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
            argument_layout: id(0x1612, crate::ValueLayoutId::from_normalized_identity),
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
}
