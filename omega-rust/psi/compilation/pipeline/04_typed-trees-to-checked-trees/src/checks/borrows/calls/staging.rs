//! Local state arrivals evaluate arguments in authored order and capture scalar
//! snapshots before committing the arrival transfer of existing reference carriers. A copied observation through that carrier's own route
//! does not create a competing successor loan. This follows state_contracts.md
//! (Bindings and arrivals) and expressions.md (Evaluation schedule), independent
//! of attached-data namespaces. The distinction is transfer versus formation:
//! explicit new borrows, retained observations, other alias routes, nested-call
//! writes and ordinary returning calls keep their normal conflict judgments.
//! Active loans are checked separately against every original access row; this
//! judgment neither restores suspended parent authority nor extends backing.

use checked_trees::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallFact, CheckFacts, FlowStateFact,
};
use typed_trees::expression::ExpressionNode;

/// A staged access ends at its producer-owned completion, not when the next
/// accessed place happens to be observed. Explicit moves supply the prefix
/// before their occurrence; an implicit completion supplies the inclusive
/// prefix after completion. Both keep the same branch selection.
pub(in crate::checks::borrows) fn access_released_in_prefix(
    access: &BorrowArgumentAccessFact,
    prefix: &[crate::checks::ownership_occurrences::StatementOccurrence],
    route: &[crate::checks::ownership_occurrences::EvaluationBranch],
) -> bool {
    use crate::checks::ownership_occurrences::{StatementStep, routes_compatible};
    access.end_before_access == Some(0)
        || (access.released_at.is_valid()
            && prefix.iter().any(|occurrence| {
                matches!(occurrence.step, StatementStep::Complete(expression)
                    if expression == access.released_at)
                    && routes_compatible(&occurrence.route, route)
            }))
}

pub(super) fn copied_read_before_reference_transfer(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
    transferred: &BorrowArgumentAccessFact,
    copied: &BorrowArgumentAccessFact,
) -> bool {
    if super::super::statements::local_jump(program, state_flow, call).is_none()
        || transferred.kind != BorrowAccessKind::Mutable
        || transferred.end_before_access.is_some()
        || copied.kind != BorrowAccessKind::Read
        || copied.end_before_access.is_none()
        || transferred.root != copied.root
        || !facts
            .borrow
            .access_segments(copied)
            .starts_with(facts.borrow.access_segments(transferred))
    {
        return false;
    }
    // Reference values forward their established loan. A newly authored
    // borrow starts a child lifetime during argument evaluation instead.
    let mut expression = transferred.expression;
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(_) | ExpressionNode::Call(_) => return false,
            // Reference recasts form a new loan over backing storage. Other
            // casts preserve their operand's formation provenance; they must
            // not hide a borrow beneath a qualified result type.
            ExpressionNode::Cast(cast) => {
                if cast.form.is_recast() {
                    return false;
                }
                expression = cast.value;
            }
            _ => break,
        }
    }
    validation::expression_type_reference_in_state(
        program,
        state_flow.state_symbol,
        call.statement_index,
        expression,
    )
    .and_then(|reference| validation::reference_shell(program, reference))
    .is_some_and(|reference| reference.access == language_semantics::ReferenceAccess::Mutable)
}
