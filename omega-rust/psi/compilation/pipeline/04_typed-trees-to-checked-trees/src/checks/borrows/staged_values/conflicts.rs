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
        for (right_offset, right) in leaves[index + 1..].iter().enumerate() {
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
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE staged-copy-conflict state={:?} statement={} left_index={} right_index={}",
                    state.state_symbol,
                    statement,
                    index,
                    index + 1 + right_offset,
                );
                trace_held(facts, "left", left);
                trace_held(facts, "right", right);
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

// Failure-only imaging of the exact pair already selected by the check. These
// bounded arena reads neither resolve storage nor change its refusal decision.
fn trace_held(facts: &CheckFacts, side: &str, held: &HeldLeaf) {
    let control = &facts.flow.control;
    let capture = control.operands.get(held.operand);
    eprintln!(
        "CHECKSITE staged-copy-held side={} operand={:?} expression={:?} type={:?} completion={} until={} route={:?} copied={} access={:?} place={:?} path_count={} path_prefix={:?}",
        side,
        held.operand,
        held.expression,
        capture.type_reference,
        held.completion,
        held.until,
        held.route,
        held.copied,
        held.access,
        held.place
            .as_ref()
            .map(|place| (place.incoming_reference, place.root)),
        held.place.as_ref().map_or(0, |place| place.segments.len()),
        held.place
            .as_ref()
            .map(|place| &place.segments[..place.segments.len().min(16)]),
    );
    eprintln!(
        "CHECKSITE staged-copy-capture side={} kind={:?} source={:?} backing={:?} snapshot={:?} retention={:?} referent_count={} reference_leaf_count={}",
        side,
        capture.completion,
        capture.value_source,
        capture.backing,
        capture.storage_snapshot,
        capture.retention,
        capture.referents.len(),
        capture.reference_leaves.len(),
    );
    trace_profile(facts, side, capture.retention);
    for (index, leaf) in control
        .captured_reference_leaves
        .span_or_empty(capture.reference_leaves)
        .iter()
        .take(4)
        .enumerate()
    {
        let path = control.operand_child_paths.span_or_empty(leaf.path);
        eprintln!(
            "CHECKSITE staged-copy-reference side={} index={} type={:?} path_count={} path_prefix={:?} referent_count={} retention={:?} constraints={:?}",
            side,
            index,
            leaf.type_reference,
            path.len(),
            &path[..path.len().min(16)],
            leaf.referents.len(),
            leaf.retention,
            leaf.constraints,
        );
        trace_profile(facts, side, leaf.retention);
    }
}

fn trace_profile(
    facts: &CheckFacts,
    side: &str,
    retention: Handle<checked_trees::FlowOperandRetention>,
) {
    let control = &facts.flow.control;
    if !control.operand_retentions.is_valid(retention) {
        return;
    }
    let profile = control.operand_retentions.get(retention);
    eprintln!(
        "CHECKSITE staged-copy-profile side={} retention={:?} immutable_image={} root_count={} descriptor={:?} enclosing={:?} region_count={} alternative_count={} stored_value={:?} leaf_count={}",
        side,
        retention,
        profile.immutable_image,
        profile.roots.len(),
        profile.descriptor,
        profile.enclosing_storage,
        profile.enclosing_regions.len(),
        profile.alternatives.len(),
        profile.stored_value,
        profile.leaves.len(),
    );
    for (index, leaf) in control
        .operand_retained_leaves
        .span_or_empty(profile.leaves)
        .iter()
        .take(4)
        .enumerate()
    {
        let path = control.operand_retained_paths.span_or_empty(leaf.path);
        eprintln!(
            "CHECKSITE staged-copy-retained side={} index={} path_count={} path_prefix={:?} retention={:?}",
            side,
            index,
            path.len(),
            &path[..path.len().min(16)],
            leaf.retention,
        );
    }
}
