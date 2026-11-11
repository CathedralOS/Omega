/*
    Call arguments are checked once against each other and the active loans.
    Flow supplies the left-to-right access lifetimes; this pass judges overlap
    over the original borrow facts. Source validation already judges reference
    formation against the exact receiving leaf, including every enclosing
    reference's access. Rechecking only a captured place's root here loses that
    authority: an immutable owned aggregate can contain an exclusive descriptor.
    An operand named through a local reference is judged against the active loans
    at the storage that reference's loans captured, by the same place-access
    judgment as a read or a write through it.
*/

use crate::checks::borrows::loan_occurrences::LoanOccurrences;
use crate::checks::ownership_occurrences::InvocationIdentity::Expression;
use crate::checks::ownership_occurrences::statement_steps;
use checked_trees::{BorrowCallFact, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use crate::flow::expressions;
use crate::published::labels::call_target_label;

mod conflicts {
    use super::super::moves::owned_storage_source;
    use super::super::overlap::captured_place_compatibility;
    use super::super::reads::loans_ended_before_call;
    use super::super::selected_retirement::before_call;
    use super::super::statements::local_jump;
    use super::super::statements::local_loan_ends_before_successor;
    use super::super::statements::source_exiting_without_local_loan_carriers;
    use super::receiver::explicit_carried_data_access;
    use super::staging::InvocationBoundary;
    use super::staging::access_released_in_prefix;
    use super::staging::completed_observation_before_terminal_transfer;
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use checked_trees::{BorrowCallFact, CheckFacts, FlowStateFact};
    use diagnostics::Diagnostic;

    use crate::published::labels::{borrow_access_label, symbol_name};

    use super::super::details::active_loan_detail;
    use super::super::overlap::StatedOrderingPremise;
    use super::super::overlap::canonical_place_non_interfering;
    use super::super::place_access;
    use super::super::simultaneous::{Hold, simultaneous_hold_conflict};

    /*
        Compare one retained storage move with earlier operands still held by a
        containing invocation. Exact expression occurrences, not affine-looking
        actual types, distinguish a move from implicit shared lending. The source
        schedule supplies branch compatibility; the call roster supplies loan
        completion, including a nested call whose result retains no borrow.
    */
    pub(in crate::checks::borrows) fn check_staged_operand_move<'program>(
        program: &'program dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        borrow_state: &checked_trees::StateBorrowFact,
        statement: &checked_trees::FlowStatementFact,
        event: &crate::flow::DiscoveredMoveEvent,
        preceding: &[crate::checks::ownership_occurrences::StatementOccurrence],
        route: &[crate::checks::ownership_occurrences::EvaluationBranch],
        premises: &[StatedOrderingPremise],
        diagnostics: &mut Vec<Diagnostic>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'program>>,
    ) {
        use crate::checks::ownership_occurrences::{StatementStep, routes_compatible};
        let path = facts.flow.ownership.segments.span_or_empty(event.segments);
        if !event.expression.is_valid()
            || !crate::checks::multiplicity::transfers_owned_storage(
                program,
                state.state_symbol,
                statement.statement_index,
                event,
                path,
            )
        {
            return;
        }
        let mut moved_source = None;
        for call in facts
            .borrow
            .calls
            .span_or_empty(borrow_state.calls)
            .iter()
            .filter(|call| call.statement_index == statement.statement_index)
        {
            let accesses = facts.borrow.argument_accesses.span_or_empty(call.accesses);
            for (move_index, _) in accesses
                .iter()
                .enumerate()
                .filter(|(_, access)| access.expression == event.expression)
            {
                for access in &accesses[..move_index] {
                    if access_released_in_prefix(access, preceding, route)
                        || preceding.iter().any(|occurrence| {
                            let StatementStep::Move {
                                event,
                                ordered: true,
                                ..
                            } = occurrence.step
                            else {
                                return false;
                            };
                            let previous = &facts
                                .flow
                                .ownership
                                .storage_transfers
                                .span_or_empty(state.storage_transfers)[event];
                            previous.expression == access.expression
                                && routes_compatible(&occurrence.route, route)
                                && crate::checks::multiplicity::transfers_owned_storage(
                                    program,
                                    state.state_symbol,
                                    statement.statement_index,
                                    previous,
                                    facts
                                        .flow
                                        .ownership
                                        .segments
                                        .span_or_empty(previous.segments),
                                )
                        })
                        || !preceding.iter().any(|occurrence| {
                            matches!(occurrence.step, StatementStep::Observe { expression, .. }
                                if expression == access.expression)
                                && routes_compatible(&occurrence.route, route)
                        })
                    {
                        continue;
                    }
                    let moved = match &moved_source {
                        Some(place) => place,
                        None => {
                            match owned_storage_source(
                                program,
                                facts,
                                loan_occurrences,
                                state,
                                statement,
                                event,
                                path,
                            ) {
                                Ok(Some(source)) => moved_source.insert(source.place),
                                Ok(None) => return,
                                Err(diagnostic) => {
                                    diagnostics.push(diagnostic);
                                    return;
                                }
                            }
                        }
                    };
                    let backings = place_access::argument_backings(
                        program,
                        facts,
                        loan_occurrences,
                        state,
                        call,
                        statement.entry_constraints,
                        access,
                    );
                    let Ok(backings) = backings else {
                        continue;
                    };
                    if backings.places.iter().all(|backing| {
                        captured_place_compatibility(
                            program,
                            &facts.borrow,
                            &backing.place,
                            &access.kind,
                            moved,
                            &checked_trees::BorrowAccessKind::Mutable,
                            premises,
                            bound_lookup,
                        )
                        .non_interfering
                    }) {
                        continue;
                    }
                    diagnostics.push(Diagnostic::error(format!(
                        "state `{}` receives owned `{}` while an earlier argument in the same call still borrows it",
                        crate::published::labels::call_target_label(program, call.target_symbol),
                        validation::CanonicalPlace { root: event.root, segments: path.to_vec() }.label(program),
                    )));
                }
            }
        }
    }

    pub(super) fn check_call_access_conflicts<'p>(
        program: &'p dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state_flow: &FlowStateFact,
        borrow_call: &BorrowCallFact,
        entry_constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
        target_name: &str,
        stated_premises: &[StatedOrderingPremise],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        diagnostics: &mut Vec<Diagnostic>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) {
        let accesses: Vec<_> = facts
            .borrow
            .argument_accesses
            .span_or_empty(borrow_call.accesses)
            .iter()
            .collect();
        let data_access = explicit_carried_data_access(program, facts, state_flow, borrow_call);
        let internal_jump = local_jump(program, state_flow, borrow_call).is_some();
        let exiting_source = source_exiting_without_local_loan_carriers(
            program,
            state_flow,
            borrow_call,
            call_frames,
        );
        let boundary = InvocationBoundary::at(program, facts, state_flow, borrow_call);
        let access_routes = boundary
            .as_ref()
            .and_then(|boundary| boundary.argument_routes(&accesses));
        let ended_before_call =
            loans_ended_before_call(program, facts, state_flow, borrow_call, boundary.as_ref());
        let active_loans: Vec<_> = facts
            .flow
            .borrow_loan_constraints(entry_constraints)
            .filter(|loan| {
                if before_call(program, facts, state_flow, borrow_call, *loan) {
                    return false;
                }
                !ended_before_call.contains(loan)
                    && !exiting_source.is_some_and(|source| {
                        local_loan_ends_before_successor(
                            program,
                            facts,
                            state_flow,
                            source,
                            *loan,
                            borrow_call,
                            call_frames,
                        )
                    })
            })
            .map(|loan| {
                let row = facts.borrow.loans.get(loan);
                let inputs = super::staging::returned_loan_input_range(
                    &facts.borrow,
                    row,
                    borrow_call,
                    &accesses,
                );
                (loan, row, inputs)
            })
            .collect();

        for transferred in
            crate::flow::owned_call_operand_places(program, facts, state_flow, borrow_call)
        {
            for (loan_handle, loan, _) in &active_loans {
                if canonical_place_non_interfering(
                    program,
                    &transferred,
                    *loan_handle,
                    &facts.borrow,
                    stated_premises,
                    bound_lookup,
                ) {
                    continue;
                }
                let detail = active_loan_detail(
                    state_flow,
                    facts,
                    *loan_handle,
                    borrow_call.statement_index,
                );
                diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives an owned value while local borrow `{}` is still active{}",
                    symbol_name(program, loan.owner_symbol),
                    detail
                        .map(|detail| format!(" ({detail})"))
                        .unwrap_or_default(),
                )));
            }
        }

        let mut operand_backings = Vec::with_capacity(accesses.len());
        for access in &accesses {
            let backings = place_access::argument_backings(
                program,
                facts,
                loan_occurrences,
                state_flow,
                borrow_call,
                entry_constraints,
                access,
            );
            match backings {
                Ok(backings) => operand_backings.push(backings),
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "state `{target_name}` receives `{}` without an exact retained backing for its loan: {reason}",
                        borrow_access_label(program, &facts.borrow, access),
                    )));
                    operand_backings.push(place_access::ArgumentBackings::default());
                }
            }
        }

        for (index, access) in accesses.iter().enumerate() {
            for (other_index, other_access) in accesses.iter().enumerate().skip(index + 1) {
                if access_routes.as_ref().is_some_and(|routes| {
                    let first = &routes[index];
                    let second = &routes[other_index];
                    !first.is_empty()
                        && !second.is_empty()
                        && !first.iter().any(|first| {
                            second.iter().any(|second| {
                                crate::checks::ownership_occurrences::routes_compatible(
                                    first, second,
                                )
                            })
                        })
                }) {
                    continue;
                }
                if other_access
                    .evaluation_start
                    .is_some_and(|start| start <= index)
                {
                    continue;
                }
                if (data_access == Some(index)
                    && other_access.end_before_access.is_none()
                    && carried_child_is_within_parent(
                        program,
                        state_flow,
                        borrow_call,
                        access,
                        &operand_backings[index],
                        &operand_backings[other_index],
                    ))
                    || (data_access == Some(other_index)
                        && access.end_before_access.is_none()
                        && carried_child_is_within_parent(
                            program,
                            state_flow,
                            borrow_call,
                            other_access,
                            &operand_backings[other_index],
                            &operand_backings[index],
                        ))
                {
                    continue;
                }
                if completed_observation_before_terminal_transfer(
                    program,
                    state_flow,
                    borrow_call,
                    boundary.as_ref(),
                    access,
                    other_access,
                ) || completed_observation_before_terminal_transfer(
                    program,
                    state_flow,
                    borrow_call,
                    boundary.as_ref(),
                    other_access,
                    access,
                ) {
                    continue;
                }
                if access
                    .end_before_access
                    .is_some_and(|end| end <= other_index)
                    || other_access
                        .end_before_access
                        .is_some_and(|end| end <= index)
                {
                    continue;
                }
                if (operand_backings[other_index].forwards(&operand_backings[index])
                    && access
                        .kind
                        .direct_reborrow_effect(&other_access.kind)
                        .is_some())
                    || (operand_backings[index].forwards(&operand_backings[other_index])
                        && other_access
                            .kind
                            .direct_reborrow_effect(&access.kind)
                            .is_some())
                {
                    continue;
                }
                if matches!(
                    (&access.kind, &other_access.kind),
                    (
                        checked_trees::BorrowAccessKind::Read,
                        checked_trees::BorrowAccessKind::Read
                    )
                ) {
                    continue;
                }
                let conflict =
                    super::held_places::places(&operand_backings[index]).find_map(|first| {
                        super::held_places::places(&operand_backings[other_index]).find_map(
                            |second| {
                                simultaneous_hold_conflict(
                                    program,
                                    &facts.borrow,
                                    first,
                                    &access.kind,
                                    second,
                                    &other_access.kind,
                                    stated_premises,
                                    bound_lookup,
                                )
                            },
                        )
                    });
                let Some(conflict) = conflict else {
                    continue;
                };
                let label = |hold| {
                    borrow_access_label(
                        program,
                        &facts.borrow,
                        match hold {
                            Hold::First => access,
                            Hold::Second => other_access,
                        },
                    )
                };
                if ::validation::trace_flags::check_sites() {
                    eprintln!(
                        "CHECKSITE simultaneous-hold state={:?} statement={} ordinal={} target={:?} first_index={} second_index={} first={:?} second={:?}",
                        state_flow.state_symbol,
                        borrow_call.statement_index,
                        borrow_call.call_ordinal,
                        borrow_call.target_symbol,
                        index,
                        other_index,
                        access,
                        other_access,
                    );
                    for (index, backings) in [
                        (index, &operand_backings[index]),
                        (other_index, &operand_backings[other_index]),
                    ] {
                        for backing in &backings.places {
                            eprintln!(
                                "CHECKSITE simultaneous-hold-backing index={} place={:?} lineage={:?}",
                                index, backing.place, backing.lineage,
                            );
                        }
                    }
                }
                diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives {}",
                    conflict.phrase(label, "another argument in the same call"),
                )));
            }

            if boundary.as_ref().is_some_and(|boundary| {
                boundary.access_finished_before_call(
                    program,
                    facts,
                    state_flow,
                    borrow_call,
                    index,
                    accesses.len(),
                    access,
                )
            }) {
                continue;
            }

            for (loan_handle, loan, producing_inputs) in &active_loans {
                if producing_inputs
                    .as_ref()
                    .is_some_and(|range| range.contains(&index))
                {
                    continue;
                }
                if local_jump(program, state_flow, borrow_call).is_some()
                    && access.end_before_access.is_none()
                    && access.root == facts::PlaceRoot::Symbol(loan.owner_symbol)
                    && crate::borrow::owner_path_overlaps_place_segments(
                        program,
                        facts.borrow.loan_owner_path(loan),
                        facts.borrow.access_segments.span_or_empty(access.segments),
                    )
                    && let Some(local_root) = loan.root.as_symbol()
                    && program.symbols.get(local_root).kind == symbols::SymbolKind::Local
                    && program.symbols.get(local_root).parent == state_flow.state_symbol
                    && !validation::loan_carrier_has_owned_storage(
                        program,
                        state_flow.state_symbol,
                        borrow_call.statement_index,
                        loan.owner_symbol,
                        facts.borrow.loan_owner_path(loan),
                    )
                    && !call_frames.is_some_and(|frames| {
                        let Some(jump) = local_jump(program, state_flow, borrow_call) else {
                            return false;
                        };
                        let Some((machine, source)) = validation::machine_state_by_symbols(
                            program,
                            state_flow.machine_symbol,
                            state_flow.state_symbol,
                        ) else {
                            return false;
                        };
                        frames.local_backing_survives_state_jump(
                            machine,
                            source,
                            borrow_call.statement_index,
                            jump.target,
                            validation::call_site_argument_expressions(program, &jump.site),
                            local_root,
                        )
                    })
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "state `{target_name}` receives a carried loan of local `{}`, whose storage ends at the source state exit without a complete retained-home relation",
                        symbol_name(program, local_root),
                    )));
                    continue;
                }
                if data_access == Some(index)
                    && (internal_jump
                        || accesses.iter().enumerate().any(|(child_index, child)| {
                            child_index != index
                                && child.end_before_access.is_none()
                                && operand_backings[child_index].places.iter().all(|backing| {
                                    backing.lineage.contains(loan_handle)
                                        && backing.place.incoming_reference
                                            == loan.incoming_reference
                                        && backing.place.incoming_owner_path
                                            == facts
                                                .borrow
                                                .access_segments
                                                .span_or_empty(loan.incoming_owner_path)
                                        && backing.place.root == loan.root
                                        && backing.place.segments
                                            == facts.borrow.loan_segments(loan)
                                })
                                && carried_child_is_within_parent(
                                    program,
                                    state_flow,
                                    borrow_call,
                                    access,
                                    &operand_backings[index],
                                    &operand_backings[child_index],
                                )
                        }))
                {
                    continue;
                }
                if !place_access::access_interferes(
                    program,
                    facts,
                    loan_occurrences,
                    &operand_backings[index].places,
                    &access.kind,
                    *loan_handle,
                    stated_premises,
                    bound_lookup,
                ) {
                    continue;
                }
                let detail = active_loan_detail(
                    state_flow,
                    facts,
                    *loan_handle,
                    borrow_call.statement_index,
                );
                if ::validation::trace_flags::state_reference_diagnostics() {
                    eprintln!(
                        "call-live-loan-conflict target={target_name} state={:?} statement={} ordinal={} access={access:?} loan={loan_handle:?} fact={loan:?}",
                        state_flow.state_symbol,
                        borrow_call.statement_index,
                        borrow_call.call_ordinal
                    );
                    for backing in &operand_backings[index].places {
                        eprintln!(
                            "call-live-loan-backing place={:?} lineage={:?}",
                            backing.place, backing.lineage
                        );
                    }
                }
                diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` while local borrow `{}` is still active{}",
                    borrow_access_label(program, &facts.borrow, access),
                    symbol_name(program, loan.owner_symbol),
                    detail
                        .map(|detail| format!(" ({detail})"))
                        .unwrap_or_default(),
                )));
            }
        }
    }

    /*
        A carried parent stays suspended by its actual descendant; it is not a
        second exclusive permission to the same whole value. Every retained origin
        and parent lineage must agree, and a stored pointer crossing cannot turn
        storage reached through that pointer into an owned child of its carrier.
    */
    fn carried_child_is_within_parent(
        program: &dyn validation::CheckingInput,
        state: &FlowStateFact,
        call: &BorrowCallFact,
        access: &checked_trees::BorrowArgumentAccessFact,
        parent: &place_access::ArgumentBackings,
        child: &place_access::ArgumentBackings,
    ) -> bool {
        let Some(reference) = validation::expression_type_reference_in_state(
            program,
            state.state_symbol,
            call.statement_index,
            access.expression,
        )
        .and_then(|reference| validation::reference_shell(program, reference)) else {
            return false;
        };
        !parent.places.is_empty()
            && !child.places.is_empty()
            && parent.places.iter().all(|parent| {
                child.places.iter().all(|child| {
                    let prefix = parent.place.segments.len();
                    parent.place.root == child.place.root
                        && parent.place.incoming_reference == child.place.incoming_reference
                        && parent.place.incoming_owner_path == child.place.incoming_owner_path
                        && prefix < child.place.segments.len()
                        && parent.place.segments.iter().zip(&child.place.segments).all(
                            |(parent, child)| {
                                crate::flow::canonical_place_segments_equal(
                                    program, *parent, *child,
                                )
                            },
                        )
                        && parent
                            .lineage
                            .iter()
                            .all(|loan| child.lineage.contains(loan))
                        && owned_child_storage(
                            program,
                            reference.referee,
                            &child.place.segments[prefix..],
                        )
                })
            })
    }

    /*
        Whether `segments` select storage the parent owns inline, crossing no
        stored reference. A trailing range over an owned fixed array still selects
        the parent's own elements (`&mut main.bytes[4] as &mut [Pair]` lends
        `main.bytes[4..12]`), so it is as much a child as one element. A range is
        accepted only as the last segment and only over a non-reference collection:
        storage reached through a view or pointer never becomes an owned child.
    */
    fn owned_child_storage(
        program: &dyn validation::CheckingInput,
        parent: typed_trees::types::TypeReferenceHandle,
        segments: &[facts::PlaceSegment],
    ) -> bool {
        let (prefix, range) = match segments.split_last() {
            Some((
                facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange,
                prefix,
            )) => (prefix, true),
            _ => (segments, false),
        };
        let Some(collection) = validation::owned_projection_type_reference(program, parent, prefix)
        else {
            return false;
        };
        !range
            || validation::reference_shell(program, collection).is_none()
                && validation::collection_element_type_reference(program, collection).is_some()
    }
}
mod held_places {
    /*
        Simultaneous holds use the complete retained access frontier.

        The common argument resolver has already authenticated the actual descriptor,
        its completion cut, every alternative and its loan lineage. That frontier may
        conservatively cover more storage than the pointer's exact address. Comparing
        all of it is sufficient for interference; requiring a second exact-address
        inventory would reject valid dynamic projections. This query supplies no
        address, narrower permission, pointer load or lifetime extension.
    */
    use checked_trees::CapturedPlace;

    use super::super::place_access::ArgumentBackings;

    pub(super) fn places(backings: &ArgumentBackings) -> impl Iterator<Item = &CapturedPlace> {
        backings.places.iter().map(|backing| &backing.place)
    }
}
mod receiver {
    /*
        Exclusive-place operations and explicit state-jump subject access.
        Ordinary machine arguments use the shared call compatibility checks.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use crate::published::places::place_at;
    use checked_trees::{BorrowAccessKind, CapturedPlace, CheckFacts, FlowStateFact};
    use diagnostics::Diagnostic;
    use typed_trees::types::TypeReferenceNode;

    use super::super::overlap::{StatedOrderingPremise, captured_place_compatibility};
    use super::super::statements::local_jump;

    use super::super::{access_sources, aliases, place_access};
    mod footprint {
        /*
            Terminal-carrier footprints preserve loans carried into a successor.
            Ordinary returning calls use explicit argument exclusivity, not a
            receiver-specific relaxation based on which fields the callee accesses.
        */
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use crate::flow;
        use validation::CanonicalPlace;

        use checked_trees::{
            BorrowAccessKind, BorrowCallFact, CapturedPlace, CheckFacts, FlowStateFact,
        };
        use diagnostics::Diagnostic;
        use language_semantics::ReferenceAccess;
        use typed_trees::types::TypeReferenceNode;

        use super::super::super::overlap::{StatedOrderingPremise, captured_place_compatibility};
        use super::aliases;
        use crate::flow::{StateMutationSummaryCache, StateReceiverReadCache};
        use crate::published::labels::call_target_label;

        /* The receiver summaries a footprint judgment reads. */
        pub(in crate::checks::borrows) struct ReceiverSummaries<'cache> {
            pub(in crate::checks::borrows) writes: &'cache StateMutationSummaryCache,
            pub(in crate::checks::borrows) reads: &'cache StateReceiverReadCache,
        }

        pub(in crate::checks::borrows) fn check_receiver_footprint_against_argument_loans<'p>(
            program: &'p dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state_flow: &FlowStateFact,
            call: &BorrowCallFact,
            stated_premises: &[StatedOrderingPremise],
            summaries: ReceiverSummaries<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
        ) {
            if forwarded_mutable_parent_machine(program, state_flow, call).is_none() {
                return;
            }
            let data_access = explicit_carried_data_access(program, facts, state_flow, call);
            let retained_arguments: Vec<_> = facts
                .borrow
                .argument_accesses
                .span_or_empty(call.accesses)
                .iter()
                .enumerate()
                .filter(|(ordinal, argument)| {
                    Some(*ordinal) != data_access && argument.end_before_access.is_none()
                })
                .map(|(_, argument)| argument)
                .collect();
            let entry_constraints =
                super::super::call_borrow_constraints(program, call, state_flow, facts);
            for argument in retained_arguments {
                let operand = CapturedPlace {
                    root: argument.root,
                    segments: facts.borrow.access_segments(argument).to_vec(),
                    ..Default::default()
                };
                let loan = CanonicalPlace {
                    root: operand.root,
                    segments: operand.segments.clone(),
                };
                let writes = captured_places(flow::call_receiver_write_accesses(
                    program,
                    state_flow.machine_symbol,
                    state_flow.state_symbol,
                    &facts.borrow,
                    call,
                    summaries.writes,
                    &loan,
                ));
                let reads = if argument.kind.is_exclusive() {
                    captured_places(flow::call_receiver_read_accesses(
                        program,
                        state_flow.machine_symbol,
                        state_flow.state_symbol,
                        &facts.borrow,
                        call,
                        summaries.reads,
                        Some(&loan),
                    ))
                } else {
                    Vec::new()
                };
                let lent_place = if reads.is_empty() || !argument.kind.is_exclusive() {
                    None
                } else {
                    aliases::resolve_place(
                        program,
                        facts,
                        loan_occurrences,
                        state_flow,
                        entry_constraints,
                        operand.clone(),
                    )
                    .filter(|resolved| resolved.local_loan.is_valid())
                    .map(|resolved| resolved.place)
                };
                let argument_place = operand;
                let mut interferes =
                    |receiver_place: &CapturedPlace,
                     receiver_access,
                     argument_place: &CapturedPlace| {
                        !captured_place_compatibility(
                            program,
                            &facts.borrow,
                            receiver_place,
                            receiver_access,
                            argument_place,
                            &argument.kind,
                            stated_premises,
                            bound_lookup,
                        )
                        .non_interfering
                    };
                let read_argument_place = lent_place.as_ref().unwrap_or(&argument_place);
                let Some((receiver_access, receiver_place, argument_place)) = writes
                    .iter()
                    .find(|write| interferes(write, &BorrowAccessKind::Mutable, &argument_place))
                    .map(|write| ("writes", write, &argument_place))
                    .or_else(|| {
                        reads
                            .iter()
                            .find(|read| {
                                interferes(read, &BorrowAccessKind::Read, read_argument_place)
                            })
                            .map(|read| ("reads", read, read_argument_place))
                    })
                else {
                    continue;
                };
                diagnostics.push(Diagnostic::error(format!(
                    "state `{}` receives `{}` as {} while its mutable receiver {receiver_access} `{}`",
                    call_target_label(program, call.target_symbol),
                    place_label(program, argument_place),
                    match argument.kind {
                        BorrowAccessKind::Read => "read-only",
                        BorrowAccessKind::Mutable => "mutable",
                        BorrowAccessKind::WriteOnly => "write-only",
                    },
                    place_label(program, receiver_place),
                )));
            }
        }

        /*
            A terminal successor receives its existing data permission together with
            the child loans it keeps alive. The parent is judged by that successor's footprint;
            it is not a second independent loan of every field.
            Keep the original access roster and its coordinates for operand evaluation
            and independent replay. Only one exact retained parent access can use this
            judgment, and ordinary returning calls retain their operand exclusivity.
        */
        pub(in crate::checks::borrows::calls) fn explicit_carried_data_access(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            state_flow: &FlowStateFact,
            call: &BorrowCallFact,
        ) -> Option<usize> {
            let machine = forwarded_mutable_parent_machine(program, state_flow, call)?;
            retained_subject_access(program, facts, state_flow, call, machine)
        }

        /*
            The one RETAINED mutable access naming `call`'s subject parent, if exactly
            one does. A completed access is never it: the filter below drops them.
        */
        fn retained_subject_access(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            state_flow: &FlowStateFact,
            call: &BorrowCallFact,
            machine: &typed_trees::machine::Machine,
        ) -> Option<usize> {
            let target = program.state_by_symbol(call.target_symbol)?;
            flow::family_data_binding_parameter(
                program,
                machine.symbol,
                program.state_parameters(target),
            )?;
            let site = validation::find_call_site(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
            let parent = flow::canonical_receiver_place_for_call_site(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                &site,
                call.statement_index,
            )?;
            let root = parent.root.as_symbol()?;
            let mut matching = facts
                .borrow
                .argument_accesses
                .span_or_empty(call.accesses)
                .iter()
                .enumerate()
                .filter(|(_, access)| {
                    access.kind == BorrowAccessKind::Mutable
                        && access.end_before_access.is_none()
                        && access.root == facts::PlaceRoot::Symbol(root)
                        && facts.borrow.access_segments(access) == parent.segments
                });
            let (ordinal, _) = matching.next()?;
            matching.next().is_none().then_some(ordinal)
        }

        /*
            A terminal invocation can forward an existing data parent while that parent
            stays suspended by its child. This does not authorize fresh overlapping borrow formations
            or change any caller-local scope/cleanup boundary.
        */
        fn forwarded_mutable_parent_machine<'p>(
            program: &'p dyn ::validation::CheckingInput,
            state_flow: &FlowStateFact,
            call: &BorrowCallFact,
        ) -> Option<&'p typed_trees::machine::Machine> {
            let transfer = flow::terminal_invocation(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                call,
            )?;
            let machine = program.machine_by_symbol(state_flow.machine_symbol)?;
            let target = transfer.target;
            let parameter = flow::family_data_binding_parameter(
                program,
                machine.symbol,
                program.state_parameters(target),
            )?;
            if !matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Reference {
                    access: ReferenceAccess::Mutable,
                    ..
                }
            ) {
                return None;
            }
            if super::local_jump(program, state_flow, call).is_none() {
                let ordinal = typed_trees::signature::state_parameter_position(
                    program.state_parameters(target),
                    parameter.symbol,
                )?;
                let actual = *validation::call_site_argument_expressions(program, &transfer.site)
                    .get(ordinal)?;
                if validation::reference_formation_source(program, actual).is_some()
                    || !validation::expression_type_reference_in_state(
                        program,
                        state_flow.state_symbol,
                        call.statement_index,
                        actual,
                    )
                    .and_then(|reference| validation::reference_shell(program, reference))
                    .is_some_and(|shell| shell.access == ReferenceAccess::Mutable)
                {
                    return None;
                }
            }
            let receiver = validation::find_call_site(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )
            .and_then(|site| {
                flow::canonical_receiver_place_for_call_site(
                    program,
                    state_flow.machine_symbol,
                    state_flow.state_symbol,
                    &site,
                    call.statement_index,
                )
            });
            let Some(CanonicalPlace {
                root: facts::PlaceRoot::Symbol(_),
                segments,
            }) = receiver
            else {
                return None;
            };
            segments.is_empty().then_some(machine)
        }

        /*
            Summaries already use the caller's explicit storage coordinates, including
            computed backing. Projection does not require a named declaration root.
        */
        fn captured_places(places: Vec<CanonicalPlace>) -> Vec<CapturedPlace> {
            places
                .into_iter()
                .filter_map(|place| {
                    place.root.is_valid().then_some(CapturedPlace {
                        root: place.root,
                        segments: place.segments,
                        ..Default::default()
                    })
                })
                .collect()
        }

        fn place_label(program: &dyn ::validation::CheckingInput, place: &CapturedPlace) -> String {
            facts::canonical_place_label_from_parts(program, place.root, &place.segments)
        }
    }

    pub(super) use footprint::explicit_carried_data_access;
    pub(in crate::checks::borrows) use footprint::{
        ReceiverSummaries, check_receiver_footprint_against_argument_loans,
    };

    /*
        A compiler-owned operation borrows a checked place without invoking a
        source machine. Retained alias ancestry supplies authority, not competing
        loans; every other live overlapping loan must remain excluded.
    */
    pub(in crate::checks::borrows) fn check_exclusive_place_use<'p>(
        program: &'p dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state_flow: &FlowStateFact,
        statement: &checked_trees::FlowStatementFact,
        expression: typed_trees::expression::ExpressionHandle,
        later_operand: typed_trees::expression::ExpressionHandle,
        stated_premises: &[StatedOrderingPremise],
        diagnostics: &mut Vec<Diagnostic>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) {
        let Some(receiver) = access_sources::resolve(
            program,
            facts,
            loan_occurrences,
            state_flow,
            statement,
            expression,
        ) else {
            diagnostics.push(Diagnostic::error(
                "exclusive operation requires an exact receiver source and available loan authority",
            ));
            return;
        };
        if !receiver_is_writable(
            program,
            facts,
            state_flow,
            statement.entry_constraints,
            &receiver.place,
        ) {
            diagnostics.push(Diagnostic::error(
                "exclusive operation receiver is not writable in this state",
            ));
            return;
        }
        for loan_handle in facts
            .flow
            .borrow_loan_constraints(statement.entry_constraints)
        {
            if place_access::access_interferes(
                program,
                facts,
                loan_occurrences,
                std::slice::from_ref(&receiver),
                &BorrowAccessKind::Mutable,
                loan_handle,
                stated_premises,
                bound_lookup,
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "exclusive operation receiver overlaps local borrow `{}` which is still active",
                    program
                        .symbols
                        .name(facts.borrow.loans.get(loan_handle).owner_symbol),
                )));
            }
        }
        if later_operand.is_valid()
            && (matches!(
                program.expression_table.expression(expression),
                typed_trees::expression::ExpressionNode::Borrow(_)
            ) || place_at(
                program,
                state_flow.state_symbol,
                statement.statement_index,
                expression,
            )
            .is_none_or(|place| !matches!(place.root, facts::PlaceRoot::Symbol(_))))
        {
            let mut segments = arena::Arena::new();
            let mut accesses = arena::Arena::new();
            let span = crate::borrow::accesses::collect_argument_accesses(
                program,
                &mut segments,
                &mut accesses,
                &[later_operand],
                state_flow.state_symbol,
                statement.statement_index,
                state_flow.machine_symbol,
                symbols::SymbolHandle::default(),
                crate::borrow::accesses::ArgumentAccessSchedule::Invocation,
            );
            for access in accesses.span_or_empty(span) {
                let place = CapturedPlace {
                    root: access.root,
                    segments: segments.span_or_empty(access.segments).to_vec(),
                    ..Default::default()
                };
                let Some(access) = aliases::resolve_place(
                    program,
                    facts,
                    loan_occurrences,
                    state_flow,
                    statement.entry_constraints,
                    place,
                ) else {
                    diagnostics.push(Diagnostic::error(
                        "exclusive receiver result requires an exact later operand loan origin",
                    ));
                    continue;
                };
                if !captured_place_compatibility(
                    program,
                    &facts.borrow,
                    &receiver.place,
                    &BorrowAccessKind::Mutable,
                    &access.place,
                    &BorrowAccessKind::Read,
                    stated_premises,
                    bound_lookup,
                )
                .non_interfering
                {
                    diagnostics.push(Diagnostic::error(
                        "later operand accesses storage held by the exclusive receiver result",
                    ));
                }
            }
        }
    }

    fn receiver_is_writable(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        entry_constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
        receiver: &CapturedPlace,
    ) -> bool {
        let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
            return false;
        };
        if receiver.root == facts::PlaceRoot::Symbol(state_flow.machine_symbol) {
            return false;
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let Some(local) = statement.as_local_data()
                && facts::PlaceRoot::Symbol(local.symbol) == receiver.root
            {
                return match program
                    .type_reference_table
                    .type_reference(local.type_reference)
                {
                    TypeReferenceNode::Reference { access, .. } => access.is_exclusive(),
                    _ => local.is_mutable,
                };
            }
        }
        facts
            .flow
            .borrow_writable_root_constraints(entry_constraints)
            .any(|root| {
                facts::PlaceRoot::Symbol(facts.borrow.writable_roots.get(root).symbol)
                    == receiver.root
            })
    }
}
mod staging {
    /*
        Terminal invocations evaluate arguments in authored order and capture scalar
        snapshots before committing the arrival transfer of existing reference carriers.
        A completed read does not create a competing successor loan, including when
        an alias supplied that read. This follows state_contracts.md
        (Bindings and arrivals) and expressions.md (Evaluation schedule), independent
        of attached-data namespaces. The distinction is transfer versus formation:
        explicit new borrows, retained observations, nested-call
        writes and ordinary returning calls keep their normal conflict judgments.
        Each invocation checks its live access windows against the active loans.
        Finished nested inputs are checked at their own entry, while descendants
        stored by that invocation remain active; no parent authority is restored.
    */
    use checked_trees::{
        BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallFact, CheckFacts, FlowStateFact,
    };
    use typed_trees::expression::ExpressionNode;

    /*
        A returned loan cannot compete with inputs evaluated before it existed.
        The checked access range ends at this exact result, so later operands still
        compete with all of its input lifetime debts, including unselected inputs.
    */
    pub(super) fn returned_loan_input_range(
        borrow: &checked_trees::BorrowFacts,
        loan: &checked_trees::BorrowLoanFact,
        outer: &BorrowCallFact,
        accesses: &[&BorrowArgumentAccessFact],
    ) -> Option<std::ops::Range<usize>> {
        if !borrow.is_call_result_loan(loan) || loan.statement_index != outer.statement_index {
            return None;
        }
        let producer = borrow.calls.get(loan.established_by_call);
        if producer.call_ordinal == outer.call_ordinal {
            return None;
        }
        let mut results = accesses.iter().enumerate().filter_map(|(end, access)| {
            let start = access.evaluation_start?;
            (access.expression == producer.authored_expression && start < end).then_some(start..end)
        });
        let range = results.next()?;
        results.next().is_none().then_some(range)
    }

    /*
        One exact invocation boundary in the shared evaluated occurrence schedule.
        This is borrowed-check construction scratch, not a new loan or cleanup plan.
    */
    pub(in crate::checks::borrows) struct InvocationBoundary {
        pub(in crate::checks::borrows) occurrences:
            Vec<crate::checks::ownership_occurrences::StatementOccurrence>,
        pub(in crate::checks::borrows) completion: usize,
        pub(in crate::checks::borrows) ends_source_path: bool,
    }

    impl InvocationBoundary {
        pub(in crate::checks::borrows) fn at(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            flow: &FlowStateFact,
            call: &BorrowCallFact,
        ) -> Option<Self> {
            use crate::checks::ownership_occurrences::InvocationIdentity;
            let (machine, state) = validation::machine_state_by_symbols(
                program,
                flow.machine_symbol,
                flow.state_symbol,
            )?;
            facts
                .flow
                .control
                .statements
                .span(flow.statements)?
                .iter()
                .find(|statement| statement.statement_index == call.statement_index)?;
            let node = program
                .statement_table
                .statement_at(state, call.statement_index)?;
            let site = validation::find_call_site_in_state(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            )?;
            let target = validation::transition_call_target(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            );
            let ends_source_path = target.is_some_and(|target| {
                match (program.statement_table.transition_target(target), site) {
                    (
                        typed_trees::statement::TransitionTargetNode::Value(value),
                        validation::CallSite::Expression { expression, .. },
                    ) => *value == expression,
                    (
                        typed_trees::statement::TransitionTargetNode::Named { .. },
                        validation::CallSite::TransitionNamed { .. },
                    ) => true,
                    _ => false,
                }
            });
            let identity = InvocationIdentity::from_call_site(site, target)?;
            let occurrences = crate::checks::ownership_occurrences::statement_steps(
                program,
                machine,
                state,
                call.statement_index,
                node,
                &[],
                &facts.operators,
            );
            let mut completions = occurrences
                .iter()
                .enumerate()
                .filter(|(_, occurrence)| occurrence.invocation == Some(identity));
            let (completion, _) = completions.next()?;
            if completions.next().is_some() {
                return None;
            }
            Some(Self {
                occurrences,
                completion,
                ends_source_path,
            })
        }

        /*
            Retain the evaluated routes of each staged operand before this exact
            invocation. Opposite arms of one decision never form simultaneous
            loans. A missing occurrence remains unknown, rather than authorizing
            omission of its ordinary conflict check.
        */
        pub(in crate::checks::borrows) fn argument_routes(
            &self,
            accesses: &[&BorrowArgumentAccessFact],
        ) -> Option<Vec<Vec<&[crate::checks::ownership_occurrences::EvaluationBranch]>>> {
            use crate::checks::ownership_occurrences::{StatementStep, routes_compatible};
            let prefix = &self.occurrences[..self.completion];
            if prefix.iter().all(|occurrence| occurrence.route.is_empty()) {
                return None;
            }
            let invocation_route = &self.occurrences[self.completion].route;
            Some(
                accesses
                    .iter()
                    .map(|access| {
                        prefix
                            .iter()
                            .filter_map(|occurrence| {
                                (matches!(occurrence.step, StatementStep::Complete(expression)
                                    if expression == access.expression)
                                    && routes_compatible(&occurrence.route, invocation_route))
                                .then_some(occurrence.route.as_slice())
                            })
                            .collect()
                    })
                    .collect(),
            )
        }

        /*
            A finished nested invocation releases its direct input window. Its
            stored descendants remain live and independently constrain later uses.
            A copied descriptor read is not such an invocation completion.
        */
        pub(in crate::checks::borrows) fn access_finished_before_call(
            &self,
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            flow: &FlowStateFact,
            call: &BorrowCallFact,
            access_index: usize,
            access_count: usize,
            access: &BorrowArgumentAccessFact,
        ) -> bool {
            use crate::checks::ownership_occurrences::{InvocationIdentity, routes_compatible};
            if !access
                .end_before_access
                .is_some_and(|end| end > access_index && end <= access_count)
                || !access.released_at.is_valid()
                || access.released_at == access.expression
            {
                return false;
            }
            let prefix = &self.occurrences[..self.completion];
            let route = &self.occurrences[self.completion].route;
            if !access_released_in_prefix(access, prefix, route) {
                return false;
            }
            let mut completions = prefix.iter().filter(|occurrence| {
                occurrence.invocation == Some(InvocationIdentity::Expression(access.released_at))
                    && routes_compatible(&occurrence.route, route)
            });
            if completions.next().is_none() || completions.next().is_some() {
                return false;
            }
            let Some(borrow_state) = super::super::matching_borrow_state(facts.view(), flow) else {
                return false;
            };
            let Some(calls) = facts.borrow.calls.span(borrow_state.calls) else {
                return false;
            };
            let mut suppliers = calls.iter().filter(|candidate| {
                candidate.statement_index == call.statement_index
                    && candidate.call_ordinal != call.call_ordinal
                    && candidate.authored_expression == access.released_at
            });
            if suppliers.next().is_none() || suppliers.next().is_some() {
                return false;
            }
            validation::expression_type_reference_in_state(
                program,
                flow.state_symbol,
                call.statement_index,
                access.released_at,
            )
            .is_some_and(|reference| {
                validation::has_contents(
                    program,
                    reference,
                    &[],
                    validation::ContentsRequirement::BorrowFree,
                )
            })
        }
    }

    /*
        A staged access ends at its producer-owned completion, not when the next
        accessed place happens to be observed. Explicit moves supply the prefix
        before their occurrence; an implicit completion supplies the inclusive
        prefix after completion. Both keep the same branch selection.
    */
    pub(in crate::checks::borrows) fn access_released_in_prefix(
        access: &BorrowArgumentAccessFact,
        prefix: &[crate::checks::ownership_occurrences::StatementOccurrence],
        route: &[crate::checks::ownership_occurrences::EvaluationBranch],
    ) -> bool {
        use crate::checks::ownership_occurrences::{StatementStep, routes_compatible};
        access.end_before_access == Some(0)
            || (access.released_at.is_valid()
                && prefix.iter().any(|occurrence| {
                    matches!(occurrence.step, StatementStep::Complete(expression)
                        if expression == access.released_at)
                        && routes_compatible(&occurrence.route, route)
                }))
    }

    pub(super) fn completed_observation_before_terminal_transfer(
        program: &dyn validation::CheckingInput,
        state_flow: &FlowStateFact,
        call: &BorrowCallFact,
        boundary: Option<&InvocationBoundary>,
        transferred: &BorrowArgumentAccessFact,
        copied: &BorrowArgumentAccessFact,
    ) -> bool {
        let Some(boundary) = boundary.filter(|boundary| boundary.ends_source_path) else {
            return false;
        };
        if transferred.kind != BorrowAccessKind::Mutable
            || transferred.end_before_access.is_some()
            || copied.kind != BorrowAccessKind::Read
            || copied.end_before_access.is_none()
            || !access_released_in_prefix(
                copied,
                &boundary.occurrences[..boundary.completion],
                &boundary.occurrences[boundary.completion].route,
            )
        {
            return false;
        }
        let mut expression = transferred.expression;
        loop {
            match program.expression_table.expression(expression) {
                ExpressionNode::Borrow(_) | ExpressionNode::Call(_) => return false,
                ExpressionNode::Cast(cast) => {
                    if cast.form.is_recast() {
                        return false;
                    }
                    expression = cast.value;
                }
                _ => break,
            }
        }
        validation::expression_type_reference_in_state(
            program,
            state_flow.state_symbol,
            call.statement_index,
            expression,
        )
        .and_then(|reference| validation::reference_shell(program, reference))
        .is_some_and(|reference| reference.access == language_semantics::ReferenceAccess::Mutable)
    }
}

pub(super) use receiver::{
    ReceiverSummaries, check_exclusive_place_use, check_receiver_footprint_against_argument_loans,
};

use self::conflicts::check_call_access_conflicts;
use crate::checks::ranges::incoming_guards::IncomingGuardIndex;
pub(super) use conflicts::check_staged_operand_move;
pub(super) use staging::{InvocationBoundary, access_released_in_prefix};

pub(super) fn check_compatibility(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state_flows: &[(&FlowStateFact, Option<&checked_trees::StateBorrowFact>)],
    incoming_guards: &IncomingGuardIndex,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let per_state: Vec<Vec<Diagnostic>> = validation::checking_thread_pool().install(|| {
        use rayon::prelude::*;
        state_flows
            .par_iter()
            .filter_map(|&(state_flow, borrow_state)| {
                let borrow_state = borrow_state?;
                let calls = facts.borrow.calls.span_or_empty(borrow_state.calls);
                if calls.is_empty() {
                    return None;
                }
                let mut local_diagnostics = Vec::new();
                let mut local_bound_lookup = None;
                let premises = validation::find_state_in_machine(
                    program,
                    state_flow.machine_symbol,
                    state_flow.state_symbol,
                )
                .and_then(|state| {
                    program
                        .machine_by_symbol(state_flow.machine_symbol)
                        .map(|machine| (machine, state))
                })
                .map(|(machine, state)| {
                    super::overlap::stated_ordering_premises(
                        program,
                        facts,
                        machine,
                        state,
                        incoming_guards,
                        &mut local_bound_lookup,
                    )
                })
                .unwrap_or_default();
                let mut guarantees =
                    crate::checks::contracts::prover::call_guarantees::StateGuarantees::new(
                        program,
                        facts.view(),
                        state_flow,
                        call_frames,
                    );
                for call in calls {
                    if !invocation_is_evaluated(program, facts, state_flow, call) {
                        continue;
                    }
                    let mut available_premises = premises.clone();
                    super::overlap::append_statement_premises(
                        program,
                        facts,
                        call.statement_index,
                        &mut guarantees,
                        &mut available_premises,
                        &mut local_bound_lookup,
                    );
                    let target_name = call_target_label(program, call.target_symbol);
                    let entry_constraints =
                        call_borrow_constraints(program, call, state_flow, facts);
                    check_build_selection_receiver(
                        program,
                        facts,
                        loan_occurrences,
                        state_flow,
                        calls,
                        call,
                        &available_premises,
                        &mut local_diagnostics,
                        &mut local_bound_lookup,
                    );
                    check_call_access_conflicts(
                        program,
                        facts,
                        loan_occurrences,
                        state_flow,
                        call,
                        entry_constraints,
                        &target_name,
                        &available_premises,
                        call_frames,
                        &mut local_diagnostics,
                        &mut local_bound_lookup,
                    );
                }
                (!local_diagnostics.is_empty()).then_some(local_diagnostics)
            })
            .collect()
    });
    for local_diagnostics in per_state {
        diagnostics.extend(local_diagnostics);
    }
}

/*
    Source recording calls require exclusive Build use even though their host
    preparation only observes identity. Judge the exact evaluated preparation
    prefix, independently of whether the commit is a statement or a nested
    expression. The emitted protocol never manufactures writes to Build fields.
*/
fn check_build_selection_receiver<'p>(
    program: &'p dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state_flow: &FlowStateFact,
    calls: &[BorrowCallFact],
    commit: &BorrowCallFact,
    premises: &[super::overlap::StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    if !commit.authored_expression.is_valid() {
        return;
    }
    let Some(operation) = expressions(program).call_node(commit.authored_expression) else {
        return;
    };
    // `prepare_tests` prepares a test-group registration on the Build's
    // `tests` facet, the same exclusive-use preparation as `prepare`.
    let is_preparation = |target: &str| matches!(target, "prepare" | "prepare_tests");
    if !operation.build_selection.is_valid() || is_preparation(operation.target.as_str()) {
        return;
    }
    let request = expressions(program).build_selection_request(operation.build_selection);
    let arguments = expressions(program).expression_handles(operation.arguments);
    let receiver_preparation = arguments.first().and_then(|expression| {
        let preparation = program.expression_table.call_node(*expression)?;
        let retained = expressions(program).build_selection_request(preparation.build_selection)?;
        if !is_preparation(preparation.target.as_str()) || Some(retained) != request {
            return None;
        }
        let receiver = expressions(program)
            .expression_handles(preparation.arguments)
            .first()?;
        Some((*expression, *receiver))
    });
    let Some((preparation_expression, receiver)) = receiver_preparation else {
        diagnostics.push(Diagnostic::error(
            "Build product operation lost its source-issued receiver preparation",
        ));
        return;
    };
    let Some(prefix) = calls.iter().rev().find(|call| {
        call.statement_index == commit.statement_index
            && call.authored_expression == preparation_expression
    }) else {
        diagnostics.push(Diagnostic::error(
            "Build product operation lost its evaluated receiver preparation",
        ));
        return;
    };
    let Some(statement) = facts
        .flow
        .control
        .statements
        .span_or_empty(state_flow.statements)
        .iter()
        .find(|statement| statement.statement_index == commit.statement_index)
    else {
        diagnostics.push(Diagnostic::error(
            "Build product operation lost its containing statement frontier",
        ));
        return;
    };
    if !invocation_is_evaluated(program, facts, state_flow, prefix) {
        diagnostics.push(Diagnostic::error(
            "Build product receiver preparation is absent from its evaluated schedule",
        ));
        return;
    }
    let Some(frontier) = facts.flow.state_call(
        state_flow,
        prefix.statement_index,
        prefix.call_ordinal,
        prefix.target_symbol,
        prefix.receiver_symbol,
    ) else {
        diagnostics.push(Diagnostic::error(
            "Build product receiver preparation lost its exact call-entry frontier",
        ));
        return;
    };
    let mut prefix_statement = statement.clone();
    prefix_statement.entry_constraints = frontier.entry_constraints;
    check_exclusive_place_use(
        program,
        facts,
        loan_occurrences,
        state_flow,
        &prefix_statement,
        receiver,
        arguments.get(1).copied().unwrap_or_default(),
        premises,
        diagnostics,
        bound_lookup,
    );
}

fn call_borrow_constraints<'a>(
    program: &dyn validation::CheckingInput,
    borrow_call: &BorrowCallFact,
    state_flow: &'a FlowStateFact,
    facts: &'a CheckFacts,
) -> arena::HandleSpan<checked_trees::FlowConstraintRef> {
    if let Some(observation) =
        super::place_access::storage_address_observation(program, facts, state_flow, borrow_call)
    {
        return observation.constraints;
    }
    facts.flow.state_call_entry_constraints(
        state_flow,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
        borrow_call.target_symbol,
        borrow_call.receiver_symbol,
    )
}

/*
    Borrow collection keeps stable authored call ordinals, including calls in
    unselected closed short-circuit operands. Only the shared evaluated schedule
    can establish that an authored invocation is absent; missing state metadata
    leaves the ordinary, fail-closed argument judgment in place.
*/
fn invocation_is_evaluated(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
) -> bool {
    if !call.authored_expression.is_valid() {
        return true;
    }
    let Some((machine, state)) = validation::machine_state_by_symbols(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return true;
    };
    let Some(statement) = program
        .statement_table
        .statement_at(state, call.statement_index)
    else {
        return true;
    };
    statement_steps(
        program,
        machine,
        state,
        call.statement_index,
        statement,
        &[],
        &facts.operators,
    )
    .iter()
    .any(|occurrence| occurrence.invocation == Some(Expression(call.authored_expression)))
}
