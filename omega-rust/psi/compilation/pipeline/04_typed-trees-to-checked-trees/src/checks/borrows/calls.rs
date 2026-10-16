//! Call arguments are checked once against each other and the active loans.
//! Flow supplies the left-to-right access lifetimes; this pass judges overlap
//! over the original borrow facts. Source validation already judges reference
//! formation against the exact receiving leaf, including every enclosing
//! reference's access. Rechecking only a captured place's root here loses that
//! authority: an immutable owned aggregate can contain an exclusive descriptor.
//! An operand named through a local reference is judged against the active loans
//! at the storage that reference's loans captured, by the same place-access
//! judgment as a read or a write through it.

use checked_trees::{BorrowCallFact, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use crate::labels::call_target_label;

mod conflicts;
mod receiver;
mod staging;

pub(super) use receiver::{
    ReceiverSummaries, check_exclusive_place_use, check_receiver_footprint_against_argument_loans,
};

use self::conflicts::check_call_access_conflicts;
use crate::checks::ranges::incoming_guards::IncomingGuardIndex;
pub(super) use conflicts::check_staged_operand_move;

pub(super) fn check_compatibility<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    incoming_guards: &IncomingGuardIndex,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(borrow_state) = super::matching_borrow_state(facts, state_flow) else {
            continue;
        };
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
                bound_lookup,
            )
        })
        .unwrap_or_default();
        for call in facts.borrow.calls.span_or_empty(borrow_state.calls) {
            let mut available_premises = premises.clone();
            super::overlap::append_call_premises(
                program,
                facts,
                state_flow,
                call.statement_index,
                call_frames,
                &mut available_premises,
                bound_lookup,
            );
            let target_name = call_target_label(program, call.target_symbol);
            let entry_constraints = call_borrow_constraints(call, state_flow, facts);
            check_call_access_conflicts(
                program,
                facts,
                state_flow,
                call,
                entry_constraints,
                &target_name,
                &available_premises,
                call_frames,
                diagnostics,
                bound_lookup,
            );
        }
    }
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
