//! Internal arrival follows argument capture and the selected residual cleanup.
//! A lexical last-use row cannot close a reference stored inside a nominal
//! receiver: the hook runs before structural field cleanup. Join the loan home
//! to that exact edge's disposition, excluding transferred or inactive leaves.
//! The caller separately rules out carrying local loans into the successor.
use checked_trees::{BorrowCallFact, BorrowLoanFact, CheckFacts, FlowStateFact};
use language_semantics::PermissionEventSource;
use typed_trees::statement::TransitionTargetNode;

pub(super) fn disposes_loan(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    call: &BorrowCallFact,
    loan: &BorrowLoanFact,
) -> bool {
    let Some(jump) = super::statements::local_jump(program, flow, call) else {
        return false;
    };
    let validation::CallSite::TransitionNamed { path, .. } = jump.site else {
        return false;
    };
    let Some(source) = program.state_by_symbol(flow.state_symbol) else {
        return false;
    };
    let Some(transition) = program
        .statement_table
        .statement_at(source, call.statement_index)
        .and_then(|statement| statement.as_transition())
    else {
        return false;
    };
    [transition.target, transition.continuation]
        .into_iter()
        .filter(|target| target.is_valid())
        .any(|target| {
            let TransitionTargetNode::Named { path: selected, .. } =
                program.statement_table.transition_target(target)
            else {
                return false;
            };
            // Exact immutable source occurrence, not an equal callee label.
            std::ptr::eq(path, selected)
                && disposes_loan_on_target(program, facts, flow, call.statement_index, target, loan)
        })
}

pub(super) fn disposes_loan_on_target(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    statement_index: usize,
    target: typed_trees::statement::TransitionTargetHandle,
    loan: &BorrowLoanFact,
) -> bool {
    let Some(source) = program.state_by_symbol(flow.state_symbol) else {
        return false;
    };
    let Some(transition) = program
        .statement_table
        .statement_at(source, statement_index)
        .and_then(|statement| statement.as_transition())
    else {
        return false;
    };
    let cleanup = &facts.contextual_cleanup;
    let owner_path = facts.borrow.loan_owner_path(loan);
    cleanup.dispositions.iter().any(|(_, disposition)| {
        if disposition.machine != flow.machine_symbol
            || disposition.state != flow.state_symbol
            || disposition.source != PermissionEventSource::StateExit
            || disposition.exit.state_symbol != flow.state_symbol
            || disposition.exit.statement_index != statement_index
            || disposition.root.as_symbol() != Some(loan.owner_symbol)
            || disposition.zero_image
            || disposition.completion.is_valid()
        {
            return false;
        }
        let disposed_target = match disposition.exit.kind {
            checked_trees::FlowClaimJoinExitKind::TransitionTarget => transition.target,
            checked_trees::FlowClaimJoinExitKind::TransitionContinuation => transition.continuation,
            _ => return false,
        };
        if !target.is_valid() || disposed_target != target {
            return false;
        }
        if !crate::borrow::place_path_covers_owner(
            program,
            facts
                .flow
                .ownership
                .segments
                .span_or_empty(disposition.path),
            owner_path,
        ) {
            return false;
        }
        [disposition.exclusions, disposition.inactive_cases]
            .into_iter()
            .flat_map(|regions| cleanup.exclusions.span_or_empty(regions))
            .all(|excluded| {
                excluded.root != disposition.root
                    || !crate::borrow::owner_path_overlaps_place_segments(
                        program,
                        owner_path,
                        cleanup.segments.span_or_empty(excluded.segments),
                    )
            })
    })
}
