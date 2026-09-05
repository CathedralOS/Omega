//! Conditional call-path reconstruction for scalar stack composition.
//!
//! This module locates each retained call in the canonical conditional tree
//! and determines whether two call paths are mutually exclusive. It does not
//! validate branch bytes or compose stack demand.

use machine_code::{
    InternalCallRelocation, ScalarConditionalBranchEvidence, ScalarControlFlowEvidence,
    ScalarStackEvidence,
};
use target::Architecture;

pub(super) fn conditional_call_path(
    architecture: Architecture,
    bytes: &[u8],
    stack: Option<&ScalarStackEvidence>,
    call: &InternalCallRelocation,
) -> Option<Vec<(usize, bool)>> {
    let ScalarControlFlowEvidence::ConditionalTree { decisions, .. } = &stack?.control_flow else {
        return None;
    };
    let call_offset = match architecture {
        Architecture::X86_64 => call.offset.checked_sub(1)?,
        Architecture::Aarch64 => call.offset,
    };
    if call_offset >= bytes.len() {
        return None;
    }
    conditional_call_path_in_region(call_offset, 0, bytes.len(), decisions, &mut Vec::new())
}

fn conditional_call_path_in_region(
    call_offset: usize,
    start: usize,
    end: usize,
    decisions: &[ScalarConditionalBranchEvidence],
    path: &mut Vec<(usize, bool)>,
) -> Option<Vec<(usize, bool)>> {
    let Some((root, descendants)) = decisions.split_first() else {
        return (start <= call_offset && call_offset < end).then(|| path.clone());
    };
    if start <= call_offset && call_offset < root.branch_offset {
        return Some(path.clone());
    }
    let branch_end = root.branch_offset.checked_add(root.branch_byte_count)?;
    let true_decision_count =
        descendants.partition_point(|branch| branch.branch_offset < root.false_arm_offset);
    let (true_decisions, false_decisions) = descendants.split_at(true_decision_count);
    if branch_end <= call_offset && call_offset < root.false_arm_offset {
        path.push((root.branch_offset, true));
        let result = conditional_call_path_in_region(
            call_offset,
            branch_end,
            root.false_arm_offset,
            true_decisions,
            path,
        );
        path.pop();
        return result;
    }
    if root.false_arm_offset <= call_offset && call_offset < end {
        path.push((root.branch_offset, false));
        let result = conditional_call_path_in_region(
            call_offset,
            root.false_arm_offset,
            end,
            false_decisions,
            path,
        );
        path.pop();
        return result;
    }
    None
}

pub(super) fn conditional_paths_are_exclusive(
    left: &[(usize, bool)],
    right: &[(usize, bool)],
) -> bool {
    left.iter().any(|(left_decision, left_outcome)| {
        right.iter().any(|(right_decision, right_outcome)| {
            left_decision == right_decision && left_outcome != right_outcome
        })
    })
}
