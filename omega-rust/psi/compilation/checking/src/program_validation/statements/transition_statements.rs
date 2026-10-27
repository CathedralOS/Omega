//! A transition statement: the target state or return value and the
//! environment carried into it.

use super::{StatementOutputs, StatementScope};
use crate::declarations::transitions::validate_transition_target_node;
use crate::{
    proof_contracts::arithmetic_domains, proof_contracts::domain_weakening,
    value_custody::expression_types, value_custody::struct_literals,
};
use diagnostics::Diagnostic;
use typed_trees::statement::StatementNode;
use typed_trees::statement::TransitionTargetNode;

pub(super) fn validate(
    scope: &StatementScope<'_>,
    outputs: &mut StatementOutputs<'_>,
    statement: &StatementNode,
) {
    let Some(transition) = statement.as_transition() else {
        unreachable!("dispatched transition_statements")
    };
    let StatementScope {
        program,
        machine,
        state_name,
        current_state,
        machine_symbols,
        symbols,
        writable_roots,
        transition_values,
        ..
    } = *scope;
    let value_environment = &mut *outputs.value_environment;
    let arithmetic_decisions = &mut *outputs.arithmetic_decisions;
    let diagnostics = &mut *outputs.diagnostics;
    if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
        // Every Exact operation in a guard owes its representability proof,
        // as it would in a `let` or a return, under the facts its evaluation
        // order establishes. Without this, `a + b == 70` over unconstrained
        // operands passed as a guard and lowering later found no proof.
        let owner = format!(
            "machine `{}` state `{state_name}` transition guard",
            machine.name
        );
        arithmetic_domains::validate_value_range_retaining_decisions(
            program,
            machine,
            current_state,
            guard,
            value_environment,
            Some(typed_trees::types::PrimitiveType::Bool),
            numerics::arithmetic::ArithmeticDomain::Exact,
            &owner,
            diagnostics,
            arithmetic_decisions,
        );
    }
    validate_transition_target_node(
        program,
        machine,
        current_state,
        value_environment,
        transition_values.for_target(transition.target),
        transition.target,
        writable_roots,
        diagnostics,
        &mut arithmetic_decisions.integer_range_obligations,
    );

    if transition.continuation.is_valid() {
        validate_transition_target_node(
            program,
            machine,
            current_state,
            value_environment,
            transition_values.for_target(transition.continuation),
            transition.continuation,
            writable_roots,
            diagnostics,
            &mut arithmetic_decisions.integer_range_obligations,
        );
    }

    // Return and argument expressions use their collected target-local
    // premises: the primary arm assumes the guard, the continuation
    // assumes its complement, and each crosses only its own effects.
    // Keep the ordinary guard environment for targets without values.
    let narrowed = arithmetic_domains::guard_narrowed_environment(
        program,
        machine,
        current_state,
        &transition.guard,
        value_environment,
    );

    // Fall-through complement: an exit-if-true transition (valid
    // target, no `_` arm) leaves the guard REFUTED for every later
    // statement in this state -- the MR2 terminal-tail shape's
    // `n - 1` after `transition n == 0 { true -> exit }`.
    if transition.target.is_valid() && !transition.continuation.is_valid() {
        *value_environment = arithmetic_domains::fall_through_narrowed_environment(
            program,
            machine,
            current_state,
            &transition.guard,
            value_environment,
        );
    }

    // A terminal call forwards the same logical result as an ordinary call.
    // A Unit call can complete a Unit state; a scalar/structural value still
    // cannot escape through a state that declares no such result.
    if let Some(state) = current_state
        && !state.return_type.is_valid()
    {
        for target in transition.targets() {
            if let TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(target)
                && !crate::machine_calls::calls::unit_statement_call_is_admitted(
                    program,
                    machine,
                    state,
                    *expression,
                )
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{state_name}` transition returns a value, but the state declares no result; a terminal arm is `_ -> {{}}`",
                    machine.name
                )));
            }
        }
    }

    // When the state's return type declares a `[a..=b]`, enforce the value is
    // provably within it (so call-site narrowing that trusts the range is
    // sound); the exact-overflow + narrowing obligations apply too. Gated
    // inside `validate_return_value_range`, so plain returns are unaffected.
    if let Some(state) = current_state
        && state.return_type.is_valid()
    {
        let mut return_owner = None;
        let mut slot_context = None;
        for target in transition.targets() {
            if let TransitionTargetNode::Value(return_expression) =
                program.statement_table.transition_target(target)
            {
                let return_owner = return_owner.get_or_insert_with(|| {
                    format!(
                        "machine `{}` state `{state_name}` return value",
                        machine.name
                    )
                });
                let slot_context = slot_context.get_or_insert_with(|| {
                    format!("machine `{}` state `{state_name}`", machine.name)
                });
                crate::value_custody::reference_access::report_reference_access_conflict(
                    program,
                    machine,
                    Some(state),
                    &program.statement_table.statements(state.statement_nodes)
                        [..scope.statement_index],
                    *return_expression,
                    state.return_type,
                    return_owner,
                    diagnostics,
                );
                struct_literals::validate_array_literal_elements(
                    program,
                    machine,
                    state,
                    *return_expression,
                    state.return_type,
                    diagnostics,
                    &mut arithmetic_decisions.integer_range_obligations,
                );
                let return_environment = transition_values
                    .for_target(target)
                    .first()
                    .unwrap_or(&narrowed);
                // Cross-class guard: `_ -> (true)` returning a bool from an
                // i32-returning machine is a silent miscompile. The terminal
                // `{ true }` return form is caught by the general shape gate;
                // the transition-VALUE form was not. Class complement of the
                // range/overflow/narrowing check below.
                if let Some(return_primitive) = program.primitive_type_reference(state.return_type)
                {
                    expression_types::report_cross_class_store(
                        program,
                        Some(machine),
                        Some(state),
                        *return_expression,
                        return_primitive,
                        slot_context,
                        "return value",
                        diagnostics,
                    );
                }
                expression_types::report_delivered_type_mismatch(
                    program,
                    machine.symbol,
                    *return_expression,
                    state.return_type,
                    return_owner,
                    diagnostics,
                );
                // Nominal guard: `-> Foo { transition { _ -> (self.bar) } }`
                // returns a `Bar` as a `Foo`.
                expression_types::report_data_type_conflict(
                    program,
                    machine,
                    Some(state),
                    *return_expression,
                    state.return_type,
                    slot_context,
                    "return value",
                    diagnostics,
                );
                // Shape guard: an array returned as a scalar (or vice versa).
                expression_types::report_array_scalar_shape_mismatch(
                    program,
                    machine,
                    Some(state),
                    *return_expression,
                    state.return_type,
                    slot_context,
                    "return value",
                    diagnostics,
                );
                expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    Some(state),
                    *return_expression,
                    state.return_type,
                    slot_context,
                    "return value",
                    diagnostics,
                );
                domain_weakening::validate_return_domain_weakening(
                    program,
                    machine,
                    state,
                    *return_expression,
                    slot_context,
                    diagnostics,
                );
                arithmetic_domains::validate_return_value_range(
                    program,
                    machine,
                    state,
                    *return_expression,
                    return_environment,
                    return_owner,
                    diagnostics,
                    arithmetic_decisions,
                );
            }
        }
    }

    let mut argument_owner = None;
    for target in transition.targets() {
        if let TransitionTargetNode::Named { arguments, .. } =
            program.statement_table.transition_target(target)
        {
            let arguments = program.statement_table.expression_handles(*arguments);
            if !arguments.is_empty() {
                let argument_owner = argument_owner.get_or_insert_with(|| {
                    format!(
                        "machine `{}` state `{state_name}` transition argument",
                        machine.name
                    )
                });
                for (argument_index, argument) in arguments.iter().enumerate() {
                    let argument_environment = transition_values
                        .for_target(target)
                        .get(argument_index)
                        .unwrap_or(&narrowed);
                    arithmetic_domains::validate_value_range_retaining_decisions(
                        program,
                        machine,
                        current_state,
                        *argument,
                        argument_environment,
                        None,
                        numerics::arithmetic::ArithmeticDomain::Exact,
                        argument_owner,
                        diagnostics,
                        arithmetic_decisions,
                    );
                }
            }
        }
    }
}
