use super::*;

pub(super) fn filter_expired_borrow_loans(
    borrow_weakenings: &mut omega_core::arena::Arena<FlowBorrowWeakeningFact>,
    constraint_refs: &mut omega_core::arena::Arena<FlowConstraintRef>,
    source: omega_core::arena::HandleSpan<FlowConstraintRef>,
    borrow: &BorrowFacts,
    statement_index: usize,
    reason: FlowBorrowWeakeningReason,
) -> omega_core::arena::HandleSpan<FlowConstraintRef> {
    common::filter_constraint_refs(
        constraint_refs,
        source,
        |constraint_ref| match constraint_ref.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                let keep = borrow.loans.get(loan).last_use_statement_index >= statement_index;
                if !keep {
                    borrow_weakenings.append(FlowBorrowWeakeningFact {
                        source: FlowInvalidationSource::Statement { statement_index },
                        loan,
                        reason,
                    });
                }
                keep
            }
            FlowConstraintKind::Unknown
            | FlowConstraintKind::SemanticContext { .. }
            | FlowConstraintKind::BorrowState { .. }
            | FlowConstraintKind::BorrowCall { .. }
            | FlowConstraintKind::BorrowWritableRoot { .. }
            | FlowConstraintKind::BorrowAccess { .. } => true,
        },
    )
}

pub(super) fn filter_reassigned_borrow_loans(
    borrow_weakenings: &mut omega_core::arena::Arena<FlowBorrowWeakeningFact>,
    constraint_refs: &mut omega_core::arena::Arena<FlowConstraintRef>,
    source: omega_core::arena::HandleSpan<FlowConstraintRef>,
    borrow: &BorrowFacts,
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) -> omega_core::arena::HandleSpan<FlowConstraintRef> {
    let reassigned_symbol = match statement {
        StatementNode::Assignment(assignment) => {
            let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            ) else {
                return source;
            };
            match (place.root, place.segments.is_empty()) {
                (omega_facts::PlaceRoot::Symbol(symbol), true) => symbol,
                _ => return source,
            }
        }
        StatementNode::Call(_)
        | StatementNode::Expression(_)
        | StatementNode::LocalData(_)
        | StatementNode::Transition(_) => return source,
    };

    common::filter_constraint_refs(
        constraint_refs,
        source,
        |constraint_ref| match constraint_ref.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                let keep = borrow.loans.get(loan).owner_symbol != reassigned_symbol;
                if !keep {
                    borrow_weakenings.append(FlowBorrowWeakeningFact {
                        source: FlowInvalidationSource::Statement { statement_index },
                        loan,
                        reason: FlowBorrowWeakeningReason::LocalReassigned,
                    });
                }
                keep
            }
            FlowConstraintKind::Unknown
            | FlowConstraintKind::SemanticContext { .. }
            | FlowConstraintKind::BorrowState { .. }
            | FlowConstraintKind::BorrowCall { .. }
            | FlowConstraintKind::BorrowWritableRoot { .. }
            | FlowConstraintKind::BorrowAccess { .. } => true,
        },
    )
}
