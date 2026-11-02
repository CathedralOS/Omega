//! Expression statements distinguish evaluated Unit/Never work from the
//! actual state return. A preceding value expression cannot silently publish
//! or discard a non-Unit result; calls retain their separate explicit-use gate.

use super::{StatementOutputs, StatementScope};
use crate::{machine_calls::calls, proof_contracts::arithmetic_domains};
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
    let (program, machine, state_name, current_state, _, _) = scope.reads();
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

    // The terminal expression answers the declared result exactly as an
    // authored `_ -> (value)` arm does: both are completion values.
    super::validate_completion_value(
        scope,
        state,
        *expression,
        &mut arithmetic_decisions.integer_range_obligations,
        diagnostics,
    );
    let owner = format!(
        "machine `{}` state `{state_name}` terminal expression",
        machine.name
    );
    let return_primitive = program.primitive_type_reference(state.return_type);
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
