//! Conditional whole-local transfers, keyed by existing source selection nodes.
//! Availability is conservative; residual custody remains in the receipts until
//! the actual death edge. A path constructing a fresh result discards, at the
//! join edge, the one source whose residual slot the result displaces; every
//! other complement stays in the receipt until the actual death edge.
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

pub(super) use availability::{apply_availability, record_statement};
pub(in crate::checks) use operands::reachable_arms;
use operands::*;
mod availability;
mod leaves;
mod operands;

/// Two canonical paths overlap when one reaches inside the other or both
/// name the same place; a dynamic index cannot be statically excluded from a
/// fixed element claim, so it conflicts on the element axis.
pub(super) fn place_paths_overlap(
    used: &[facts::PlaceSegment],
    consumed: &[facts::PlaceSegment],
) -> bool {
    used.iter().zip(consumed.iter()).all(|(used, consumed)| {
        used == consumed
            || (matches!(used, facts::PlaceSegment::Index { .. })
                && matches!(
                    consumed,
                    facts::PlaceSegment::FixedIndex { .. }
                        | facts::PlaceSegment::Index { .. }
                        | facts::PlaceSegment::FixedRange { .. }
                ))
    })
}

/// Whether an owned match source still owes loan-closure evidence: either a
/// syntactic borrow the loan ledger never recorded (an escape without a
/// trackable loan, for example persistent machine storage) or a recorded loan
/// on the source root still live at this statement's entry. The flow builder
/// drops a loan from the entry constraint set once its last use passes, so a
/// once-borrowed source whose loans all closed joins like any other owner; a
/// live loan keeps the explicit rejection instead of guessing at referent
/// custody across the conditional move.
fn source_needs_loan_closure(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    prior_statements: &[StatementNode],
    symbol: SymbolHandle,
) -> bool {
    let borrowed = prior_statements.iter().any(|statement| {
        statement_expressions(program, statement).iter().any(|expression| {
            matches!(program.expression_table.expression(*expression), ExpressionNode::Borrow(borrow)
                if expression_names(program, borrow.target, symbol))
        })
    });
    let recorded = facts
        .borrow
        .loans
        .iter()
        .any(|(_, loan)| loan.root_symbol == symbol);
    if borrowed && !recorded {
        return true;
    }
    if !recorded {
        return false;
    }
    let statement_fact = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, flow_state)| {
            (flow_state.machine_symbol == machine.symbol && flow_state.state_symbol == state.symbol)
                .then_some(flow_state)
        })
        .and_then(|flow_state| facts.flow.state_statement(flow_state, statement_index));
    match statement_fact {
        Some(statement_fact) => facts
            .flow
            .borrow_loan_constraints(statement_fact.entry_constraints)
            .any(|loan| facts.borrow.loans.get(loan).root_symbol == symbol),
        // Without the statement's entry constraint set, fall back to the
        // loan's recorded last use, the same boundary the expiry filter uses.
        None => facts.borrow.loans.iter().any(|(_, loan)| {
            loan.root_symbol == symbol && loan.last_use_statement_index >= statement_index
        }),
    }
}

fn unsupported() -> Diagnostic {
    Diagnostic::error(
        "owned match requires one existing whole plain-affine or linear-owned local per path and an immutable local or return destination; projections, calls and borrowed custody require additional ownership evidence",
    )
}
