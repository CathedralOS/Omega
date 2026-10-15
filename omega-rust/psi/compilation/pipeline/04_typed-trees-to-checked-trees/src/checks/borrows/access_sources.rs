//! Resolve storage accesses through retained aliases and checked source returns.
//!
//! A lifetime annotation names the complete input obligation, while checked
//! exits refine its storage projections. Use the loan producer's same frontier,
//! then rejoin local results to live loans at their formation point. Selection
//! keeps all captured backings; it never grants access to a private descriptor
//! slot or relabels derived loans as direct reborrows.

use super::aliases::{self, ResolvedAlias};
use arena::HandleSpan;
use checked_trees::{
    BorrowLoanLineage, CapturedPlace, CheckFacts, FlowConstraintRef, FlowStateFact,
    FlowStatementFact,
};
use language_semantics::ReferenceAccess;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(in crate::checks::borrows) fn resolve(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: &FlowStatementFact,
    expression: ExpressionHandle,
) -> Option<ResolvedAlias> {
    let mut sources = resolve_all(program, facts, state, statement, expression)?;
    (sources.len() == 1).then(|| sources.remove(0))
}

/// Resolve every possible backing with its independently established access
/// lineage. The complete roster is required for writes through a selection.
pub(in crate::checks::borrows) fn resolve_all(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: &FlowStatementFact,
    expression: ExpressionHandle,
) -> Option<Vec<ResolvedAlias>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) if borrow.access == ReferenceAccess::Mutable => {
            return resolve_all(program, facts, state, statement, borrow.target);
        }
        ExpressionNode::Call(call) => {
            let mut resolved = Vec::new();
            let arguments = program.expression_table.expression_handles(call.arguments);
            for (source, projection) in
                crate::borrow::call_reference_sources(program, state.state_symbol, call)?
            {
                // This query resolves a direct reference result, not an owned
                // carrier's descriptor fields. Those need their own retained
                // leaf correspondence before they can grant ancestor access.
                if !source.owner_path.is_empty() || !source.source_path.is_empty() {
                    return None;
                }
                let argument = *arguments.get(source.parameter_index)?;
                for mut actual in resolve_all(program, facts, state, statement, argument)? {
                    actual.place.segments.extend_from_slice(&projection);
                    if !resolved.iter().any(|previous: &ResolvedAlias| {
                        previous.place == actual.place && previous.lineage == actual.lineage
                    }) {
                        resolved.push(actual);
                    }
                }
            }
            return (!resolved.is_empty()).then_some(resolved);
        }
        _ => {}
    }
    let source = validation::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement.statement_index,
        expression,
    )?;
    let place = CapturedPlace {
        incoming_reference: Default::default(),
        root: source.root,
        segments: source.segments,
    };
    resolve_captured_all(
        program,
        facts,
        state,
        statement.statement_index,
        statement.entry_constraints,
        place,
    )
}

/// Resolve a place already captured at `statement_index`, such as a call
/// operand's recorded access, against the loans live at `entry_constraints`.
/// A root that is not a local reference resolves to itself without lineage.
pub(in crate::checks::borrows) fn resolve_captured_all(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    entry_constraints: HandleSpan<FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedAlias>> {
    let root = place.root;
    let (_, borrow_state) =
        crate::flow::borrow_state_fact(&facts.borrow, state.machine_symbol, state.state_symbol)?;
    let candidates =
        aliases::resolve_places(program, facts, state, entry_constraints, place.clone())?;
    let mut resolved = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if !candidate.local_loan.is_valid() {
            resolved.push(ResolvedAlias {
                place: candidate.place,
                lineage: Vec::new(),
            });
            continue;
        }
        if let Some(lineage) =
            aliases::retained_lineage(&facts.borrow, borrow_state, candidate.local_loan)
        {
            resolved.push(ResolvedAlias {
                place: candidate.place,
                lineage,
            });
            continue;
        }
        let loan = facts.borrow.loans.get(candidate.local_loan);
        if loan.lineage != BorrowLoanLineage::UnretainedDerived
            || loan.statement_index >= statement_index
        {
            return None;
        }
        // Captured storage and the permission itself are already established
        // by the loan producer, including opaque calls and recasts. An ancestor
        // exemption is a separate judgment: it needs exact source substitution
        // at formation and cannot follow from overlapping captured places.
        let supplying = (|| {
            let source_state = program.state_by_symbol(state.state_symbol)?;
            let local = program
                .statement_table
                .statement_at(source_state, loan.statement_index)?
                .as_local_data()?;
            if facts::PlaceRoot::Symbol(local.symbol) != root {
                return None;
            }
            let formation = facts
                .flow
                .control
                .statements
                .span_or_empty(state.statements)
                .iter()
                .find(|previous| previous.statement_index == loan.statement_index)?;
            resolve_all(program, facts, state, formation, local.initial_value)
        })();
        let mut lineage = vec![candidate.local_loan];
        if let Some(supplying) = supplying {
            for mut source in supplying {
                source.place.segments.extend_from_slice(&place.segments);
                if source.place == candidate.place {
                    for ancestor in source.lineage {
                        if !lineage.contains(&ancestor) {
                            lineage.push(ancestor);
                        }
                    }
                }
            }
        }
        resolved.push(ResolvedAlias {
            place: candidate.place,
            lineage,
        });
    }
    Some(resolved)
}
