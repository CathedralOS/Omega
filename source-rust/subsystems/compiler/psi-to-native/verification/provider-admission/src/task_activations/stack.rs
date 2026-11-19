//! Compose the checked call frames for this activation and retain unresolved
//! sites in the projection that checks the proposed fixed stack.
use super::{TaskPlanDiagnostic, validate_alignment};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::TerminalTaskUnresolvedCallKind;
use wire::fnv1a::Fnv1a;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct UnresolvedCallSite {
    pub(super) frame: u64,
    pub(super) state: String,
    pub(super) statement_index: usize,
    pub(super) call_ordinal: usize,
    pub(super) kind: TerminalTaskUnresolvedCallKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TaskStackFrameSummary {
    pub(super) frame: u64,
    pub(super) local_bytes: u64,
    pub(super) alignment: u64,
    pub(super) validation: u64,
    pub(super) calls: Vec<u64>,
    pub(super) unresolved_calls: Vec<UnresolvedCallSite>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ValidatedTaskStackFrameSummary(TaskStackFrameSummary);

impl ValidatedTaskStackFrameSummary {
    pub(super) const fn summary(&self) -> &TaskStackFrameSummary {
        &self.0
    }
}

pub(super) fn validate_task_stack_frame_summary(
    mut summary: TaskStackFrameSummary,
) -> Result<ValidatedTaskStackFrameSummary, TaskPlanDiagnostic> {
    if summary.local_bytes == 0 {
        return Err(TaskPlanDiagnostic(format!(
            "task stack frame 0x{:016x} has zero local WCSU",
            summary.frame
        )));
    }
    validate_alignment(
        summary.alignment,
        &format!("task stack frame 0x{:016x}", summary.frame),
    )?;
    for site in &summary.unresolved_calls {
        if site.frame != summary.frame {
            return Err(TaskPlanDiagnostic(format!(
                "task stack frame 0x{:016x} carries an unresolved call site attributed to frame \
                 0x{:016x}",
                summary.frame, site.frame
            )));
        }
    }
    summary.calls.sort_unstable();
    summary.calls.dedup();
    summary.unresolved_calls.sort_unstable();
    summary.unresolved_calls.dedup();
    Ok(ValidatedTaskStackFrameSummary(summary))
}

pub(super) fn task_stack_frame_validation_identity(
    frame: u64,
    local_bytes: u64,
    alignment: u64,
    calls: &[u64],
    unresolved_calls: &[UnresolvedCallSite],
) -> u64 {
    let mut calls = calls.to_vec();
    calls.sort_unstable();
    let mut unresolved_calls = unresolved_calls.to_vec();
    unresolved_calls.sort_unstable();
    let mut hash = Fnv1a::new();
    hash.byte(0x56);
    hash.u64(frame);
    hash.u64(local_bytes);
    hash.u64(alignment);
    hash.u64(calls.len() as u64);
    for call in &calls {
        hash.byte(1);
        hash.u64(*call);
    }
    hash_unresolved_calls(&mut hash, unresolved_calls.iter());
    hash.finish_or(Fnv1a::OFFSET_BASIS)
}

#[derive(Debug)]
pub(super) struct ComposedTaskStackDemand {
    identity: u64,
    root: u64,
    pub(super) bytes: u64,
    pub(super) alignment: u64,
    frame_validations: BTreeSet<u64>,
    unresolved_calls: BTreeSet<UnresolvedCallSite>,
}

pub(super) struct StackProjection<'a> {
    identity: u64,
    demand: &'a ComposedTaskStackDemand,
    representation: u64,
}

impl StackProjection<'_> {
    pub(super) fn validate(&self, bytes: u64, alignment: u64) -> Result<(), TaskPlanDiagnostic> {
        if bytes == 0 {
            return Err(TaskPlanDiagnostic(
                "activation stack size must be nonzero".into(),
            ));
        }
        validate_alignment(alignment, "activation stack")?;
        if self.identity
            != stack_plan_projection_report_fingerprint(self.demand, self.representation)
        {
            return Err(TaskPlanDiagnostic("WCSU stack-plan projection identity does not match its retained composition evidence".into()));
        }
        if (bytes, alignment) != (self.demand.bytes, self.demand.alignment) {
            return Err(TaskPlanDiagnostic(
                "activation stack shape does not exactly match its WCSU stack-plan projection"
                    .into(),
            ));
        }
        Ok(())
    }
}

pub(super) fn project_wcsu_stack_plan(
    demand: &ComposedTaskStackDemand,
    representation: u64,
) -> StackProjection<'_> {
    StackProjection {
        identity: stack_plan_projection_report_fingerprint(demand, representation),
        demand,
        representation,
    }
}

pub(super) fn compose_task_stack_demand(
    root: u64,
    summaries: impl IntoIterator<Item = ValidatedTaskStackFrameSummary>,
) -> Result<ComposedTaskStackDemand, TaskPlanDiagnostic> {
    let mut frames = BTreeMap::new();
    for summary in summaries {
        let frame = summary.summary().frame;
        if frames.insert(frame, summary).is_some() {
            return Err(TaskPlanDiagnostic(format!(
                "task WCSU graph duplicates frame 0x{:016x}",
                frame
            )));
        }
    }
    if !frames.contains_key(&root) {
        return Err(TaskPlanDiagnostic(format!(
            "task WCSU graph has no root frame 0x{:016x}",
            root
        )));
    }
    for summary in frames.values() {
        for call in &summary.summary().calls {
            if !frames.contains_key(call) {
                return Err(TaskPlanDiagnostic(format!(
                    "task stack frame 0x{:016x} calls missing checked frame 0x{:016x}",
                    summary.summary().frame,
                    call
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
            unreachable
        )));
    }

    let mut memo = BTreeMap::new();
    let (bytes, alignment) = compose_frame(root, &frames, &mut memo)?;
    let frame_validations = frames
        .values()
        .map(|summary| summary.summary().validation)
        .collect();
    let unresolved_calls = frames
        .values()
        .flat_map(|summary| summary.summary().unresolved_calls.iter().cloned())
        .collect();
    let identity = composition_report_fingerprint(root, &frames, bytes, alignment);
    Ok(ComposedTaskStackDemand {
        identity,
        root,
        bytes,
        alignment,
        frame_validations,
        unresolved_calls,
    })
}

fn collect_reachable(
    frame: u64,
    frames: &BTreeMap<u64, ValidatedTaskStackFrameSummary>,
    visiting: &mut BTreeSet<u64>,
    reachable: &mut BTreeSet<u64>,
) -> Result<(), TaskPlanDiagnostic> {
    if reachable.contains(&frame) {
        return Ok(());
    }
    if !visiting.insert(frame) {
        return Err(TaskPlanDiagnostic(format!(
            "task WCSU graph contains a non-lowered call cycle through frame 0x{:016x}",
            frame
        )));
    }
    for call in &frames
        .get(&frame)
        .expect("checked endpoint validation ran above")
        .summary()
        .calls
    {
        collect_reachable(*call, frames, visiting, reachable)?;
    }
    visiting.remove(&frame);
    reachable.insert(frame);
    Ok(())
}

fn compose_frame(
    frame: u64,
    frames: &BTreeMap<u64, ValidatedTaskStackFrameSummary>,
    memo: &mut BTreeMap<u64, (u64, u64)>,
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
        let (child_bytes, child_alignment) = compose_frame(*call, frames, memo)?;
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
    root: u64,
    frames: &BTreeMap<u64, ValidatedTaskStackFrameSummary>,
    bytes: u64,
    alignment: u64,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(root);
    hash.u64(frames.len() as u64);
    for summary in frames.values() {
        let summary = summary.summary();
        hash.u64(summary.frame);
        hash.u64(summary.local_bytes);
        hash.u64(summary.alignment);
        hash.u64(summary.validation);
        hash.u64(summary.calls.len() as u64);
        for call in &summary.calls {
            hash.byte(1);
            hash.u64(*call);
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
        hash.u64(site.frame);
        for byte in site.state.as_bytes() {
            hash.byte(*byte);
        }
        hash.byte(0);
        hash.u64(site.statement_index as u64);
        hash.u64(site.call_ordinal as u64);
        hash.byte(match site.kind {
            TerminalTaskUnresolvedCallKind::UnresolvedTarget => 1,
            TerminalTaskUnresolvedCallKind::NonCheckedSupply => 2,
        });
    }
}

fn stack_plan_projection_report_fingerprint(
    demand: &ComposedTaskStackDemand,
    representation: u64,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(demand.identity);
    hash.u64(demand.root);
    hash.u64(demand.bytes);
    hash.u64(demand.alignment);
    let frame_validations = &demand.frame_validations;
    hash.u64(frame_validations.len() as u64);
    for validation in frame_validations {
        hash.u64(*validation);
    }
    // Published activation demands carry checked call edges. Opaque provider
    // report IDs and commitments enter the physical call evidence later.
    hash.u64(0);
    hash.u64(0);
    hash_unresolved_calls(&mut hash, demand.unresolved_calls.iter());
    hash.u64(representation);
    hash.finish_or(Fnv1a::OFFSET_BASIS)
}
