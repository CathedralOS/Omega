//! Expression statements distinguish evaluated Unit/Never work from the
//! actual state return. A preceding value expression cannot silently publish
//! or discard a non-Unit result; calls retain their separate explicit-use gate.

use super::{StatementOutputs, StatementScope};
use crate::value_custody::expression_types::ExpressionTypeOwner;
use crate::value_custody::expression_types::validate_expression_type_handle;
use crate::{
    machine_calls::calls, proof_contracts::arithmetic_domains, proof_contracts::domain_weakening,
    value_custody::expression_types, value_custody::struct_literals,
};
use diagnostics::Diagnostic;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceNode;

pub(super) fn validate(
    scope: &StatementScope<'_>,
    outputs: &mut StatementOutputs<'_>,
    statement: &StatementNode,
) {
    let Some(expression) = statement.as_expression() else {
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
    let arithmetic_decisions = &mut *outputs.arithmetic_decisions;
    let diagnostics = &mut *outputs.diagnostics;
    let Some(state) = current_state else {
        return;
    };

    if calls::unit_statement_call_is_admitted(program, machine, state, *expression) {
        // The call signature owns Unit's result. Its evaluated operands still
        // owe arithmetic formation, whose decisions leave this walk together.
        arithmetic_domains::validate_value_range_retaining_decisions(
            program,
            machine,
            Some(state),
            *expression,
            value_environment,
            None,
            numerics::arithmetic::ArithmeticDomain::Exact,
            &format!(
                "machine `{}` state `{state_name}` call operands",
                machine.name
            ),
            diagnostics,
            arithmetic_decisions,
        );
        return;
    }
    if scope.statement_index.checked_add(1)
        != Some(
            program
                .statement_table
                .statements(state.statement_nodes)
                .len(),
        )
    {
        let result_type =
            crate::expression_result_type_reference(program, machine, state, *expression);
        let permits_statement = result_type.is_some_and(|result| {
            let carrier = crate::value_custody::places::peel_declared_constraints(program, result);
            match program.type_reference_table.type_reference(carrier) {
                TypeReferenceNode::Unit => true,
                TypeReferenceNode::Named { symbol, .. } => {
                    program.symbols.builtin_type_atom(*symbol)
                        == Some(symbols::BuiltinTypeAtom::Never)
                }
                _ => false,
            }
        });
        if !permits_statement {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{state_name}` expression statement discards its non-Unit value; consume the value or discard a call explicitly with `_ = call(...);`",
                machine.name,
            )));
        }
        // Strict use and arithmetic formation are independent. Disposition
        // cannot waive the evaluated expression's partial-operation checks.
        let primitive = result_type.and_then(|result| program.primitive_type_reference(result));
        let domain = result_type.map_or(numerics::arithmetic::ArithmeticDomain::Exact, |result| {
            program.arithmetic_domain_for_type_reference(result)
        });
        arithmetic_domains::validate_value_range_retaining_decisions(
            program,
            machine,
            Some(state),
            *expression,
            value_environment,
            primitive,
            domain,
            &format!(
                "machine `{}` state `{state_name}` expression statement",
                machine.name
            ),
            diagnostics,
            arithmetic_decisions,
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
    crate::value_custody::reference_access::report_reference_access_conflict(
        program,
        machine,
        Some(state),
        &program.statement_table.statements(state.statement_nodes)[..scope.statement_index],
        *expression,
        state.return_type,
        &format!(
            "machine `{}` state `{state_name}` return value",
            machine.name
        ),
        diagnostics,
    );
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
        machine.symbol,
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
    let return_domain = program.arithmetic_domain_for_type_reference(state.return_type);
    let (return_interval, source_primitive, source_domain) =
        arithmetic_domains::validate_anonymous_integer_range(
            program,
            state.return_type,
            *expression,
            &owner,
            diagnostics,
        )
        .unwrap_or_else(|| {
            arithmetic_domains::validate_value_range_retaining_decisions(
                program,
                machine,
                Some(state),
                *expression,
                value_environment,
                return_primitive,
                return_domain,
                &owner,
                diagnostics,
                arithmetic_decisions,
            )
        });
    // A result retains its carrier even when its arithmetic is invalid.
    let (source_primitive, source_domain) = arithmetic_domains::stored_value_carrier(
        program,
        machine,
        Some(state),
        *expression,
        source_primitive,
        source_domain,
    );
    arithmetic_domains::check_narrowing_assignment(
        return_primitive,
        return_domain,
        return_interval,
        source_primitive,
        source_domain,
        &owner,
        diagnostics,
    );
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
}
