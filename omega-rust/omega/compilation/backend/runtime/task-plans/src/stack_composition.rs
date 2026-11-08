/*
We compose whole-call-graph worst-case stack usage (WCSU) for one fixed-stack
task activation (`compose_task_stack_demand`, `project_wcsu_stack_plan`,
`admit_same_stack_contribution`).

Checked same-stack calls extend the live frame chain, and sequential sibling
calls share capacity so we take their maximum aligned chain rather than their
sum. Opaque same-stack leaves enter only through an `AdmittedSameStackContribution`
bound to the selected provider plan's 32-byte commitment and requirement
identity. Any live call the producer cannot resolve enters `UnresolvedCallSite`
and keeps the composed demand partial so a runtime never mistakes a covered
subgraph for the whole call graph.
*/

use super::validate_alignment;
use crate::identities::commitment_digest;
use crate::{
    AdmittedStackContributionReportId, CanonicalSuspensionCrossing,
    SameStackContributionAdmissionReceiptId, StackPlan, StackPlanProjectionId,
    StackRepresentationId, TaskPlanDiagnostic, TaskStackCompositionId, TaskStackFrameId,
    TaskStackFrameValidationId,
};
use semantic_vocabulary::Fnv1a;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameStackContributionAdmissionCandidate {
    pub provider_plan_report_identity: u64, // Historical compact report coordinate; not admission authority.
    pub provider_plan_commitment: SameStackProviderPlanCommitment, // Strong SHA-256 identity of the selected provider plan.
    pub requirement_identity: terminal_psi::TerminalIdentityIndex,
    pub receipt: SameStackContributionAdmissionReceiptId,
    pub bytes: u64,
    pub alignment: u64,
}

commitment_digest!(SameStackProviderPlanCommitment);
commitment_digest!(SameStackContributionCommitment);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnresolvedCallKind {
    UnresolvedTarget, // Requirement slot, machine parameter, or dynamic descriptor call.
    NonCheckedSupply, // Machine state supplied by requirement, boundary, admission-claim, or external realization.
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnresolvedCallSite {
    pub frame: TaskStackFrameId,
    pub state: String,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub kind: UnresolvedCallKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallTargetBinding {
    pub frame: TaskStackFrameId,
    pub state: String,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub callee: TaskStackFrameId,
    pub subtree: Vec<ValidatedTaskStackFrameSummary>,
    pub crossings: Vec<CanonicalSuspensionCrossing>,
}

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
    unresolved_calls: BTreeSet<UnresolvedCallSite>, // Non-empty means the bound covers only the resolved subgraph.
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

    pub const fn unresolved_calls(&self) -> &BTreeSet<UnresolvedCallSite> {
        &self.unresolved_calls
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WcsuStackPlanProjection {
    identity: StackPlanProjectionId,
    demand: ComposedTaskStackDemand,
    representation: StackRepresentationId,
}

impl WcsuStackPlanProjection {
    pub const fn identity(&self) -> StackPlanProjectionId {
        self.identity
    }

    pub const fn composition(&self) -> TaskStackCompositionId {
        self.demand.identity()
    }

    pub const fn root(&self) -> TaskStackFrameId {
        self.demand.root()
    }

    pub const fn bytes(&self) -> u64 {
        self.demand.bytes()
    }

    pub const fn alignment(&self) -> u64 {
        self.demand.alignment()
    }

    pub const fn frame_validations(&self) -> &BTreeSet<TaskStackFrameValidationId> {
        self.demand.frame_validations()
    }

    pub const fn admitted_contribution_report_identities(
        &self,
    ) -> &BTreeSet<AdmittedStackContributionReportId> {
        self.demand.admitted_contribution_report_identities()
    }

    pub const fn admitted_contribution_commitments(
        &self,
    ) -> &BTreeSet<SameStackContributionCommitment> {
        self.demand.admitted_contribution_commitments()
    }

    pub const fn unresolved_calls(&self) -> &BTreeSet<UnresolvedCallSite> {
        self.demand.unresolved_calls()
    }

    pub const fn representation(&self) -> StackRepresentationId {
        self.representation
    }

    pub const fn stack_plan(&self) -> StackPlan {
        StackPlan {
            bytes: self.demand.bytes(),
            alignment: self.demand.alignment(),
            representation: self.representation,
        }
    }

    pub(crate) fn has_valid_identity(&self) -> bool {
        self.identity
            == StackPlanProjectionId(stack_plan_projection_report_fingerprint(
                &self.demand,
                self.representation,
            ))
    }
}

pub fn project_wcsu_stack_plan(
    demand: &ComposedTaskStackDemand,
    representation: StackRepresentationId,
) -> WcsuStackPlanProjection {
    let identity = StackPlanProjectionId(stack_plan_projection_report_fingerprint(
        demand,
        representation,
    ));
    WcsuStackPlanProjection {
        identity,
        demand: demand.clone(),
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
    SameStackContributionCommitment::from_digest(hash.finish())
}

fn stack_plan_projection_report_fingerprint(
    demand: &ComposedTaskStackDemand,
    representation: StackRepresentationId,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(demand.identity().normalized_identity());
    hash.u64(demand.root().normalized_identity());
    hash.u64(demand.bytes());
    hash.u64(demand.alignment());
    let frame_validations = demand.frame_validations();
    hash.u64(frame_validations.len() as u64);
    for validation in frame_validations {
        hash.u64(validation.normalized_identity());
    }
    let admitted_contribution_report_identities = demand.admitted_contribution_report_identities();
    hash.u64(admitted_contribution_report_identities.len() as u64);
    for contribution in admitted_contribution_report_identities {
        hash.u64(contribution.normalized_identity());
    }
    let admitted_contribution_commitments = demand.admitted_contribution_commitments();
    hash.u64(admitted_contribution_commitments.len() as u64);
    for commitment in admitted_contribution_commitments {
        for byte in commitment.as_bytes() {
            hash.byte(byte);
        }
    }
    hash_unresolved_calls(&mut hash, demand.unresolved_calls().iter());
    hash.u64(representation.normalized_identity());
    hash.finish_or(Fnv1a::OFFSET_BASIS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_frame_component_retains_exact_admission_and_call_coverage() {
        let commitment = SameStackProviderPlanCommitment::from_digest([7; 32]);
        let requirement = terminal_psi::TerminalIdentityIndex(1);
        let candidate = SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: 17,
            provider_plan_commitment: commitment,
            requirement_identity: requirement,
            receipt: SameStackContributionAdmissionReceiptId::from_normalized_identity(19).unwrap(),
            bytes: 0,
            alignment: 16,
        };
        let contribution =
            admit_same_stack_contribution(candidate.clone(), 17, commitment, requirement)
                .expect("a leaf may allocate no additional frame");
        assert_eq!(contribution.bytes(), 0);
        let summary = validate_task_stack_frame_summary(TaskStackFrameSummary {
            frame: TaskStackFrameId::from_normalized_identity(23).unwrap(),
            local_bytes: 16,
            alignment: 16,
            validation: TaskStackFrameValidationId::from_normalized_identity(29).unwrap(),
            calls: vec![StackCallContribution::AdmittedSameStack(contribution)],
            unresolved_calls: Vec::new(),
        })
        .unwrap();
        assert_eq!(summary.summary().calls.len(), 1);
        let composed = compose_task_stack_demand(summary.summary().frame, [summary]).unwrap();
        assert_eq!(composed.bytes(), 16);
        assert_eq!(composed.admitted_contribution_report_identities().len(), 1);
        assert!(composed.unresolved_calls().is_empty());
        assert!(
            admit_same_stack_contribution(candidate.clone(), 18, commitment, requirement).is_err()
        );
        let invalid = SameStackContributionAdmissionCandidate {
            alignment: 3,
            ..candidate
        };
        assert!(admit_same_stack_contribution(invalid, 17, commitment, requirement).is_err());
    }
}
