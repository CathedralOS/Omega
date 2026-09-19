use checked_trees::statement::StatementNode;
use checked_trees::{CheckFacts, FlowStateFact};
use symbols::SymbolHandle;

/// The checked access a reference-typed binding's declaration lends over its
/// referent — the parent authority for any borrow expression formed on that
/// binding. `&T`/`&mut T`/`&write T` bindings answer `Read`/`Mutable`/
/// `WriteOnly`; value bindings, attached storage, and machine-owned roots
/// answer `None` and stay on the writable-place gate instead. Locals are only
/// visible before `statement_index`, matching how the borrow places resolve
/// them.
pub(super) fn binding_reference_access(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    root_symbol: SymbolHandle,
) -> Option<checked_trees::BorrowAccessKind> {
    let state =
        crate::semantic_calls::find_state_in_machine(program, machine_symbol, state_symbol)?;
    let type_reference = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == root_symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(statement_index)
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.symbol == root_symbol => {
                        Some(local.type_reference)
                    }
                    _ => None,
                })
        })?;
    crate::borrow::reference_borrow_access_kind(program, type_reference)
}

pub(super) fn active_loan_detail(
    state_flow: &FlowStateFact,
    facts: &CheckFacts,
    loan: arena::Handle<checked_trees::BorrowLoanFact>,
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
                    checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                    checked_trees::FlowInvalidationSource::Statement {
                        statement_index: weakening_statement,
                    },
                ) if weakening_statement > statement_index => Some(format!(
                    "borrowed at statement {}; its last use is at statement {}",
                    loan.statement_index, loan.last_use_statement_index
                )),
                (
                    checked_trees::FlowBorrowWeakeningReason::StateExit,
                    checked_trees::FlowInvalidationSource::Statement { .. },
                ) if loan.last_use_statement_index > statement_index => Some(format!(
                    "borrowed at statement {}; its last use is at statement {} and it is released at state exit",
                    loan.statement_index, loan.last_use_statement_index
                )),
                (
                    checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                    checked_trees::FlowInvalidationSource::Statement {
                        statement_index: weakening_statement,
                    },
                ) if weakening_statement > statement_index => Some(format!(
                    "borrowed at statement {}; it is reassigned at statement {}",
                    loan.statement_index, weakening_statement
                )),
                (
                    checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                    checked_trees::FlowInvalidationSource::Statement { .. },
                )
                | (
                    checked_trees::FlowBorrowWeakeningReason::StateExit,
                    checked_trees::FlowInvalidationSource::Statement { .. },
                )
                | (
                    checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                    checked_trees::FlowInvalidationSource::Statement { .. },
                )
                | (
                    checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                    checked_trees::FlowInvalidationSource::Call { .. },
                )
                | (
                    checked_trees::FlowBorrowWeakeningReason::StateExit,
                    checked_trees::FlowInvalidationSource::Call { .. },
                )
                | (
                    checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                    checked_trees::FlowInvalidationSource::Call { .. },
                ) => None,
            }
        })
}

pub(super) fn canonical_place_label(
    program: &typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
) -> String {
    crate::labels::canonical_place_label_from_parts(program, place.root, &place.segments)
}
