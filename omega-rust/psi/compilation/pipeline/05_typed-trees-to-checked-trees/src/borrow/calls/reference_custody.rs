//! A call publishes its returned references before the next operand uses them.
//! A promised opaque result owns the completion's permission and retains every
//! input lifetime. Stored replacements keep their exact earlier suppliers.
//! The checking evaluator supplies the ordered entry/call/join graph; named
//! bindings retain its completed values rather than creating another grant.

use super::super::state::{BorrowFactArenas, append_carrier_loans};
use super::super::tracker::StateLoanTracker;
use checked_trees::{BorrowCallFact, BorrowLoanFact};

/// Retain unjoined invocation generations alongside the statement's joined
/// successor roster. Flow installs only the exact call occurrence that ran;
/// these historical rows never replace another arm's current source inventory.
pub(in crate::borrow) fn append_reference_custody(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    frames: Option<&validation::CallFrameResolver<'_>>,
    arenas: &mut BorrowFactArenas<'_>,
    calls: arena::HandleSpan<BorrowCallFact>,
    loans: &mut arena::HandleSpan<BorrowLoanFact>,
    history: &mut Vec<StateLoanTracker>,
    statement_entry: &[StateLoanTracker],
) -> Result<Option<Vec<StateLoanTracker>>, diagnostics::Diagnostic> {
    let Some(schedule) = frames
        .and_then(|frames| frames.stored_reference_call_contexts(machine, state, statement_index))
    else {
        return Ok(None);
    };
    let schedule_len = schedule.len();
    let mut contexts: Vec<Option<Vec<StateLoanTracker>>> = Vec::new();
    let mut published_calls = Vec::new();
    for (position, context) in schedule.into_iter().enumerate() {
        use validation::StoredReferenceCallContext;
        let current = match context {
            StoredReferenceCallContext::StatementCompletion { entry } => {
                if position + 1 != schedule_len {
                    return Err(diagnostics::Diagnostic::error(
                        "normal reference-slot completion precedes another context",
                    ));
                }
                return contexts.get_mut(entry).map(Option::take).ok_or_else(|| {
                    diagnostics::Diagnostic::error(
                        "normal reference-slot completion names an unavailable predecessor",
                    )
                });
            }
            StoredReferenceCallContext::Entry if contexts.is_empty() => {
                Some(statement_entry.to_vec())
            }
            StoredReferenceCallContext::Entry => {
                return Err(diagnostics::Diagnostic::error(
                    "normal reference-slot context repeats its entry",
                ));
            }
            StoredReferenceCallContext::Join { alternatives } => {
                if alternatives.is_empty() {
                    contexts.push(None);
                    continue;
                }
                let mut joined = Some(Vec::new());
                for alternative in alternatives {
                    let Some(roster) = contexts.get(alternative) else {
                        return Err(diagnostics::Diagnostic::error(
                            "normal reference-slot join names an unavailable predecessor context",
                        ));
                    };
                    match (&mut joined, roster) {
                        (Some(joined), Some(roster)) => {
                            for tracker in roster {
                                if !joined.contains(tracker) {
                                    joined.push(tracker.clone());
                                }
                            }
                        }
                        _ => joined = None,
                    }
                }
                joined
            }
            StoredReferenceCallContext::Invocation {
                entry,
                expression,
                effects,
            } => {
                let Some(entry) = contexts.get(entry) else {
                    return Err(diagnostics::Diagnostic::error(
                        "normal reference-slot invocation names an unavailable entry context",
                    ));
                };
                let (Some(incoming), Some(effects)) = (entry, effects) else {
                    contexts.push(None);
                    continue;
                };
                let mut matching = calls.handles().filter(|handle| {
                    let call = arenas.calls.get(*handle);
                    call.statement_index == statement_index
                        && call.authored_expression == expression
                });
                let Some(call) = matching.next().filter(|_| matching.next().is_none()) else {
                    return Err(diagnostics::Diagnostic::error(format!(
                        "machine `{}` state `{}` statement {statement_index}: normal reference-slot completion for {expression:?} has no unique checked invocation",
                        machine.name, state.name,
                    )));
                };
                if published_calls.contains(&call) {
                    return Err(diagnostics::Diagnostic::error(
                        "normal reference-slot invocation has more than one completion context",
                    ));
                }
                published_calls.push(call);
                arenas.calls.get_mut(call).input_reference_loans = Some(
                    arenas
                        .loan_suppliers
                        .insert_many(incoming.iter().map(|loan| loan.handle)),
                );
                let mut destinations = arena::HandleSpan::empty();
                for effect in &effects {
                    let segments = arenas
                        .access_segments
                        .insert_many(effect.destination.local_segments.iter().copied());
                    arenas.reference_slot_effects.append_to_span(
                        &mut destinations,
                        checked_trees::BorrowCallReferenceSlotEffect {
                            root: facts::PlaceRoot::Symbol(effect.destination.local_symbol),
                            segments,
                            owner_path: arenas.owner_segments.insert_many(
                                crate::borrow::owner_path_from_place_segments(
                                    program,
                                    &effect.destination.local_segments,
                                ),
                            ),
                            may_select: effect.may_select,
                        },
                    );
                }
                arenas.calls.get_mut(call).reference_slot_effects = destinations;
                let mut current = incoming
                    .iter()
                    .filter(|tracker| {
                        !effects.iter().any(|effect| {
                            !effect.may_select
                                && tracker.storage_owner
                                    == facts::PlaceRoot::Symbol(effect.destination.local_symbol)
                                && crate::borrow::place_path_covers_owner(
                                    program,
                                    &effect.destination.local_segments,
                                    &tracker.owner_path,
                                )
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let mut produced =
                    crate::borrow::loans::stored_reference_effect_loans(program, effects, incoming);
                produced.extend(
                    crate::borrow::loans::complete_invocation_results(
                        program,
                        machine,
                        state,
                        statement_index,
                        expression,
                        statement_entry,
                        incoming,
                        frames,
                    )
                    .map_err(|reason| {
                        let target = program.expression_table.call_node(expression)
                            .map_or("<unknown>", |call| call.target.as_str());
                        diagnostics::Diagnostic::error(format!(
                            "normal call `{target}` ({expression:?}) in machine `{}` state `{}` statement {statement_index}: {reason}",
                            machine.name, state.name,
                        ))
                    })?,
                );
                for loan in &mut produced {
                    loan.formation_expression = expression;
                }
                let published = append_carrier_loans(
                    arenas,
                    loans,
                    &mut current,
                    history,
                    produced,
                    statement_index,
                    call,
                );
                arenas.calls.get_mut(call).completed_reference_loans = published;
                Some(current)
            }
        };
        contexts.push(current);
    }
    Err(diagnostics::Diagnostic::error(
        "normal reference-slot schedule has no statement completion",
    ))
}

/// A definite transferred slot replaces exactly the descriptor occurrences
/// stored beneath that destination. May-select destinations keep unselected
/// occurrences. Backing address equality supplies no reaccess authority.
pub(crate) fn replaces_reference_loan(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    call: &BorrowCallFact,
    loan: arena::Handle<checked_trees::BorrowLoanFact>,
) -> bool {
    let previous = borrow.loans.get(loan);
    borrow
        .reference_slot_effects
        .span_or_empty(call.reference_slot_effects)
        .iter()
        .any(|effect| {
            !effect.may_select
                && effect.root == previous.storage_owner
                && crate::borrow::place_path_covers_owner(
                    program,
                    borrow.access_segments.span_or_empty(effect.segments),
                    borrow.loan_owner_path(previous),
                )
        })
}
