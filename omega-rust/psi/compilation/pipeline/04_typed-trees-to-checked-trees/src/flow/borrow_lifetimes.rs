//! Borrow-loan expiry in the active constraints. `filter_expired_borrow_loans`
//! drops loans past their last use, and `filter_reassigned_borrow_loans` drops
//! loans carried by the place an assignment overwrites. Each dropped loan
//! records a `FlowBorrowWeakeningFact`.

use validation::CanonicalPlace;

use crate::flow::reference_spans;
use checked_trees::{
    BorrowFacts, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason, FlowConstraintKind,
    FlowConstraintRef, FlowInvalidationSource,
};

pub(crate) fn filter_expired_borrow_loans(
    borrow_weakenings: &mut arena::Arena<FlowBorrowWeakeningFact>,
    constraint_refs: &mut arena::Arena<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
    borrow: &BorrowFacts,
    statement_index: usize,
    reason: FlowBorrowWeakeningReason,
) -> arena::HandleSpan<FlowConstraintRef> {
    reference_spans::filter_constraint_refs(constraint_refs, source, |constraint_ref| {
        match constraint_ref.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                let loan_fact = borrow.loans.get(loan);
                let keep = loan_fact.last_use_statement_index >= statement_index;
                if !keep {
                    // The weakening boundary is a recorded coordinate — one
                    // statement past the loan's recorded last use — never the
                    // position this traversal happened to filter at. In the
                    // ordinary per-statement walk the two coincide (the filter
                    // fires at `last_use + 1`, and the state-exit pass reaches
                    // only loans whose boundary is the statement count), but
                    // stamping the recorded boundary keeps the emitted fact a
                    // function of the loan row alone, so a replayed derivation
                    // — `receiver_aliases` re-derives exactly
                    // `last_use_statement_index + 1` — reaches the same
                    // evidence under any filtering cadence.
                    borrow_weakenings.append(FlowBorrowWeakeningFact {
                        source: FlowInvalidationSource::Statement {
                            statement_index: loan_fact.last_use_statement_index + 1,
                        },
                        loan,
                        reason,
                    });
                }
                keep
            }
            FlowConstraintKind::Unknown | checked_trees::borrow_context_quint_pattern!(..) => true,
        }
    })
}

/// Both flow construction and independent replay prepare their resolver before
/// traversing statements. `None` is that immutable preparation's failed result,
/// not a request to reconstruct the resolver for each loan-lifetime query.
pub(crate) fn filter_reassigned_borrow_loans(
    borrow_weakenings: &mut arena::Arena<FlowBorrowWeakeningFact>,
    constraint_refs: &mut arena::Arena<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
    borrow: &BorrowFacts,
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    reassigned_place: Option<CanonicalPlace>,
    statement_index: usize,
    statement: &typed_trees::statement::StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> arena::HandleSpan<FlowConstraintRef> {
    // Completed calls replace slots regardless of their result type or where
    // the invocation appears. Then apply any enclosing assignment's store.
    let source = if let Some(effects) =
        call_frames.and_then(|frames| frames.stored_reference_effects(machine, statement))
    {
        reference_spans::filter_constraint_refs(constraint_refs, source, |constraint| {
            let FlowConstraintKind::BorrowLoan { loan } = constraint.kind else {
                return true;
            };
            let active = borrow.loans.get(loan);
            let replaced = active.statement_index < statement_index
                && effects.iter().any(|effect| {
                    !effect.retains_incoming
                        && active.storage_owner
                            == facts::PlaceRoot::Symbol(effect.destination.local_symbol)
                        && crate::borrow::place_path_covers_owner(
                            program,
                            &effect.destination.local_segments,
                            borrow.loan_owner_path(active),
                        )
                });
            if replaced {
                borrow_weakenings.append(FlowBorrowWeakeningFact {
                    source: FlowInvalidationSource::Statement { statement_index },
                    loan,
                    reason: FlowBorrowWeakeningReason::LocalReassigned,
                });
            }
            !replaced
        })
    } else {
        source
    };
    let Some(place) = reassigned_place else {
        return source;
    };
    let (reassigned_symbol, reassigned_segments) = match place.root {
        facts::PlaceRoot::Symbol(symbol) => (symbol, place.segments),
        _ => return source,
    };

    // Reference carriers have the same lifetime rule at a root or a projection:
    // writing through one preserves its captured loan, while replacing it ends
    // the old occurrence. Classify the typed target before comparing owner paths.
    let mut writes_referent = false;
    if let Some(assignment) = statement.as_assignment()
        && let Some((machine, state)) =
            validation::find_state(program, program.symbols.get(reassigned_symbol).parent)
        && let Some(mut reference) =
            validation::declared_place_type_raw(program, machine, Some(state), assignment.target)
    {
        while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        if matches!(
            program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        ) {
            let Some(replacement) = call_frames.and_then(|frames| {
                frames.assignment_replaces_reference_binding(machine, statement)
            }) else {
                return source;
            };
            writes_referent = !replacement;
        }
    }

    let target = checked_trees::CapturedPlace {
        incoming_reference: Default::default(),
        root: facts::PlaceRoot::Symbol(reassigned_symbol),
        segments: reassigned_segments,
    };
    let destinations =
        validation::find_state(program, program.symbols.get(reassigned_symbol).parent)
            .map(|(_, state)| {
                crate::borrow::store_destination_places(
                    program,
                    state,
                    target.clone(),
                    writes_referent,
                    constraint_refs
                        .span_or_empty(source)
                        .iter()
                        .filter_map(|constraint| match constraint.kind {
                            FlowConstraintKind::BorrowLoan { loan } => Some(borrow.loans.get(loan)),
                            _ => None,
                        })
                        .filter(|loan| loan.statement_index < statement_index)
                        .map(|loan| {
                            (
                                loan.storage_owner,
                                borrow.loan_owner_path(loan),
                                loan.incoming_reference,
                                loan.root,
                                borrow.loan_segments(loan),
                            )
                        }),
                )
            })
            .unwrap_or_else(|| vec![target.clone()]);
    let [destination] = destinations.as_slice() else {
        // A may-alias set cannot prove which old slot was replaced.
        return source;
    };

    reference_spans::filter_constraint_refs(constraint_refs, source, |constraint_ref| {
        match constraint_ref.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                let active = borrow.loans.get(loan);
                // Loans established by this assignment describe the replacement
                // value and must survive it. Older loans carried by the
                // overwritten whole/place are invalidated precisely.
                let keep = active.storage_owner != destination.root
                    || (writes_referent
                        && destination == &target
                        && borrow.loan_owner_path(active).len() == target.segments.len())
                    || active.statement_index == statement_index
                    || !crate::borrow::place_path_covers_owner(
                        program,
                        &destination.segments,
                        borrow.loan_owner_path(active),
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
            FlowConstraintKind::Unknown | checked_trees::borrow_context_quint_pattern!(..) => true,
        }
    })
}
