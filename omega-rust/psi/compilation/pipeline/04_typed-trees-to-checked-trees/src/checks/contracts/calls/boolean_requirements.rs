//! Prove a Boolean arrival requirement at one exact call occurrence.
//!
//! Each conjunct may use a different existing proof route. No child is installed
//! as a premise: all queries keep the same substituted operands and live context.
use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};

use super::super::places::expression_is_boolean_place_like;
use super::super::prover::{
    call_entry_contexts_prove_boolean_contract_expression, closed_boolean_value,
};
use super::{incoming_guard_proves_requires, transition_guard_proves_boolean_requirement};

/// A collection-predicate requirement over a field of the caller's argument
/// place is the same fact the place's data states in `where`; it stands at
/// every call because calls are consumption points for invariant windows.
fn standing_domain_fact_proves_requirement(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    expression: ExpressionHandle,
) -> bool {
    let Some(caller) = program.machine_by_symbol(state_flow.machine_symbol) else {
        return false;
    };
    let Some(caller_state) = program.state_in_machine(caller, state_flow.state_symbol) else {
        return false;
    };
    let Some(call_site) = validation::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    let Some(parameters) = validation::call_target_parameters(program, call_flow.target_symbol)
    else {
        return false;
    };
    let Some(callee) = program
        .machine_by_symbol(call_flow.target_symbol)
        .or_else(|| {
            validation::find_machine_by_entry_state(program, call_flow.target_symbol)
                .map(|(machine, _)| machine)
        })
    else {
        return false;
    };
    validation::standing_facts_prove_requirement(
        program,
        caller,
        caller_state,
        callee.symbol,
        parameters,
        validation::call_site_argument_expressions(program, &call_site),
        expression,
    )
}

/// A `.len` over declared fixed storage is a closed value in the caller's own
/// scope: `64 <= main.control.len` on a `[u8; 64]` field or a `&mut` loan of
/// it decides without a carried premise. Anything else — a runtime-length
/// slice, a non-length member, or no literal side — stays undecided and falls
/// through to the bound, guard, and entailment channels unchanged.
fn constant_extent_requirement_value(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    expression: ExpressionHandle,
) -> Option<bool> {
    let machine = program.machine_by_symbol(state_flow.machine_symbol)?;
    let state = validation::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    )?;
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    super::super::call_bounds::constant_extent_comparison_value(program, machine, state, binary)
}

/// A call binding every parameter positionally substitutes the subject
/// argument too (`Main::fill(main, 2, node)`): the instantiated
/// judgment — the only channel that substitutes `self` — then decides
/// the requires on the bound values alone. Covers every fully-bound
/// call site, any callee state, guarded or not; receiver-form calls
/// bind `self` implicitly and never reach full arity here.
fn explicit_subject_call_requirement_proven(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    expression: ExpressionHandle,
) -> bool {
    let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
        return false;
    };
    let Some(state) = validation::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return false;
    };
    let Some(callee_parameters) =
        validation::call_target_parameters(program, call_flow.target_symbol)
    else {
        return false;
    };
    let Some(site) = validation::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    let arguments = validation::call_site_argument_expressions(program, &site);
    if arguments.len() != callee_parameters.len() {
        return false;
    }
    validation::instantiated_value_call_requires_proven(
        program,
        machine,
        state,
        callee_parameters,
        &arguments,
        expression,
    )
}

fn closed_selected_float_requirement(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    expression: ExpressionHandle,
) -> Option<bool> {
    let (machine, state) = validation::find_state_with_machine(program, call_flow.target_symbol)
        .or_else(|| {
            let machine = program.machine_by_symbol(call_flow.target_symbol)?;
            Some((machine, program.machine_states(machine).first()?))
        })?;
    let parameters = validation::call_target_parameters(program, call_flow.target_symbol)?;
    let arguments = super::super::call_values::captured_argument_expressions(
        program, facts, state_flow, call_flow,
    )?;
    crate::checks::operators::closed_float_boolean_requirement(
        program,
        facts,
        machine.symbol,
        state.symbol,
        expression,
        parameters,
        &arguments,
    )
}

pub(super) fn proves(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[facts::FactContextHandle],
    expression: ExpressionHandle,
    incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let proven =
        closed_selected_float_requirement(program, facts, state_flow, call_flow, expression)
            == Some(true)
            || closed_boolean_value(program, &facts.operators, expression) == Some(true)
            || constant_extent_requirement_value(program, state_flow, expression) == Some(true)
            || super::super::call_bounds::proves(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                expression,
                None,
            )
            || call_entry_contexts_prove_boolean_contract_expression(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                expression,
                call_frames,
            )
            || super::super::call_bounds::proves_in_context(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                expression,
                call_frames,
            )
            || super::super::entailment::structural_call_requirement(
                program,
                facts,
                state_flow,
                call_flow,
                expression,
                call_frames,
            )
            || explicit_subject_call_requirement_proven(program, state_flow, call_flow, expression)
            || if !expression_is_boolean_place_like(program, expression) {
                // R1: a DOMINATING incoming-arm guard establishes a
                // boolean requires fact -- the ranges machinery's
                // IncomingGuard walk-back is the soundness gate
                // (single-predecessor edges, rewrite-fenced across
                // intermediate states), and the caller state's OWN
                // statements must preserve the named fields up to
                // the call (conservative whole-state scan).
                incoming_guard_proves_requires(
                    program,
                    facts,
                    state_flow,
                    call_flow,
                    expression,
                    incoming_guards,
                    call_frames,
                )
            } else {
                // The actual-aware proof above already checked
                // this formal. A raw callee name is not a caller
                // fact, even when both parameters spell `flag`.
                false
            }
            || transition_guard_proves_boolean_requirement(
                program,
                facts,
                state_flow,
                call_flow,
                expression,
                call_frames,
            )
            || standing_domain_fact_proves_requirement(program, state_flow, call_flow, expression);
    if proven {
        return true;
    }
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    // Preserve selected operator meanings. Splitting a builtin conjunction
    // combines verdicts, never assumptions, and cannot turn a disjunction into
    // independently available premises.
    binary.operator == BinaryOperator::And
        && super::super::prover::has_builtin_operators(program, &facts.operators, expression)
        && [binary.left, binary.right].into_iter().all(|conjunct| {
            proves(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                conjunct,
                incoming_guards,
                call_frames,
            )
        })
}
