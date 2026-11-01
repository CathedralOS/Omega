//! A transition statement: the target state or return value and the
//! environment carried into it.

use super::{StatementOutputs, StatementScope};
use crate::declarations::transitions::validate_transition_target_node;
use crate::proof_contracts::arithmetic_domains;
use diagnostics::Diagnostic;
use typed_trees::statement::StatementNode;
use typed_trees::statement::TransitionGuardNode;
use typed_trees::statement::TransitionTargetNode;

pub(super) fn validate(
    scope: &StatementScope<'_>,
    outputs: &mut StatementOutputs<'_>,
    statement: &StatementNode,
) {
    let Some(transition) = statement.as_transition() else {
        unreachable!("dispatched transition_statements")
    };
    let (program, machine, state_name, current_state, _symbols, writable_roots) = scope.reads();
    let transition_values = scope.transition_values;
    let value_environment = &mut *outputs.value_environment;
    let arithmetic_decisions = &mut *outputs.arithmetic_decisions;
    let diagnostics = &mut *outputs.diagnostics;
    if let TransitionGuardNode::When(guard) = transition.guard {
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
    let guard_is_always = matches!(transition.guard, TransitionGuardNode::Always);
    let narrowed_owned = (!guard_is_always).then(|| {
        arithmetic_domains::guard_narrowed_environment(
            program,
            machine,
            current_state,
            &transition.guard,
            value_environment,
        )
    });

    // Fall-through complement: an exit-if-true transition (valid
    // target, no `_` arm) leaves the guard REFUTED for every later
    // statement in this state -- the MR2 terminal-tail shape's
    // `n - 1` after `transition n == 0 { true -> exit }`.
    if transition.target.is_valid() && !transition.continuation.is_valid() && !guard_is_always {
        *value_environment = arithmetic_domains::fall_through_narrowed_environment(
            program,
            machine,
            current_state,
            &transition.guard,
            value_environment,
        );
    }
    let narrowed = narrowed_owned.as_ref().unwrap_or(&*value_environment);

    // A terminal call forwards the same logical result as an ordinary call.
    // A Unit call can complete a Unit state; a scalar/structural value still
    // cannot escape through a state that declares no such result.
    if let Some(state) = current_state
        && !state.return_type.is_valid()
    {
        for target in transition.targets() {
            if let TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(target)
            {
                if !crate::machine_calls::calls::unit_statement_call_is_admitted(
                    program,
                    machine,
                    state,
                    *expression,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{state_name}` transition returns a value, but the state declares no result; a terminal arm is `_ -> {{}}`",
                        machine.name
                    )));
                    continue;
                }
                // Forwarding Unit still evaluates every call operand. Retain
                // their formation decisions under this exact arm's facts, as
                // for an ordinary Unit expression call or a typed return.
                let environment = transition_values
                    .for_target(target)
                    .first()
                    .unwrap_or(&narrowed);
                arithmetic_domains::validate_value_range_retaining_decisions(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    environment,
                    None,
                    numerics::arithmetic::ArithmeticDomain::Exact,
                    &format!(
                        "machine `{}` state `{state_name}` terminal call operands",
                        machine.name
                    ),
                    diagnostics,
                    arithmetic_decisions,
                );
            }
        }
    }

    // Every value arm answers the declared result through the completion-value
    // checks. When the return type declares a `[a..=b]`, the value must also be
    // provably within it (so call-site narrowing that trusts the range is
    // sound); the exact-overflow + narrowing obligations apply too. Gated
    // inside `validate_return_value_range`, so plain returns are unaffected.
    if let Some(state) = current_state
        && state.return_type.is_valid()
    {
        let mut return_owner = None;
        for target in transition.targets() {
            if let TransitionTargetNode::Value(return_expression) =
                program.statement_table.transition_target(target)
            {
                super::validate_completion_value(
                    scope,
                    state,
                    *return_expression,
                    &mut arithmetic_decisions.integer_range_obligations,
                    diagnostics,
                );
                let return_owner = return_owner.get_or_insert_with(|| {
                    format!(
                        "machine `{}` state `{state_name}` return value",
                        machine.name
                    )
                });
                let return_environment = transition_values
                    .for_target(target)
                    .first()
                    .unwrap_or(&narrowed);
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
