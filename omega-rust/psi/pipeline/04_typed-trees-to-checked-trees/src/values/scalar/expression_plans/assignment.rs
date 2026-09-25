//! An assignment: an atomic carrier's authored operands, each index of the
//! target place (`AssignmentIndex`) and the stored value (`AssignmentValue`).

use super::{
    CheckedLocatedScalarExpression, CheckedScalarExpressionBindings, CheckedScalarExpressionRole,
    ExpressionNode, StatementPlanner, assignment_target_primitive_type, lower_call_arguments,
    lower_index_expression, lower_return_expression, retain_call_arguments,
    retain_subslice_endpoints, scalar_qualified_call_expression,
};

pub(super) fn plan(
    planner: StatementPlanner<'_, '_>,
    assignment: &typed_trees::statement::TableAssignment,
) {
    let StatementPlanner {
        program,
        operators,
        exact_integer_casts,
        machine,
        state,
        parameters,
        scalar_parameters,
        parameter_types,
        statement_index,
        statement_ordinal,
        locals,
        expressions,
        proof_terms,
        source_bindings,
        binding_symbols,
        ..
    } = planner;
    // An atomic carrier retains its authored operands,
    // never its arithmetic model (`atomic_operands.rs`).
    if let Some(operands) = super::super::atomic_operands::lower(
        program,
        operators,
        machine,
        state,
        assignment,
        scalar_parameters,
        parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    ) {
        retain_subslice_endpoints(
            operands,
            state.symbol,
            statement_ordinal,
            scalar_parameters,
            locals,
            expressions,
            source_bindings,
            binding_symbols,
        );
        return;
    }
    // Every indexed step of the target keeps its own
    // selector row, keyed by its depth from the target.
    for (depth, index) in validation::assignment_target_selectors(program, assignment.target)
        .into_iter()
        .enumerate()
    {
        let Ok(depth) = u32::try_from(depth) else {
            break;
        };
        let Some(expression) = lower_index_expression(
            program,
            operators,
            index,
            scalar_parameters,
            parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        ) else {
            continue;
        };
        source_bindings.append(CheckedScalarExpressionBindings {
            destination: symbols::SymbolHandle::invalid(),
            state: state.symbol,
            statement_ordinal,
            role: CheckedScalarExpressionRole::AssignmentIndex { depth },
            expression: index,
            symbols: binding_symbols.insert_many(
                scalar_parameters
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .chain(
                        locals
                            .iter()
                            .filter(|local| !local.is_mutable)
                            .map(|local| local.symbol),
                    ),
            ),
        });
        expressions.push(CheckedLocatedScalarExpression {
            state: state.symbol,
            statement_ordinal,
            role: CheckedScalarExpressionRole::AssignmentIndex { depth },
            expression,
        });
    }
    // A call delivering its result to an assignment target
    // needs the same scalar-argument custody rows as one
    // delivering it to a local: the arguments are evaluated
    // and transferred identically, and where the result
    // lands does not change their source bindings. The
    // qualified-call spelling below stays for the cast
    // chains only ordinary integer-returning machines have.
    if let Some(expression) =
        scalar_qualified_call_expression(program, assignment.value).or_else(|| {
            matches!(
                program.expression_table.expression(assignment.value),
                ExpressionNode::Call(_)
            )
            .then_some(assignment.value)
        })
        && let ExpressionNode::Call(call) = program.expression_table.expression(expression)
        && let Some(arguments) = lower_call_arguments(
            program,
            operators,
            state,
            statement_ordinal,
            0,
            &crate::semantic::calls::CallSite::Expression { expression, call },
            scalar_parameters,
            parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        )
    {
        retain_call_arguments(
            arguments,
            scalar_parameters,
            locals,
            expressions,
            proof_terms,
            source_bindings,
            binding_symbols,
        );
    }
    // Retain selected RHS meaning at the statement. A later
    // executable consumer still owns its admitted store shape.
    let Some(target_type_reference) = crate::flow::expression_type_reference_in_state(
        program,
        state.symbol,
        statement_index,
        assignment.target,
    ) else {
        return;
    };
    let Some(target_type) = assignment_target_primitive_type(program, target_type_reference) else {
        return;
    };
    let Some(expression) = lower_return_expression(
        program,
        operators,
        assignment.value,
        scalar_parameters,
        parameters,
        parameter_types,
        locals,
        target_type,
        exact_integer_casts,
    ) else {
        return;
    };
    source_bindings.append(CheckedScalarExpressionBindings {
        destination: match program.expression_table.expression(assignment.target) {
            ExpressionNode::Name(path) => path.symbol,
            _ => symbols::SymbolHandle::invalid(),
        },
        state: state.symbol,
        statement_ordinal,
        role: CheckedScalarExpressionRole::AssignmentValue,
        expression: assignment.value,
        symbols: binding_symbols.insert_many(
            scalar_parameters
                .iter()
                .map(|parameter| parameter.symbol)
                .chain(
                    locals
                        .iter()
                        .filter(|local| !local.is_mutable)
                        .map(|local| local.symbol),
                ),
        ),
    });
    expressions.push(CheckedLocatedScalarExpression {
        state: state.symbol,
        statement_ordinal,
        role: CheckedScalarExpressionRole::AssignmentValue,
        expression,
    });
}
