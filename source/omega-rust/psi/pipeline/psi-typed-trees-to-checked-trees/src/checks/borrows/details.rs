use psi_checked_trees::{CheckFacts, FlowStateFact};

pub(super) fn active_loan_detail(
    state_flow: &FlowStateFact,
    facts: &CheckFacts,
    loan: psi_arena::Handle<psi_checked_trees::BorrowLoanFact>,
    statement_index: usize,
) -> Option<String> {
    facts
        .flow.borrow_lifetimes.weakenings
        .span_or_empty(state_flow.borrow_weakenings)
        .iter()
        .find(|weakening| weakening.loan == loan)
        .and_then(|weakening| {
            let loan = facts.borrow.loans.get(loan);
            match (weakening.reason, weakening.source) {
                (
                    psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                    psi_checked_trees::FlowInvalidationSource::Statement {
                        statement_index: weakening_statement,
                    },
                ) if weakening_statement > statement_index => Some(format!(
                    "borrowed at statement {}; its last use is at statement {}",
                    loan.statement_index, loan.last_use_statement_index
                )),
                (
                    psi_checked_trees::FlowBorrowWeakeningReason::StateExit,
                    psi_checked_trees::FlowInvalidationSource::Statement { .. },
                ) if loan.last_use_statement_index > statement_index => Some(format!(
                    "borrowed at statement {}; its last use is at statement {} and it is released at state exit",
                    loan.statement_index, loan.last_use_statement_index
                )),
                (
                    psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                    psi_checked_trees::FlowInvalidationSource::Statement {
                        statement_index: weakening_statement,
                    },
                ) if weakening_statement > statement_index => Some(format!(
                    "borrowed at statement {}; it is reassigned at statement {}",
                    loan.statement_index, weakening_statement
                )),
                (
                    psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                    psi_checked_trees::FlowInvalidationSource::Statement { .. },
                )
                | (
                    psi_checked_trees::FlowBorrowWeakeningReason::StateExit,
                    psi_checked_trees::FlowInvalidationSource::Statement { .. },
                )
                | (
                    psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                    psi_checked_trees::FlowInvalidationSource::Statement { .. },
                )
                | (
                    psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                    psi_checked_trees::FlowInvalidationSource::Call { .. },
                )
                | (
                    psi_checked_trees::FlowBorrowWeakeningReason::StateExit,
                    psi_checked_trees::FlowInvalidationSource::Call { .. },
                )
                | (
                    psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                    psi_checked_trees::FlowInvalidationSource::Call { .. },
                ) => None,
            }
        })
}

pub(super) fn canonical_place_label(
    program: &psi_typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
) -> String {
    crate::labels::canonical_place_label_from_parts(program, place.root, &place.segments)
}
