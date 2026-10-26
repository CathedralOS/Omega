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
mod held_places;
mod receiver;
mod staging;

pub(super) use receiver::{
    ReceiverSummaries, check_exclusive_place_use, check_receiver_footprint_against_argument_loans,
};

use self::conflicts::check_call_access_conflicts;
use crate::checks::ranges::incoming_guards::IncomingGuardIndex;
pub(super) use conflicts::check_staged_operand_move;
pub(super) use staging::{InvocationBoundary, access_released_in_prefix};

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
        let calls = facts.borrow.calls.span_or_empty(borrow_state.calls);
        if calls.is_empty() {
            continue;
        }
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
        for call in calls {
            if !invocation_is_evaluated(program, facts, state_flow, call) {
                continue;
            }
            let mut available_premises = premises.clone();
            super::overlap::append_statement_premises(
                program,
                facts,
                state_flow,
                call.statement_index,
                call_frames,
                &mut available_premises,
                bound_lookup,
            );
            let target_name = call_target_label(program, call.target_symbol);
            let entry_constraints = call_borrow_constraints(program, call, state_flow, facts);
            check_build_product_receiver(
                program,
                facts,
                state_flow,
                calls,
                call,
                &available_premises,
                diagnostics,
                bound_lookup,
            );
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

/// Source recording calls require exclusive Build use even though their host
/// preparation only observes identity. Judge the exact evaluated preparation
/// prefix, independently of whether the commit is a statement or a nested
/// expression. The emitted protocol never manufactures writes to Build fields.
fn check_build_product_receiver<'p>(
    program: &'p dyn validation::CheckingInput,
    facts: &CheckFacts,
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
    let Some(operation) = program
        .expression_table
        .call_node(commit.authored_expression)
    else {
        return;
    };
    if !operation.build_product_request.is_valid() || operation.target.as_str() == "prepare" {
        return;
    }
    let request = program
        .expression_table
        .build_product_request(operation.build_product_request);
    let arguments = program
        .expression_table
        .expression_handles(operation.arguments);
    let receiver_preparation = arguments.first().and_then(|expression| {
        let preparation = program.expression_table.call_node(*expression)?;
        let retained = program
            .expression_table
            .build_product_request(preparation.build_product_request)?;
        // Copies allocate independent request handles for each call. Their
        // source-issued payload, never arena-index equality, links the pair.
        if preparation.target.as_str() != "prepare" || Some(retained) != request {
            return None;
        }
        let receiver = program
            .expression_table
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
    // Call ordinals identify authored sites, not evaluation order. The exact
    // argument edge establishes that preparation precedes commit; the shared
    // evaluated schedule and captured entry constraints establish its frontier.
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

/// Borrow collection keeps stable authored call ordinals, including calls in
/// unselected closed short-circuit operands. Only the shared evaluated schedule
/// can establish that an authored invocation is absent; missing state metadata
/// leaves the ordinary, fail-closed argument judgment in place.
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
    crate::checks::ownership_occurrences::statement_steps(
        program,
        machine,
        state,
        call.statement_index,
        statement,
        &[],
        &facts.operators,
    )
    .iter()
    .any(|occurrence| {
        occurrence.invocation
            == Some(
                crate::checks::ownership_occurrences::InvocationIdentity::Expression(
                    call.authored_expression,
                ),
            )
    })
}
