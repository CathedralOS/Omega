use checked_trees::{BorrowCallFact, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use crate::labels::{borrow_access_label, symbol_name};

use super::super::details::active_loan_detail;
use super::super::overlap::StatedOrderingPremise;
use super::super::overlap::canonical_place_loan_compatibility;
use super::super::place_access;
use super::super::simultaneous::{Hold, simultaneous_hold_conflict};

/// Compare one retained storage move with earlier operands still held by a
/// containing invocation. Exact expression occurrences, not affine-looking
/// actual types, distinguish a move from implicit shared lending. The source
/// schedule supplies branch compatibility; the call roster supplies loan
/// completion, including a nested call whose result retains no borrow.
pub(in crate::checks::borrows) fn check_staged_operand_move<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &CheckFacts,
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
                if super::staging::access_released_in_prefix(access, preceding, route)
                    // Earlier owned operands have already detached their
                    // values; they are not loans held over the caller's old
                    // storage. Repeated use is judged by move accounting.
                    || preceding.iter().any(|occurrence| {
                        let StatementStep::Move { event, ordered: true, .. } = occurrence.step else {
                            return false;
                        };
                        let previous = &facts.flow.ownership.storage_transfers
                            .span_or_empty(state.storage_transfers)[event];
                        previous.expression == access.expression
                            && routes_compatible(&occurrence.route, route)
                            && crate::checks::multiplicity::transfers_owned_storage(
                                program, state.state_symbol, statement.statement_index, previous,
                                facts.flow.ownership.segments.span_or_empty(previous.segments),
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
                // Resolve once, only when a prior staged loan needs comparison.
                // Calls with no such loan keep their ordinary source checks.
                let moved = match &moved_source {
                    Some(place) => place,
                    None => match super::super::moves::owned_storage_source(
                        program, facts, state, statement, event, path,
                    ) {
                        Ok(Some(source)) => moved_source.insert(source.place),
                        Ok(None) => return,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            return;
                        }
                    },
                };
                let backings = place_access::argument_backings(
                    program,
                    facts,
                    state,
                    call,
                    statement.entry_constraints,
                    access,
                );
                let Ok(backings) = backings else {
                    // The call's ordinary loan check reports missing backing.
                    continue;
                };
                if backings.places.iter().all(|backing| {
                    super::super::overlap::captured_place_compatibility(
                        program,
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
                    crate::labels::call_target_label(program, call.target_symbol),
                    validation::CanonicalPlace { root: event.root, segments: path.to_vec() }.label(program),
                )));
            }
        }
    }
}

pub(super) fn check_call_access_conflicts<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
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
    let data_access =
        super::receiver::explicit_jump_data_access(program, facts, state_flow, borrow_call);
    let exiting_source = super::super::statements::source_exiting_without_local_loan_carriers(
        program,
        state_flow,
        borrow_call,
        call_frames,
    );
    let boundary = super::staging::InvocationBoundary::at(program, facts, state_flow, borrow_call);
    let ended_before_call = super::super::reads::loans_ended_before_call(
        program,
        facts,
        state_flow,
        borrow_call,
        boundary.as_ref(),
    );
    let active_loans: Vec<_> = facts
        .flow
        .borrow_loan_constraints(entry_constraints)
        .filter(|loan| {
            if super::super::selected_retirement::before_call(
                program,
                facts,
                state_flow,
                borrow_call,
                *loan,
            ) {
                return false;
            }
            // The same replayed scope closure applies to forwarded argument
            // authority and to the successor's mutations. Ordinary calls,
            // carried loans and unclosed observations remain active.
            !ended_before_call.contains(loan)
                && !exiting_source.is_some_and(|source| {
                    super::super::statements::local_loan_ends_before_successor(
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
        .map(|loan| (loan, facts.borrow.loans.get(loan)))
        .collect();

    for transferred in
        crate::flow::owned_call_operand_places(program, facts, state_flow, borrow_call)
    {
        for (loan_handle, loan) in &active_loans {
            if canonical_place_loan_compatibility(
                program,
                &transferred,
                loan,
                &facts.borrow,
                stated_premises,
                bound_lookup,
            )
            .non_interfering
            {
                continue;
            }
            let detail =
                active_loan_detail(state_flow, facts, *loan_handle, borrow_call.statement_index);
            diagnostics.push(Diagnostic::error(format!(
                "state `{target_name}` receives an owned value while local borrow `{}` is still active{}",
                symbol_name(program, loan.owner_symbol),
                detail
                    .map(|detail| format!(" ({detail})"))
                    .unwrap_or_default(),
            )));
        }
    }

    // A local reference operand lends the storage its own loans captured, not
    // the local's name: judge each operand at every backing it may denote, as
    // a read or a write through the same alias is judged.
    let mut operand_backings = Vec::with_capacity(accesses.len());
    for access in &accesses {
        let backings = place_access::argument_backings(
            program,
            facts,
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

    let mut traced_loan_conflict = false;
    for (index, access) in accesses.iter().enumerate() {
        for (other_index, other_access) in accesses.iter().enumerate().skip(index + 1) {
            // A completed actual does not compete with the inputs that
            // produced it. Keep those inputs in the roster: a later sibling
            // must respect even an unselected declared lifetime dependency.
            if other_access
                .evaluation_start
                .is_some_and(|start| start <= index)
            {
                continue;
            }
            // A local jump forwards the activation's exact data parent. It
            // does not establish a second independent whole-record loan while
            // staging the other operands. The successor footprint judges each
            // retained child's lifetime; a completed copy holds no child loan.
            // Ordinary calls and pairs between other operands stay exclusive.
            if (data_access == Some(index) && other_access.end_before_access.is_none())
                || (data_access == Some(other_index) && access.end_before_access.is_none())
            {
                continue;
            }
            // A local jump stages scalar snapshots before transferring its
            // existing reference carriers. Reading through that same route
            // forms no second retained loan, regardless of the machine's
            // lexical qualification. Fresh borrows and returning invocations
            // still establish ordinary argument exclusivity.
            if super::staging::copied_read_before_reference_transfer(
                program,
                facts,
                state_flow,
                borrow_call,
                access,
                other_access,
            ) || super::staging::copied_read_before_reference_transfer(
                program,
                facts,
                state_flow,
                borrow_call,
                other_access,
                access,
            ) {
                continue;
            }
            // A copied read or completed nested-call loan no longer observes
            // its source when the later access begins. A payload its arm's
            // dispatch copied before the call (end 0) ended before the earlier
            // access began. Retained loans and the active local loans below
            // still constrain both.
            if access
                .end_before_access
                .is_some_and(|end| end <= other_index)
                || other_access
                    .end_before_access
                    .is_some_and(|end| end <= index)
            {
                continue;
            }
            // A completed call result continues its exact input's declared
            // referent loan. Both rows remain present against every sibling
            // operand and live loan; only this authenticated supplier/result
            // pair is one lineage rather than two simultaneous formations.
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
                // captured_place_compatibility admits shared pairs independently
                // of their pointer geometry. Authority checks remain separate.
                continue;
            }
            // The same authenticated frontier supplies ordinary live-loan
            // checking and this pair judgment. It is a conservative held region,
            // not an executable address recovered from a missing capture.
            let conflict = super::held_places::places(&operand_backings[index]).find_map(|first| {
                super::held_places::places(&operand_backings[other_index]).find_map(|second| {
                    simultaneous_hold_conflict(
                        program,
                        first,
                        &access.kind,
                        second,
                        &other_access.kind,
                        stated_premises,
                        bound_lookup,
                    )
                })
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
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
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

        // Earlier nested calls retain their own entry checks. Once their
        // borrow-free result completes, their direct input accesses are no
        // longer held by this later invocation. Stored child loans stay in
        // active_loans and still constrain every continuing access.
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

        for (loan_handle, loan) in &active_loans {
            // An owned carrier retains its backing with the transferred loan.
            // A caller-owned slot instead needs exact intermediate retention:
            // follow copies/replacements through successor slots, reject escape
            // and a later establishment of the still-borrowed local home.
            if super::super::statements::local_jump(program, state_flow, borrow_call).is_some()
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
                    let Some(jump) =
                        super::super::statements::local_jump(program, state_flow, borrow_call)
                    else {
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
            // Forwarding the activation's exact data parent is judged by
            // the successor's read/write footprint against the loan's captured
            // storage, just as a freshly authored projected argument is above.
            // A local carrier does not turn that same parent into a new whole
            // loan. Keep the carrier-lifetime check first: footprint precision
            // cannot extend backing storage beyond the source state's exit.
            // Returning calls establish every explicit argument loan. A
            // callee's field footprint cannot shrink a whole-record loan,
            // even when another argument forwards its borrowed child.
            if data_access == Some(index) {
                continue;
            }
            // Only the operand's own retained lineage is exempt; every other
            // live loan over a backing it may denote, such as a child view
            // formed through it, suspends that backing for the invocation.
            if !place_access::access_interferes(
                program,
                &facts.borrow,
                &operand_backings[index].places,
                &access.kind,
                *loan_handle,
                stated_premises,
                bound_lookup,
            ) {
                continue;
            }
            let detail =
                active_loan_detail(state_flow, facts, *loan_handle, borrow_call.statement_index);
            if !traced_loan_conflict && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                traced_loan_conflict = true;
                let access_path = facts.borrow.access_segments.span_or_empty(access.segments);
                let loan_path = facts.borrow.loan_segments(loan);
                let owner_path = facts.borrow.loan_owner_path(loan);
                let suppliers = facts
                    .borrow
                    .loan_suppliers
                    .span_or_empty(loan.supplying_loans);
                let backings = &operand_backings[index].places;
                eprintln!(
                    "CHECKSITE call-active-loan-conflict machine={:?} state={:?} statement={} ordinal={} target={:?} call_expression={:?} access_index={} access_expression={:?} access_root={:?} access_kind={:?} access_path_count={} access_path={:?} offending={:?} owner={:?} storage_owner={:?} root={:?} incoming={:?} formed_at={} last_use={} formation_expression={:?} established_by={:?} source_owner={:?} retained_operand={:?} lineage={:?} path_count={} path={:?} owner_path_count={} owner_path={:?} supplier_count={} suppliers={:?} backing_count={}",
                    state_flow.machine_symbol,
                    state_flow.state_symbol,
                    borrow_call.statement_index,
                    borrow_call.call_ordinal,
                    borrow_call.target_symbol,
                    borrow_call.authored_expression,
                    index,
                    access.expression,
                    access.root,
                    access.kind,
                    access_path.len(),
                    &access_path[..access_path.len().min(16)],
                    loan_handle,
                    loan.owner_symbol,
                    loan.storage_owner,
                    loan.root,
                    loan.incoming_reference,
                    loan.statement_index,
                    loan.last_use_statement_index,
                    loan.formation_expression,
                    loan.established_by_call,
                    loan.source_owner_symbol,
                    loan.retained_operand,
                    loan.lineage,
                    loan_path.len(),
                    &loan_path[..loan_path.len().min(16)],
                    owner_path.len(),
                    &owner_path[..owner_path.len().min(16)],
                    suppliers.len(),
                    &suppliers[..suppliers.len().min(4)],
                    backings.len(),
                );
                for (position, backing) in backings.iter().take(4).enumerate() {
                    eprintln!(
                        "CHECKSITE call-active-loan-backing position={} root={:?} incoming={:?} path_count={} path={:?} lineage_count={} lineage={:?}",
                        position,
                        backing.place.root,
                        backing.place.incoming_reference,
                        backing.place.segments.len(),
                        &backing.place.segments[..backing.place.segments.len().min(16)],
                        backing.lineage.len(),
                        &backing.lineage[..backing.lineage.len().min(4)],
                    );
                    for handle in backing.lineage.iter().take(4) {
                        if !facts.borrow.loans.is_valid(*handle) {
                            continue;
                        }
                        let source = facts.borrow.loans.get(*handle);
                        let path = facts.borrow.loan_segments(source);
                        let owner_path = facts.borrow.loan_owner_path(source);
                        let suppliers = facts
                            .borrow
                            .loan_suppliers
                            .span_or_empty(source.supplying_loans);
                        eprintln!(
                            "CHECKSITE call-active-loan-backing-source handle={:?} owner={:?} storage_owner={:?} root={:?} incoming={:?} formed_at={} last_use={} formation_expression={:?} established_by={:?} source_owner={:?} retained_operand={:?} lineage={:?} path_count={} path={:?} owner_path_count={} owner_path={:?} supplier_count={} suppliers={:?}",
                            handle,
                            source.owner_symbol,
                            source.storage_owner,
                            source.root,
                            source.incoming_reference,
                            source.statement_index,
                            source.last_use_statement_index,
                            source.formation_expression,
                            source.established_by_call,
                            source.source_owner_symbol,
                            source.retained_operand,
                            source.lineage,
                            path.len(),
                            &path[..path.len().min(16)],
                            owner_path.len(),
                            &owner_path[..owner_path.len().min(16)],
                            suppliers.len(),
                            &suppliers[..suppliers.len().min(4)],
                        );
                    }
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
