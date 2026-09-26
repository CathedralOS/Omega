//! A state's final expression: each scalar field of a returned case
//! literal (`ReturnCaseField`), a returned value call's scalar arguments,
//! or the returned scalar itself (`Return`).

use super::{
    CheckedLocatedScalarExpression, CheckedScalarExpressionBindings, CheckedScalarExpressionRole,
    ExpressionHandle, ExpressionNode, StatementPlanner, lower_call_arguments,
    lower_return_expression, retain_call_arguments,
};

pub(super) fn plan(planner: StatementPlanner<'_, '_>, expression: ExpressionHandle) {
    let StatementPlanner {
        program,
        operators,
        exact_integer_casts,
        machine,
        state,
        parameters,
        scalar_parameters,
        parameter_types,
        result_type,
        statement_ordinal,
        locals,
        expressions,
        proof_terms,
        source_bindings,
        binding_symbols,
        ..
    } = planner;
    if let ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(case_symbol) = literal.case_symbol
        && let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == literal.type_symbol)
        && let Some(typed_trees::data::DataMember::Variant(variant)) =
            program.data_members(data).iter().find(|member| {
                matches!(member, typed_trees::data::DataMember::Variant(variant) if variant.symbol == case_symbol)
            })
    {
        for (field_index, field) in program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .enumerate()
        {
            let Some(primitive_type) = program
                .data_payload_fields(variant)
                .iter()
                .find(|declaration| declaration.symbol == field.field_symbol)
                .and_then(|declaration| program.primitive_type_reference(declaration.type_reference))
            else {
                continue;
            };
            let Ok(field_ordinal) = u32::try_from(field_index) else {
                continue;
            };
            let Some(value) = lower_return_expression(
                program,
                operators,
                field.value,
                scalar_parameters,
                parameters,
                parameter_types,
                locals,
                primitive_type,
                exact_integer_casts,
            ) else {
                continue;
            };
            let role = CheckedScalarExpressionRole::ReturnCaseField { field_ordinal };
            source_bindings.append(CheckedScalarExpressionBindings {
                destination: symbols::SymbolHandle::invalid(),
                state: state.symbol,
                statement_ordinal,
                role,
                expression: field.value,
                symbols: binding_symbols.insert_many(
                    scalar_parameters.iter().map(|parameter| parameter.symbol)
                        .chain(locals.iter().filter(|local| !local.is_mutable).map(|local| local.symbol)),
                ),
            });
            expressions.push(CheckedLocatedScalarExpression {
                state: state.symbol,
                statement_ordinal,
                role,
                expression: value,
            });
        }
    }
    let unit_statement =
        validation::unit_statement_call_is_supported(program, machine, state, expression);
    if let ExpressionNode::Call(call) = program.expression_table.expression(expression)
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
    if !unit_statement
        && let Some(result_type) = result_type
        && let Some(return_expression) = lower_return_expression(
            program,
            operators,
            expression,
            scalar_parameters,
            parameters,
            parameter_types,
            locals,
            result_type,
            exact_integer_casts,
        )
    {
        source_bindings.append(CheckedScalarExpressionBindings {
            destination: symbols::SymbolHandle::invalid(),
            state: state.symbol,
            statement_ordinal,
            role: CheckedScalarExpressionRole::Return,
            expression,
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
            role: CheckedScalarExpressionRole::Return,
            expression: return_expression,
        });
    }
}
