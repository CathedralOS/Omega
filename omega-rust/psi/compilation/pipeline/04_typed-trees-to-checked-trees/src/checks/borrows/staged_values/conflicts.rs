//! Exclude exclusive access and owned consumption while a copied operand
//! retains the selected backing. Timing and branch compatibility are shared
//! with ownership replay; this judgment grants no permission or ancestry.
use super::super::overlap::StatedOrderingPremise;
use super::captures::HeldLeaf;
use crate::checks::ownership_occurrences::{StatementOccurrence, StatementStep, routes_compatible};
use arena::Handle;
use checked_trees::{BorrowAccessKind, CapturedPlace, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

pub(super) fn check<'program>(
    program: &'program dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: usize,
    occurrences: &[StatementOccurrence],
    leaves: &[HeldLeaf],
    premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    bounds: &mut Option<validation::ImmutableBoundLookup<'program>>,
) {
    let moves = facts
        .flow
        .ownership
        .storage_transfers
        .span_or_empty(state.storage_transfers);
    for (position, occurrence) in occurrences.iter().enumerate() {
        let StatementStep::Move {
            event,
            ordered: true,
            ..
        } = occurrence.step
        else {
            continue;
        };
        let event = &moves[event];
        let segments = facts.flow.ownership.segments.span_or_empty(event.segments);
        if !event.expression.is_valid()
            || !crate::checks::multiplicity::transfers_owned_storage(
                program,
                state.state_symbol,
                statement,
                event,
                segments,
            )
        {
            continue;
        }
        let moved = CapturedPlace {
            incoming_reference: Handle::invalid(),
            root: event.root,
            segments: segments.to_vec(),
        };
        for held in leaves.iter().filter(|held| {
            held.copied
                && held.completion <= position
                && position < held.until
                && held.route.is_none_or(|route| {
                    routes_compatible(&occurrences[route].route, &occurrence.route)
                })
        }) {
            if held.place.as_ref().is_some_and(|place| {
                super::super::overlap::captured_place_compatibility(
                    program,
                    place,
                    &held.access,
                    &moved,
                    &BorrowAccessKind::Mutable,
                    premises,
                    bounds,
                )
                .non_interfering
            }) {
                continue;
            }
            diagnostics.push(super::refusal(
                program,
                state,
                statement,
                held.expression,
                "a completed copied argument retains backing storage consumed by a later operand",
            ));
            return;
        }
    }
    for (index, left) in leaves.iter().enumerate() {
        for right in &leaves[index + 1..] {
            if !(left.copied || right.copied)
                || !(left.access.is_exclusive() || right.access.is_exclusive())
                || left.completion >= right.until
                || right.completion >= left.until
                || !compatible_routes(occurrences, left.route, right.route)
            {
                continue;
            }
            let compatible = match (&left.place, &right.place) {
                (Some(left_place), Some(right_place)) => {
                    super::super::overlap::captured_place_compatibility(
                        program,
                        left_place,
                        &left.access,
                        right_place,
                        &right.access,
                        premises,
                        bounds,
                    )
                    .non_interfering
                }
                _ => false,
            };
            if compatible {
                continue;
            }
            diagnostics.push(super::refusal(
                program, state, statement, left.expression,
                "a completed copied argument retains backing storage overlapping a staged exclusive loan",
            ));
            return;
        }
    }
}

fn compatible_routes(
    occurrences: &[StatementOccurrence],
    left: Option<usize>,
    right: Option<usize>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            routes_compatible(&occurrences[left].route, &occurrences[right].route)
        }
        _ => true,
    }
}
