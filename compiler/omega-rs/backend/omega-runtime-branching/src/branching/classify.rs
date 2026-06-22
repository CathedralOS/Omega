use super::{RuntimeBranchCallExpansion, RuntimeBranchTargetLowering, RuntimeBranchingCallEdge};
use omega_state_guards::StateGuardKind;

pub(super) fn classify_branch_call_expansion(
    edges: &[RuntimeBranchingCallEdge],
) -> RuntimeBranchCallExpansion {
    if edges.is_empty() {
        return RuntimeBranchCallExpansion::Unplanned;
    }

    let mut has_unknown_target = false;
    let mut has_straight_line_target = false;
    let mut has_nested_branching_target = false;
    let mut has_complex_guard = false;

    for edge in edges {
        match edge.lowering {
            RuntimeBranchTargetLowering::Terminal | RuntimeBranchTargetLowering::InlineLeaf => {}
            RuntimeBranchTargetLowering::InlineStraightLine => has_straight_line_target = true,
            RuntimeBranchTargetLowering::InlineBranching => has_nested_branching_target = true,
            RuntimeBranchTargetLowering::Unknown => has_unknown_target = true,
        }

        if !matches!(
            edge.guard_kind,
            StateGuardKind::Always
                | StateGuardKind::RuntimeEquality
                | StateGuardKind::RuntimeInequality
                | StateGuardKind::RuntimeOrdering
        ) {
            has_complex_guard = true;
        }
    }

    if has_unknown_target {
        return RuntimeBranchCallExpansion::UnknownTarget;
    }

    if has_nested_branching_target {
        return RuntimeBranchCallExpansion::NeedsNestedBranchTarget;
    }

    if has_straight_line_target {
        return RuntimeBranchCallExpansion::NeedsStraightLineTarget;
    }

    if has_complex_guard {
        return RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards;
    }

    RuntimeBranchCallExpansion::GuardedLeaf
}
