use super::*;

pub(super) fn filter_expired_borrow_loans(
    borrow_weakenings: &mut psi_arena::Arena<FlowBorrowWeakeningFact>,
    constraint_refs: &mut psi_arena::Arena<FlowConstraintRef>,
    source: psi_arena::HandleSpan<FlowConstraintRef>,
    borrow: &BorrowFacts,
    statement_index: usize,
    reason: FlowBorrowWeakeningReason,
) -> psi_arena::HandleSpan<FlowConstraintRef> {
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
    borrow_weakenings: &mut psi_arena::Arena<FlowBorrowWeakeningFact>,
    constraint_refs: &mut psi_arena::Arena<FlowConstraintRef>,
    source: psi_arena::HandleSpan<FlowConstraintRef>,
    borrow: &BorrowFacts,
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) -> psi_arena::HandleSpan<FlowConstraintRef> {
    let (reassigned_symbol, reassigned_segments) = match statement {
        StatementNode::Assignment(assignment) => {
            let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            ) else {
                return source;
            };
            match place.root {
                psi_facts::PlaceRoot::Symbol(symbol) => (symbol, place.segments),
                _ => return source,
            }
        }
        StatementNode::AssemblyFact(_)
        | StatementNode::Call(_)
        | StatementNode::Expression(_)
        | StatementNode::LocalData(_)
        | StatementNode::Transition(_) => return source,
    };

    common::filter_constraint_refs(
        constraint_refs,
        source,
        |constraint_ref| match constraint_ref.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                let active = borrow.loans.get(loan);
                // Loans established by this assignment describe the replacement
                // value and must survive it. Older loans carried by the
                // overwritten whole/place are invalidated precisely.
                let keep = active.owner_symbol != reassigned_symbol
                    || active.statement_index == statement_index
                    || !borrow_owner_path_overlaps_place(
                        program,
                        borrow.loan_owner_path(active),
                        &reassigned_segments,
                    );
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

fn borrow_owner_path_overlaps_place(
    program: &psi_typed_trees::TypedTrees,
    owner_path: &[psi_checked_trees::BorrowLoanOwnerSegment],
    place_segments: &[psi_facts::PlaceSegment],
) -> bool {
    // An assignment retires loans carried by the exact target or anything
    // nested inside it. A loan carried by the whole owner (empty path) is not
    // retired by assigning through that owner's pointee/field; only replacing
    // the whole owner does that.
    place_segments.len() <= owner_path.len()
        && owner_path
            .iter()
            .zip(place_segments)
            .all(|(owner, place)| match (owner, place) {
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::Field(owner_symbol),
                    psi_facts::PlaceSegment::Field {
                        symbol: place_symbol,
                    },
                ) => !place_symbol.is_valid() || owner_symbol == place_symbol,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::Case(owner_variant),
                    psi_facts::PlaceSegment::Case {
                        variant: place_variant,
                    },
                ) => owner_variant == place_variant,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    psi_facts::PlaceSegment::FixedIndex { index: place_index },
                ) => owner_index == place_index,
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    psi_facts::PlaceSegment::Index { expression },
                ) => program
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_none_or(|place_index| *owner_index == place_index),
                (
                    psi_checked_trees::BorrowLoanOwnerSegment::DynamicIndex,
                    psi_facts::PlaceSegment::FixedIndex { .. }
                    | psi_facts::PlaceSegment::Index { .. },
                ) => true,
                _ => false,
            })
}
