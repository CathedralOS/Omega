use checked_trees::{BorrowCallFact, CapturedPlace, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use crate::labels::{borrow_access_label, symbol_name};

use super::super::details::active_loan_detail;
use super::super::overlap::StatedOrderingPremise;
use super::super::overlap::{canonical_place_loan_compatibility, captured_place_compatibility};
use super::super::simultaneous::{Hold, simultaneous_hold_conflict};

pub(super) fn check_call_access_conflicts<'p>(
    program: &'p typed_trees::TypedTrees,
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
    );
    let active_loans: Vec<_> = facts
        .flow
        .borrow_loan_constraints(entry_constraints)
        .filter(|loan| {
            // The same replayed scope closure applies to forwarded argument
            // authority and to the successor's mutations. Ordinary calls,
            // carried loans and unclosed observations remain active.
            !exiting_source.is_some_and(|source| {
                super::super::statements::local_loan_ends_before_successor(
                    program, facts, state_flow, source, *loan,
                )
            })
        })
        .map(|loan| (loan, facts.borrow.loans.get(loan)))
        .collect();

    for transferred in crate::flow::owned_call_operand_places(
        program,
        &facts.operators,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        borrow_call,
    ) {
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

    for (index, access) in accesses.iter().enumerate() {
        for (other_index, other_access) in accesses.iter().enumerate().skip(index + 1) {
            // A local jump forwards the activation's exact data parent. It
            // does not establish a second independent whole-record loan while
            // staging the other operands. The successor footprint judges each
            // retained child's lifetime; a completed copy holds no child loan.
            // Ordinary calls and pairs between other operands stay exclusive.
            if data_access == Some(index) || data_access == Some(other_index) {
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
            let Some(conflict) = simultaneous_hold_conflict(
                program,
                &CapturedPlace {
                    root_symbol: access.root_symbol,
                    segments: facts.borrow.access_segments(access).to_vec(),
                },
                &access.kind,
                &CapturedPlace {
                    root_symbol: other_access.root_symbol,
                    segments: facts.borrow.access_segments(other_access).to_vec(),
                },
                &other_access.kind,
                stated_premises,
                bound_lookup,
            ) else {
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
            diagnostics.push(Diagnostic::error(format!(
                "state `{target_name}` receives {}",
                conflict.phrase(label, "another argument in the same call"),
            )));
        }

        for (loan_handle, loan) in &active_loans {
            // An owned carrier retains its backing with the transferred loan.
            // A caller-owned slot instead needs exact intermediate retention:
            // follow copies/replacements through successor slots, reject escape
            // and a later establishment of the still-borrowed local home.
            if super::super::statements::local_jump(program, state_flow, borrow_call).is_some()
                && access.end_before_access.is_none()
                && access.root_symbol == loan.owner_symbol
                && crate::borrow::owner_path_overlaps_place_segments(
                    program,
                    facts.borrow.loan_owner_path(loan),
                    facts.borrow.access_segments.span_or_empty(access.segments),
                )
                && program.symbols.get(loan.root_symbol).kind == symbols::SymbolKind::Local
                && program.symbols.get(loan.root_symbol).parent == state_flow.state_symbol
                && !crate::flow::loan_carrier_has_owned_storage(
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
                    let Some(machine) =
                        crate::lookup::machine_by_symbol(program, state_flow.machine_symbol)
                    else {
                        return false;
                    };
                    let Some(source) = program
                        .machine_states(machine)
                        .iter()
                        .find(|state| state.symbol == state_flow.state_symbol)
                    else {
                        return false;
                    };
                    frames.local_backing_survives_state_jump(
                        machine,
                        source,
                        borrow_call.statement_index,
                        jump.target,
                        crate::semantic::calls::call_site_argument_expressions(program, &jump.site),
                        loan.root_symbol,
                    )
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives a carried loan of local `{}`, whose storage ends at the source state exit without a complete retained-home relation",
                    symbol_name(program, loan.root_symbol),
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
            if captured_place_compatibility(
                program,
                &CapturedPlace {
                    root_symbol: access.root_symbol,
                    segments: facts.borrow.access_segments(access).to_vec(),
                },
                &access.kind,
                &CapturedPlace {
                    root_symbol: loan.root_symbol,
                    segments: facts.borrow.loan_segments(loan).to_vec(),
                },
                &loan.kind,
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
