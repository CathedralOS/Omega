//! Lifetimes: body-level escape check for returned views.
//!
//! The elision check (stage 1/2) verifies which input a returned view borrows
//! at the SIGNATURE level. This check is the BODY-level companion: it rejects a
//! machine that returns a view of one of its own locals (a classic dangling
//! borrow), e.g.
//!
//! ```text
//! machine leak(seed: &Cell) -> &Cell {
//!     let local: Cell = Cell { value: 9 };
//!     transition { _ -> &local }   // `local` dies at the return — rejected
//! }
//! ```
//!
//! The rule is precise, not merely conservative: returning a borrow whose place
//! ROOTS in a machine-body local that itself holds NO loan is always a dangling
//! reference (the local's storage is gone at the return). A local that DOES hold
//! a loan (`let cells: &mut [Cell] = a.cells.as_mut_slice()`) borrows further
//! back — through a parameter or `self` — so returning `&mut cells[i]` is fine;
//! the loan fact for that local is the signal. Parameters, `self`, and
//! `self.field` never appear in the local set, so they are never rejected.

use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::statement::{StatementNode, TransitionTargetNode};

use crate::borrow::accesses::borrow_access_place;
use crate::borrow::view_link::returns_borrow;
use psi_checked_trees::CheckFacts;

pub(super) fn check_view_return_escape(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if !returns_borrow(program, state.return_type) {
                continue;
            }

            let statements = program.statement_table.statements(state.statement_nodes);

            // Locals declared in this state. A returned borrow rooted in a local
            // is sound only if that local holds a loan reaching OUTSIDE the body
            // (a parameter / `self` / a field thereof); a local with no loan, or
            // a loan still rooted in another body-local, is a dangling borrow.
            let locals: Vec<SymbolHandle> = statements
                .iter()
                .filter_map(|statement| match statement {
                    StatementNode::LocalData(local_data) => Some(local_data.symbol),
                    _ => None,
                })
                .collect();
            let loans = state_loans(facts, machine.symbol, state.symbol);

            for (statement_index, statement) in statements.iter().enumerate() {
                let Some(return_expression) = return_expression(program, statement) else {
                    continue;
                };
                let Some(place) = borrow_access_place(
                    program,
                    state.symbol,
                    statement_index,
                    return_expression,
                    machine.symbol,
                ) else {
                    continue;
                };

                let root = place.root_symbol;
                if !locals.contains(&root) {
                    // Rooted in a parameter, `self`, or a field thereof — outlives the call.
                    continue;
                }
                // A body-local: sound only if it holds at least one loan and
                // every carried loan reaches outside the body. Accepting when
                // merely one field reached an input would let a sibling field
                // retain a dangling local borrow.
                let owner_loans: Vec<SymbolHandle> = loans
                    .iter()
                    .filter_map(|(owner, loan_root)| (*owner == root).then_some(*loan_root))
                    .collect();
                let escapes_through_loans = !owner_loans.is_empty()
                    && owner_loans
                        .iter()
                        .all(|loan_root| !locals.contains(loan_root));
                if escapes_through_loans {
                    continue;
                }

                let local_name = local_name(statements, root);
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` returns a view borrowing the local `{}`, which does not outlive \
                     the call; return a borrow of an input parameter or `self` instead",
                    state_subject(machine, state),
                    local_name,
                )));
            }
        }
    }
}

/// The borrow expression a state returns at a given statement: a terminal
/// `Expression` or a transition `Value` target. Other statements/targets do not
/// return a value.
fn return_expression(
    program: &psi_typed_trees::TypedTrees,
    statement: &StatementNode,
) -> Option<psi_typed_trees::expression::ExpressionHandle> {
    match statement {
        StatementNode::Expression(expression) => Some(*expression),
        StatementNode::Transition(transition) => {
            match program.statement_table.transition_target(transition.target) {
                TransitionTargetNode::Value(expression) => Some(*expression),
                _ => None,
            }
        }
        _ => None,
    }
}

/// `(owner_symbol, ultimate root_symbol)` for every loan tracked in the state.
/// The loan's `root_symbol` is already rebased to its ultimate source (a param
/// for `let cells = a.cells.as_mut_slice()`, or a body-local for `&owned_local`).
fn state_loans(
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Vec<(SymbolHandle, SymbolHandle)> {
    facts
        .borrow
        .states
        .iter()
        .find(|(_, state)| {
            state.machine_symbol == machine_symbol && state.state_symbol == state_symbol
        })
        .map(|(_, state)| {
            facts
                .borrow
                .loans
                .span_or_empty(state.loans)
                .iter()
                .map(|loan| (loan.owner_symbol, loan.root_symbol))
                .collect()
        })
        .unwrap_or_default()
}

fn local_name(statements: &[StatementNode], symbol: SymbolHandle) -> String {
    statements
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local_data) if local_data.symbol == symbol => {
                Some(local_data.name.as_str().to_owned())
            }
            _ => None,
        })
        .unwrap_or_else(|| "<local>".to_owned())
}

fn state_subject<'a>(
    machine: &'a psi_typed_trees::machine::Machine,
    state: &'a psi_typed_trees::state::State,
) -> &'a str {
    if state.name.as_str() == "entry" || machine.name.as_str().ends_with(state.name.as_str()) {
        machine.name.as_str()
    } else {
        state.name.as_str()
    }
}
