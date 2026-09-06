//! Local reference operands rejoin their retained storage and exact lineage.

use super::*;
use arena::{Handle, HandleSpan};
use checked_trees::{BorrowFacts, BorrowLoanFact, BorrowLoanLineage, StateBorrowFact};
use typed_trees::statement::StatementNode;

pub(super) struct ResolvedAlias {
    pub(super) place: CapturedPlace,
    pub(super) lineage: Vec<Handle<BorrowLoanFact>>,
}

pub(super) fn resolve(
    program: &TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<ResolvedAlias> {
    let state = crate::find_state(program, state_flow.state_symbol)?;
    let reference_local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            matches!(statement, StatementNode::LocalData(local)
                if local.symbol == place.root_symbol
                    && matches!(program.type_reference_table.type_reference(local.type_reference),
                        TypeReferenceNode::Reference { .. }))
        });
    if !reference_local {
        return Some(ResolvedAlias {
            place,
            lineage: Vec::new(),
        });
    }

    let borrow = &facts.borrow;
    let state_borrow = borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == state_flow.machine_symbol
            && state.state_symbol == state_flow.state_symbol)
            .then_some(state)
    })?;
    let mut candidates = facts
        .flow
        .borrow_loan_constraints(entry_constraints)
        .filter(|handle| {
            borrow.state_owns_loan(state_borrow, *handle)
                && borrow.loans.get(*handle).owner_symbol == place.root_symbol
        });
    let handle = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    let loan = borrow.loans.get(handle);
    if !loan.owner_path.is_empty() || !loan.root_symbol.is_valid() {
        return None;
    }
    let lineage = retained_lineage(borrow, state_borrow, handle)?;
    let mut segments = borrow.loan_segments(loan).to_vec();
    // Formation already rebased this loan to original storage. Append only
    // the current operand's suffix, retaining dynamic selectors as selectors.
    segments.extend(place.segments);
    Some(ResolvedAlias {
        place: CapturedPlace {
            root_symbol: loan.root_symbol,
            segments,
        },
        lineage,
    })
}

fn retained_lineage(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    mut handle: Handle<BorrowLoanFact>,
) -> Option<Vec<Handle<BorrowLoanFact>>> {
    // Checked resource replay precedes call exclusion. Follow only its exact
    // state-owned parent handles; shared storage or matching names do not
    // establish ancestry, and an unretained transfer grants no exemption.
    let mut lineage = Vec::new();
    loop {
        if !borrow.state_owns_loan(state, handle) || lineage.contains(&handle) {
            return None;
        }
        lineage.push(handle);
        let loan = borrow.loans.get(handle);
        match loan.lineage {
            BorrowLoanLineage::DirectRoot => return Some(lineage),
            BorrowLoanLineage::Reborrow { parent_loan } => {
                if !borrow.state_owns_loan(state, parent_loan) {
                    return None;
                }
                let parent = borrow.loans.get(parent_loan);
                if loan.source_owner_symbol != parent.owner_symbol
                    || parent.kind.direct_reborrow_effect(&loan.kind).is_none()
                {
                    return None;
                }
                handle = parent_loan;
            }
            BorrowLoanLineage::UnretainedDerived => return None,
        }
    }
}
