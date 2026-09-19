use checked_trees::{BorrowCallFact, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use crate::labels::call_target_label;

mod conflicts;
mod correspondence;
mod evidence;
mod receiver;
mod writability;

pub(super) use receiver::check_exclusive_place_use;

use self::conflicts::check_call_access_conflicts;
use self::evidence::CallCompatibility;
use self::writability::check_mutable_argument_writability;
use super::overlap::StatedOrderingPremise;
use crate::checks::ranges::incoming_guards::IncomingGuardIndex;

/// Initial construction is deliberately separate from replay: a checked
/// program with deleted evidence must not be mistaken for an unbuilt ledger.
pub(super) fn initialize_compatibility(program: &typed_trees::TypedTrees, facts: &mut CheckFacts) {
    let mut diagnostics = Vec::new();
    let call_frames = validation::CallFrameResolver::new(program);
    let incoming_guards = IncomingGuardIndex::build(program, call_frames.as_ref());
    let certificates = collect_compatibility(program, facts, &incoming_guards, &mut diagnostics);
    // Initial construction retains successful comparisons, not an admission.
    // The ordinary check pass repeats every obligation and aggregates failures
    // with statement/resource diagnostics before checked trees can be returned.
    facts
        .borrow
        .call_compatibility_certificates
        .reset_retain_capacity();
    facts
        .borrow
        .call_compatibility_certificates
        .insert_many(certificates);
}

pub(super) fn validate_compatibility(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    incoming_guards: &IncomingGuardIndex,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    let reconstructed = collect_compatibility(program, facts, incoming_guards, diagnostics);
    let mut retained_diagnostics = Vec::new();
    // Rebuild every invocation's comparisons in semantic order. Exact equality
    // verifies the full roster as well as each frozen selector and premise:
    // missing, extra, reordered or retargeted rows cannot pass independently.
    if !facts
        .borrow
        .call_compatibility_certificates
        .iter()
        .map(|(_, certificate)| certificate)
        .eq(reconstructed.iter())
    {
        retained_diagnostics.push(Diagnostic::error(
            "checked borrow call compatibility ledger drifted from exact invocation replay",
        ));
    }
    retained_diagnostics
}

fn collect_compatibility(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    incoming_guards: &IncomingGuardIndex,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<checked_trees::CheckedBorrowCallCompatibilityCertificate> {
    let mut certificates = Vec::new();
    if !correspondence::matches_source(program, facts) {
        diagnostics.push(Diagnostic::error(
            "checked borrow call roster drifted from typed source or state-owned flow",
        ));
        return certificates;
    }
    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(borrow_state) = super::matching_borrow_state(facts, state_flow) else {
            continue;
        };
        if !correspondence::matches_entry_loans(program, facts, state_flow) {
            diagnostics.push(Diagnostic::error(
                "checked borrow call entry loans drifted from source lifecycle",
            ));
            continue;
        }
        let premises = crate::semantic_calls::find_state_in_machine(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        )
        .and_then(|state| {
            crate::lookup::machine_by_symbol(program, state_flow.machine_symbol)
                .map(|machine| (machine, state))
        })
        .map(|(machine, state)| {
            super::overlap::stated_ordering_premises(
                program,
                facts,
                machine,
                state,
                incoming_guards,
            )
        })
        .unwrap_or_default();
        for (ordinal, call) in (0..borrow_state.calls.count())
            .zip(facts.borrow.calls.span_or_empty(borrow_state.calls))
        {
            let Some(call_index) = borrow_state
                .calls
                .start()
                .arena_index()
                .checked_add(ordinal)
            else {
                diagnostics.push(Diagnostic::error("borrow call span exceeds its arena"));
                continue;
            };
            let mut recording = CallCompatibility {
                state: state_flow,
                handle: arena::Handle::from_parts(
                    call_index,
                    borrow_state.calls.start().generation(),
                ),
                call,
                certificates: &mut certificates,
            };
            check_call_borrows(
                program,
                facts,
                state_flow,
                call,
                &premises,
                diagnostics,
                &mut recording,
            );
        }
    }
    certificates
}

fn check_call_borrows(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_call: &BorrowCallFact,
    stated_premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    recording: &mut CallCompatibility<'_>,
) {
    let target_name = call_target_label(program, borrow_call.target_symbol);
    let entry_constraints = call_borrow_constraints(borrow_call, state_flow, facts);
    check_call_access_conflicts(
        program,
        facts,
        state_flow,
        borrow_call,
        entry_constraints,
        &target_name,
        stated_premises,
        diagnostics,
        recording,
    );
    receiver::check_receiver_conflicts(
        program,
        facts,
        state_flow,
        borrow_call,
        entry_constraints,
        &target_name,
        stated_premises,
        diagnostics,
        recording,
    );

    check_mutable_argument_writability(
        program,
        facts,
        state_flow,
        borrow_call,
        entry_constraints,
        &target_name,
        diagnostics,
    );
}

fn call_borrow_constraints<'a>(
    borrow_call: &BorrowCallFact,
    state_flow: &'a FlowStateFact,
    facts: &'a CheckFacts,
) -> arena::HandleSpan<checked_trees::FlowConstraintRef> {
    facts.flow.state_call_entry_constraints(
        state_flow,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
        borrow_call.target_symbol,
        borrow_call.receiver_symbol,
    )
}
