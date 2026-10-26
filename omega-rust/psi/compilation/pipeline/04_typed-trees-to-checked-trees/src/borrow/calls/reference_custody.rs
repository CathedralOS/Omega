//! Exact normal-call descriptor generations. The checking evaluator supplies
//! an entry/invocation/join graph; each node borrows its authentic predecessor
//! roster and records the output's existing permission suppliers. Alternatives
//! merge once. Published statement lifetimes remain owned by the state builder.

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
    incoming: &[StateLoanTracker],
) -> Result<(), diagnostics::Diagnostic> {
    let Some(schedule) = frames
        .and_then(|frames| frames.stored_reference_call_contexts(machine, state, statement_index))
    else {
        return Ok(());
    };
    let mut contexts: Vec<Option<Vec<StateLoanTracker>>> = Vec::new();
    let mut published_calls = Vec::new();
    for context in schedule {
        use validation::StoredReferenceCallContext;
        let current = match context {
            StoredReferenceCallContext::Entry if contexts.is_empty() => Some(incoming.to_vec()),
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
                if effects.is_empty() {
                    contexts.push(Some(incoming.clone()));
                    continue;
                }
                let mut matching = calls.handles().filter(|handle| {
                    let call = arenas.calls.get(*handle);
                    call.statement_index == statement_index
                        && call.authored_expression == expression
                });
                let Some(call) = matching.next().filter(|_| matching.next().is_none()) else {
                    return Err(diagnostics::Diagnostic::error(
                        "normal reference-slot completion has no unique checked invocation",
                    ));
                };
                if published_calls.contains(&call) {
                    return Err(diagnostics::Diagnostic::error(
                        "normal reference-slot invocation has more than one completion context",
                    ));
                }
                published_calls.push(call);
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
    Ok(())
}

/// A definite transferred slot replaces exactly the descriptor occurrences
/// stored beneath that destination. May-select destinations keep unselected
/// occurrences. Backing address equality supplies no replacement authority.
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
