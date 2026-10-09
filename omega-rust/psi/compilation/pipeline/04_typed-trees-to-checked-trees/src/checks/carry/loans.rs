//! Preserve the invocation's current loan graph and its backing dependencies.
//!
//! Flow has already applied evaluation-order replacement and weakening. Read
//! this call's entry constraints, never reconstruct an active roster from the
//! historical loans' last-use indices. Parent and supplier occurrences stay
//! distinct, including dependencies whose descriptor is no longer current.
//! Captured operand referents retain backing needed by the incomplete callee,
//! including constructed children with no named caller descriptor. Both routes
//! use the same owned-storage coordinate join before closing backing liveness.
//! This graph names logical demands; it establishes no StackLease or permission
//! to park. Incoming and unresolved actuals still require a per-value join.

use arena::{Arena, Handle};
use checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanLineage, FlowCallFact, FlowFacts, StateBorrowFact,
    SuspensionCrossingBorrowFrontier, SuspensionCrossingLoanBacking, SuspensionCrossingLoanFact,
};
use diagnostics::Diagnostic;

use super::StateCarryValue;

pub(super) fn retain_call_frontier(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    state: &StateBorrowFact,
    call: &FlowCallFact,
    source_values: &[StateCarryValue<'_>],
    retained: &mut Arena<SuspensionCrossingLoanFact>,
) -> Result<SuspensionCrossingBorrowFrontier, Diagnostic> {
    let mut current = Vec::new();
    for loan in flow.borrow_loan_constraints(call.entry_constraints) {
        append_dependency(borrow, state, loan, &mut current)?;
    }
    let mut closure = current.clone();
    let mut loan_cursor = 0;
    while let Some(handle) = closure.get(loan_cursor).copied() {
        loan_cursor += 1;
        let loan = borrow.loans.get(handle);
        if let BorrowLoanLineage::Reborrow { parent_loan } = loan.lineage {
            append_dependency(borrow, state, parent_loan, &mut closure)?;
        }
        if loan.supplying_loan.is_valid() {
            append_dependency(borrow, state, loan.supplying_loan, &mut closure)?;
        }
    }

    let loans = retained.insert_many(closure.into_iter().map(|handle| {
        let loan = borrow.loans.get(handle);
        let backing = if loan.incoming_reference.is_valid() {
            SuspensionCrossingLoanBacking::IncomingReferent {
                identity: loan.incoming_reference,
            }
        } else {
            owned_backing_origin(
                program,
                source_values,
                loan.root_symbol,
                call.statement_index,
            )
            .map_or(SuspensionCrossingLoanBacking::Unresolved, |origin| {
                SuspensionCrossingLoanBacking::StateValue { origin }
            })
        };
        SuspensionCrossingLoanFact {
            loan: handle,
            active_at_call: current.contains(&handle),
            retained_for_crossing: false,
            backing,
        }
    }));
    Ok(SuspensionCrossingBorrowFrontier {
        loans,
        operands: call.operands,
    })
}

/// A root view denotes caller-supplied storage, never its descriptor slot.
/// The entry tracker models contained descriptors, not the root borrowed
/// parameter permission; absent that captured identity, backing stays unknown.
fn owned_backing_origin(
    program: &dyn ::validation::CheckingInput,
    source_values: &[StateCarryValue<'_>],
    root: symbols::SymbolHandle,
    statement_index: usize,
) -> Option<checked_trees::SuspensionCrossingValueOrigin> {
    source_values
        .iter()
        .find(|value| value.symbol == root && value.available_before(statement_index))
        .filter(|value| {
            let reference = validation::peel_declared_constraints(program, value.type_reference);
            reference.is_valid()
                && validation::reference_shell(program, reference).is_none()
                && !matches!(
                    program.type_reference_table.type_reference(reference),
                    typed_trees::types::TypeReferenceNode::Slice { .. }
                )
        })
        .map(|value| value.origin)
}

/// The callee may still be using its completed actual while the caller has no
/// later use. Traverse that retained value tree, not argument syntax or the
/// current descriptor roster: a later operand may already have replaced the
/// descriptor which supplied an earlier actual. Empty referents remain unknown
/// or detached; this operation establishes no borrow-free or parking permission.
pub(super) fn retain_operand_backing(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    frontier: SuspensionCrossingBorrowFrontier,
    source_values: &[StateCarryValue<'_>],
    statement_index: usize,
    live_symbols: &mut Vec<symbols::SymbolHandle>,
) -> Result<(), Diagnostic> {
    let missing_capture =
        || Diagnostic::error("suspension argument has no retained operand capture".to_owned());
    let mut pending = flow
        .control
        .operands
        .span(frontier.operands)
        .ok_or_else(missing_capture)?
        .iter()
        .collect::<Vec<_>>();
    // Flow appends each constructed child's capture after evaluating its own
    // children. These producer-owned edges form a tree, not a graph needing a
    // repeated linear membership search for every array or record element.
    while let Some(operand) = pending.pop() {
        for referent in flow
            .control
            .operand_referents
            .span(operand.referents)
            .ok_or_else(missing_capture)?
        {
            let Some(root) = referent.root.as_symbol() else {
                continue;
            };
            if owned_backing_origin(program, source_values, root, statement_index).is_some()
                && !live_symbols.contains(&root)
            {
                live_symbols.push(root);
            }
        }
        for child in flow
            .control
            .operand_children
            .span(operand.children)
            .ok_or_else(missing_capture)?
        {
            if !flow.control.operands.is_valid(child.value) {
                return Err(missing_capture());
            }
            pending.push(flow.control.operands.get(child.value));
        }
    }
    Ok(())
}

fn append_dependency(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    loan: Handle<BorrowLoanFact>,
    closure: &mut Vec<Handle<BorrowLoanFact>>,
) -> Result<(), Diagnostic> {
    if !borrow.state_owns_loan(state, loan) {
        return Err(Diagnostic::error(
            "suspension loan dependency has no retained state-owned occurrence".to_owned(),
        ));
    }
    if !closure.contains(&loan) {
        closure.push(loan);
    }
    Ok(())
}

/// A live descriptor keeps its actual backing live even when later source
/// expressions mention only the descriptor. Follow exact retained dependency
/// edges, not all historical loans sharing that descriptor's symbol. Incoming
/// referents have no caller-frame coordinate and never retain the descriptor
/// slot as a substitute for their backing.
pub(super) fn retain_backing_values(
    borrow: &BorrowFacts,
    frontier: &mut [SuspensionCrossingLoanFact],
    live_symbols: &mut Vec<symbols::SymbolHandle>,
) {
    let mut retained_loans = Vec::new();
    let mut value_cursor = 0;
    let mut loan_cursor = 0;
    // Backing can itself contain view leaves. Newly retained storage enters
    // the same value queue so indirect carrier chains reach a fixed point.
    while value_cursor < live_symbols.len() || loan_cursor < retained_loans.len() {
        if let Some(symbol) = live_symbols.get(value_cursor).copied() {
            value_cursor += 1;
            for entry in frontier.iter().filter(|entry| {
                entry.active_at_call && borrow.loans.get(entry.loan).owner_symbol == symbol
            }) {
                if !retained_loans.contains(&entry.loan) {
                    retained_loans.push(entry.loan);
                }
            }
            continue;
        }
        let handle = retained_loans[loan_cursor];
        loan_cursor += 1;
        let loan = borrow.loans.get(handle);
        for dependency in [
            match loan.lineage {
                BorrowLoanLineage::Reborrow { parent_loan } => parent_loan,
                BorrowLoanLineage::DirectRoot | BorrowLoanLineage::UnretainedDerived => {
                    Handle::invalid()
                }
            },
            loan.supplying_loan,
        ] {
            if dependency.is_valid() && !retained_loans.contains(&dependency) {
                retained_loans.push(dependency);
            }
        }
        let Some(entry) = frontier.iter_mut().find(|entry| entry.loan == handle) else {
            continue;
        };
        entry.retained_for_crossing = true;
        if let SuspensionCrossingLoanBacking::StateValue { origin } = entry.backing {
            let symbol = match origin {
                checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. }
                | checked_trees::SuspensionCrossingValueOrigin::Local { symbol, .. } => symbol,
                checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => continue,
            };
            if !live_symbols.contains(&symbol) {
                live_symbols.push(symbol);
            }
        }
    }
}
