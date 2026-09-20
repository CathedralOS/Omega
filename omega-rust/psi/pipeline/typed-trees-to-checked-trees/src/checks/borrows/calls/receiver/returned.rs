//! Resolve an exclusive operation's receiver from checked source returns.
//!
//! A lifetime annotation constrains a result but does not prove its storage or
//! parent authority. Reconstruct every callee exit's actual substitution, then
//! rejoin local results to their live loan and formation point. This does not
//! relabel derived loans as direct reborrows or change native carrier admission.

use super::aliases::{self, ResolvedAlias};
use checked_trees::{
    BorrowLoanLineage, CapturedPlace, CheckFacts, FlowStateFact, FlowStatementFact,
};
use facts::PlaceRoot;
use language_semantics::ReferenceAccess;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

pub(super) fn resolve(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: &FlowStatementFact,
    expression: ExpressionHandle,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<ResolvedAlias> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) if borrow.access == ReferenceAccess::Mutable => {
            return resolve(program, facts, state, statement, borrow.target, frames);
        }
        ExpressionNode::Call(call) => {
            let mut resolved: Option<ResolvedAlias> = None;
            for source in crate::flow::call_result_sources(program, call, frames)? {
                let mut actual = resolve(program, facts, state, statement, source.actual, frames)?;
                actual.place.segments.extend(source.segments);
                if let Some(previous) = &resolved {
                    if previous.place != actual.place || previous.lineage != actual.lineage {
                        return None;
                    }
                } else {
                    resolved = Some(actual);
                }
            }
            return resolved;
        }
        _ => {}
    }
    let source = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement.statement_index,
        expression,
    )?;
    let PlaceRoot::Symbol(root_symbol) = source.root else {
        return None;
    };
    let place = CapturedPlace {
        root_symbol,
        segments: source.segments,
    };
    if let Some(receiver) = aliases::resolve_receiver(
        program,
        facts,
        state,
        statement.entry_constraints,
        place.clone(),
    ) {
        return Some(receiver);
    }

    let resolved = aliases::resolve_place(
        program,
        facts,
        state,
        statement.entry_constraints,
        place.clone(),
    )?;
    if !resolved.local_loan.is_valid() {
        return None;
    }
    let loan = facts.borrow.loans.get(resolved.local_loan);
    if loan.lineage != BorrowLoanLineage::UnretainedDerived
        || loan.statement_index >= statement.statement_index
    {
        return None;
    }
    let source_state = crate::semantic_calls::find_state(program, state.state_symbol)?;
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(source_state.statement_nodes)
        .get(loan.statement_index)?
    else {
        return None;
    };
    if local.symbol != root_symbol
        || !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        )
    {
        return None;
    }
    let formation = facts
        .flow
        .control
        .statements
        .span_or_empty(state.statements)
        .iter()
        .find(|candidate| candidate.statement_index == loan.statement_index)?;
    let mut returned = resolve(
        program,
        facts,
        state,
        formation,
        local.initial_value,
        frames,
    )?;
    returned.place.segments.extend(place.segments);
    // The exact source-derived result must reproduce the independently retained
    // loan, not merely share the result type or an overlapping parent record.
    if returned.place != resolved.place {
        return None;
    }
    returned.lineage.push(resolved.local_loan);
    Some(returned)
}
