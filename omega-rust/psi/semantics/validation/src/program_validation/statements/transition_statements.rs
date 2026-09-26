//! A transition statement: the target state or return value and the
//! environment carried into it.

use super::{StatementOutputs, StatementScope};
use crate::declarations::transitions::validate_transition_target_node;
use crate::{
    proof_contracts::arithmetic_domains, proof_contracts::domain_weakening,
    value_custody::expression_types, value_custody::struct_literals,
};
use typed_trees::statement::StatementNode;
use typed_trees::statement::TransitionTargetNode;

pub(super) fn validate(
    scope: &StatementScope<'_>,
    outputs: &mut StatementOutputs<'_>,
    statement: &StatementNode,
) {
    let StatementNode::Transition(transition) = statement else {
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
    let exact_integer_casts = &mut *outputs.exact_integer_casts;
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
        arithmetic_domains::validate_guard_ranges(
            program,
            machine,
            current_state,
            guard,
            value_environment,
            &owner,
            diagnostics,
        );
        arithmetic_domains::collect_exact_integer_cast_facts(
            program,
            machine,
            current_state,
            guard,
            value_environment,
            exact_integer_casts,
        );
    }
    validate_transition_target_node(
        program,
        machine,
        current_state,
        value_environment,
        transition_values.for_target(transition.target),
        transition.target,
        machine_symbols,
        symbols,
        writable_roots,
        diagnostics,
    );

    if transition.continuation.is_valid() {
        validate_transition_target_node(
            program,
            machine,
            current_state,
            value_environment,
            transition_values.for_target(transition.continuation),
            transition.continuation,
            machine_symbols,
            symbols,
            writable_roots,
            diagnostics,
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

    for target in [transition.target, transition.continuation] {
        if !target.is_valid() {
            continue;
        }
        match program.statement_table.transition_target(target) {
            TransitionTargetNode::Named { arguments, .. } => {
                for (argument_index, argument) in program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .enumerate()
                {
                    let argument_environment = transition_values
                        .for_target(target)
                        .get(argument_index)
                        .unwrap_or(&narrowed);
                    arithmetic_domains::collect_exact_integer_cast_facts(
                        program,
                        machine,
                        current_state,
                        *argument,
                        argument_environment,
                        exact_integer_casts,
                    );
                }
            }
            TransitionTargetNode::Value(expression) => {
                let return_environment = transition_values
                    .for_target(target)
                    .first()
                    .unwrap_or(&narrowed);
                arithmetic_domains::collect_exact_integer_cast_facts(
                    program,
                    machine,
                    current_state,
                    *expression,
                    return_environment,
                    exact_integer_casts,
                );
            }
            TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
        }
    }

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

    // A transition VALUE target (`_ -> (expr)`) is a return value. When the
    // state's return type declares a `[a..=b]`, enforce the value is provably
    // within it (so call-site narrowing that trusts the range is sound); the
    // exact-overflow + narrowing obligations apply too. Gated inside
    // `validate_return_value_range`, so plain returns are unaffected.
    if let Some(state) = current_state
        && state.return_type.is_valid()
    {
        for target in [transition.target, transition.continuation] {
            if !target.is_valid() {
                continue;
            }
            if let TransitionTargetNode::Value(return_expression) =
                program.statement_table.transition_target(target)
            {
                struct_literals::validate_array_literal_elements(
                    program,
                    machine,
                    state,
                    *return_expression,
                    state.return_type,
                    diagnostics,
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
                        &format!("machine `{}` state `{state_name}`", machine.name),
                        "return value",
                        diagnostics,
                    );
                }
                // Nominal guard: `-> Foo { transition { _ -> (self.bar) } }`
                // returns a `Bar` as a `Foo`.
                expression_types::report_data_type_conflict(
                    program,
                    machine,
                    Some(state),
                    *return_expression,
                    state.return_type,
                    &format!("machine `{}` state `{state_name}`", machine.name),
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
                    &format!("machine `{}` state `{state_name}`", machine.name),
                    "return value",
                    diagnostics,
                );
                expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    Some(state),
                    *return_expression,
                    state.return_type,
                    &format!("machine `{}` state `{state_name}`", machine.name),
                    "return value",
                    diagnostics,
                );
                domain_weakening::validate_implicit_domain_weakening(
                    program,
                    machine,
                    Some(state),
                    *return_expression,
                    state.return_type,
                    &format!("machine `{}` state `{state_name}`", machine.name),
                    diagnostics,
                );
                arithmetic_domains::validate_return_value_range(
                    program,
                    machine,
                    state,
                    *return_expression,
                    return_environment,
                    &format!(
                        "machine `{}` state `{state_name}` return value",
                        machine.name
                    ),
                    diagnostics,
                );
            }
        }
    }

    for target in [transition.target, transition.continuation] {
        if !target.is_valid() {
            continue;
        }
        if let TransitionTargetNode::Named { arguments, .. } =
            program.statement_table.transition_target(target)
        {
            for (argument_index, argument) in program
                .statement_table
                .expression_handles(*arguments)
                .iter()
                .enumerate()
            {
                let argument_environment = transition_values
                    .for_target(target)
                    .get(argument_index)
                    .unwrap_or(&narrowed);
                arithmetic_domains::validate_arithmetic_domains(
                    program,
                    machine,
                    current_state,
                    *argument,
                    argument_environment,
                    None,
                    numerics::arithmetic::ArithmeticDomain::Exact,
                    &format!(
                        "machine `{}` state `{state_name}` transition argument",
                        machine.name
                    ),
                    diagnostics,
                );
            }
        }
    }
}
