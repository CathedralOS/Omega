//! An expression statement: the state's return value shape and domain.

use super::{StatementOutputs, StatementScope};
use crate::value_custody::expression_types::ExpressionTypeOwner;
use crate::value_custody::expression_types::validate_expression_type_handle;
use crate::{
    machine_calls::calls, proof_contracts::arithmetic_domains, proof_contracts::domain_weakening,
    value_custody::expression_types, value_custody::struct_literals,
};
use diagnostics::Diagnostic;
use typed_trees::statement::StatementNode;

pub(super) fn validate(
    scope: &StatementScope<'_>,
    outputs: &mut StatementOutputs<'_>,
    statement: &StatementNode,
) {
    let StatementNode::Expression(expression) = statement else {
        unreachable!("dispatched expression_statements")
    };
    let StatementScope {
        program,
        machine,
        state_name,
        current_state,
        ..
    } = *scope;
    let value_environment = &mut *outputs.value_environment;
    let exact_integer_casts = &mut *outputs.exact_integer_casts;
    let diagnostics = &mut *outputs.diagnostics;
    let Some(state) = current_state else {
        return;
    };

    if calls::unit_statement_call_is_supported(program, machine, state, *expression) {
        // Value-position call validation already checked the callee and
        // its operands. Retain cast obligations inside those operands;
        // Unit itself has no scalar return range or landing.
        arithmetic_domains::collect_exact_integer_cast_facts(
            program,
            machine,
            Some(state),
            *expression,
            value_environment,
            exact_integer_casts,
        );
        return;
    }
    if !state.return_type.is_valid() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{state_name}` has a terminal expression but no return type",
            machine.name
        )));
        return;
    }

    let shape_before = diagnostics.len();
    struct_literals::validate_array_literal_elements(
        program,
        machine,
        state,
        *expression,
        state.return_type,
        diagnostics,
    );
    validate_expression_type_handle(
        program,
        *expression,
        state.return_type,
        diagnostics,
        ExpressionTypeOwner::StateTerminalExpression {
            machine: machine.name.as_str(),
            state: state_name,
        },
    );
    let owner = format!(
        "machine `{}` state `{state_name}` terminal expression",
        machine.name
    );
    let return_primitive = program.primitive_type_reference(state.return_type);
    // The shape gate above rejects a cross-class LITERAL terminal return
    // (`-> i32 { true }`) but blanket-accepts place/name values, so a bool
    // FIELD returned from an `-> i32` machine slips. Add the class check for
    // those -- only when the shape gate did not already error, so a literal
    // is not double-reported.
    if diagnostics.len() == shape_before {
        let slot_context = format!("machine `{}` state `{state_name}`", machine.name);
        if let Some(target_primitive) = return_primitive {
            expression_types::report_cross_class_store(
                program,
                Some(machine),
                Some(state),
                *expression,
                target_primitive,
                &slot_context,
                "return value",
                diagnostics,
            );
        }
        // Nominal guard: `-> Foo { self.bar }` returns a `Bar` as a `Foo`.
        expression_types::report_data_type_conflict(
            program,
            machine,
            Some(state),
            *expression,
            state.return_type,
            &slot_context,
            "return value",
            diagnostics,
        );
        // Shape guard: `-> i32 { self.xs }` returns an array as a scalar.
        expression_types::report_array_scalar_shape_mismatch(
            program,
            machine,
            Some(state),
            *expression,
            state.return_type,
            &slot_context,
            "return value",
            diagnostics,
        );
        expression_types::report_scalar_data_shape_mismatch(
            program,
            machine,
            Some(state),
            *expression,
            state.return_type,
            &slot_context,
            "return value",
            diagnostics,
        );
        domain_weakening::validate_implicit_domain_weakening(
            program,
            machine,
            Some(state),
            *expression,
            state.return_type,
            &slot_context,
            diagnostics,
        );
    }
    let before = diagnostics.len();
    let (return_interval, source_primitive) = arithmetic_domains::validate_anonymous_integer_range(
        program,
        state.return_type,
        *expression,
        &owner,
        diagnostics,
    )
    .unwrap_or_else(|| {
        arithmetic_domains::validate_value_range(
            program,
            machine,
            Some(state),
            *expression,
            value_environment,
            return_primitive,
            program.arithmetic_domain_for_type_reference(state.return_type),
            &owner,
            diagnostics,
        )
    });
    // A cleanly-analyzed return value that cannot fit the declared return
    // type is a silent narrowing (`-> i8 { 300 }`), same as a store.
    if diagnostics.len() == before {
        arithmetic_domains::check_narrowing_assignment(
            return_primitive,
            return_interval,
            source_primitive,
            &owner,
            diagnostics,
        );
    }
    // S4: enforce a declared return `[a..=b]` so call-site narrowing
    // that trusts it stays sound (the interval is already computed above).
    arithmetic_domains::enforce_symbolic_range(
        program,
        machine,
        Some(state),
        state.return_type,
        *expression,
        value_environment,
        &owner,
        diagnostics,
    );
    arithmetic_domains::enforce_declared_return_range(
        program,
        state.return_type,
        return_interval,
        &owner,
        diagnostics,
    );
    arithmetic_domains::collect_exact_integer_cast_facts(
        program,
        machine,
        Some(state),
        *expression,
        value_environment,
        exact_integer_casts,
    );
}
