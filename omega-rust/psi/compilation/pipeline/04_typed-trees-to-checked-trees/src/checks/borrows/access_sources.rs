//! Resolve storage accesses through retained aliases and checked source returns.
//!
//! A lifetime annotation names the complete input obligation, while checked
//! exits refine its descriptor sources. Use the loan producer's classified
//! source relation, then rejoin local results to live loans at formation. Selection
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

mod descriptor_loads;

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
    resolve_captured_with_loans(
        program,
        facts,
        state,
        statement_index,
        facts.flow.borrow_loan_constraints(entry_constraints),
        place,
    )
}

pub(in crate::checks::borrows) fn resolve_captured_with_loans(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    loans: impl Iterator<Item = arena::Handle<checked_trees::BorrowLoanFact>>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedAlias>> {
    let candidates =
        aliases::resolve_places_with_loans(program, facts, state, loans, place.clone())?;
    retain_captured_lineage(program, facts, state, statement_index, place, candidates)
}

/// The argument already evaluated a reference value, including a descriptor
/// stored inside an owned carrier. Its completion constraints select that
/// descriptor's supplying loans independently of later carrier replacement.
pub(in crate::checks::borrows) fn resolve_descriptor_all(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    constraints: HandleSpan<FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedAlias>> {
    descriptor_loads::resolve(program, facts, state, statement_index, constraints, place)
}

fn retain_captured_lineage(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    place: CapturedPlace,
    candidates: Vec<aliases::ResolvedPlace>,
) -> Option<Vec<ResolvedAlias>> {
    let root = place.root;
    let (_, borrow_state) = facts
        .borrow
        .first_state_borrow(state.machine_symbol, state.state_symbol)?;
    // A current cut may contain different generations on mutually exclusive
    // paths. They are alternatives of one exact observed descriptor slot;
    // earlier saved operands retain their distinct, earlier cut. Authenticate
    // every generation before retaining this correspondence. Wildcard slots
    // can coexist and cannot be grouped by their possible storage region.
    for candidate in &candidates {
        if candidate.local_loan.is_valid() {
            let loan = facts.borrow.loans.get(candidate.local_loan);
            if loan.established_by_call.is_valid()
                && !completed_call_established_loan(facts, state, candidate.local_loan)
            {
                return None;
            }
        }
    }
    let descriptor_alternatives = candidates
        .iter()
        .map(|candidate| {
            if !candidate.local_loan.is_valid() {
                return Vec::new();
            }
            let loan = facts.borrow.loans.get(candidate.local_loan);
            let path = facts.borrow.loan_owner_path(loan);
            if path.contains(&checked_trees::BorrowLoanOwnerSegment::DynamicIndex) {
                return vec![candidate.local_loan];
            }
            candidates
                .iter()
                .filter_map(|other| {
                    if !other.local_loan.is_valid() {
                        return None;
                    }
                    let other_loan = facts.borrow.loans.get(other.local_loan);
                    (loan.storage_owner == other_loan.storage_owner
                        && facts.borrow.loan_owner_path(other_loan) == path)
                        .then_some(other.local_loan)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut resolved = Vec::with_capacity(candidates.len());
    for (candidate, alternatives) in candidates.into_iter().zip(descriptor_alternatives) {
        if !candidate.local_loan.is_valid() {
            resolved.push(ResolvedAlias {
                place: candidate.place,
                lineage: Vec::new(),
            });
            continue;
        }
        let loan = facts.borrow.loans.get(candidate.local_loan);
        // The complete candidate roster was authenticated above, before
        // its current-descriptor alternative correspondence was retained.
        let available_at_call = loan.established_by_call.is_valid();
        if let Some(mut lineage) =
            aliases::retained_lineage(&facts.borrow, borrow_state, candidate.local_loan)
        {
            for alternative in alternatives {
                if !lineage.contains(&alternative) {
                    lineage.push(alternative);
                }
            }
            resolved.push(ResolvedAlias {
                place: candidate.place,
                lineage,
            });
            continue;
        }
        if loan.lineage != BorrowLoanLineage::UnretainedDerived
            || (loan.statement_index >= statement_index && !available_at_call)
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
        let mut lineage = alternatives;
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

/// A statement's future loans cannot supply authority. An explicitly linked
/// invocation generation instead has its own actual post-call constraint cut;
/// the caller above already required that loan in the descriptor's capture.
fn completed_call_established_loan(
    facts: &CheckFacts,
    state: &FlowStateFact,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> bool {
    let loan = facts.borrow.loans.get(handle);
    if !facts.borrow.calls.is_valid(loan.established_by_call) {
        return false;
    }
    let call = facts.borrow.calls.get(loan.established_by_call);
    facts.borrow.call_at(
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) == Some(loan.established_by_call)
        && call
            .completed_reference_loans
            .handles()
            .any(|candidate| candidate == handle)
        && facts
            .flow
            .control
            .calls
            .span_or_empty(state.calls)
            .iter()
            .any(|actual| {
                actual.statement_index == loan.statement_index
                    && actual.call_ordinal == call.call_ordinal
                    && actual.target_symbol == call.target_symbol
                    && actual.authored_expression == call.authored_expression
                    && facts
                        .flow
                        .borrow_loan_constraints(actual.exit_constraints)
                        .any(|candidate| candidate == handle)
            })
}
