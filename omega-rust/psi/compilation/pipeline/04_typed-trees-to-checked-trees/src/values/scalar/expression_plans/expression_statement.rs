//! A state's final expression: a returned value call's scalar arguments,
//! or the returned scalar itself (`Return`). Structural constructions retain
//! their operands through the shared value producer, not a return-only role.

use super::{
    CheckedScalarExpressionRole, ExpressionHandle, StatementPlanner, lower_return_expression,
    retain_expression_call_arguments,
};

pub(crate) fn plan(mut planner: StatementPlanner<'_, '_>, expression: ExpressionHandle) {
    let transfer = validation::current_entry_tail_call(
        planner.program,
        planner.machine,
        planner.state,
        planner.statement_index,
        expression,
    )
    .map(|call| {
        planner
            .program
            .expression_table
            .expression_handles(call.arguments)
    });
    if let Some(arguments) = transfer
        && let Some(entry) = planner.states.first()
    {
        super::transition::arguments_plan(planner.reborrow(), entry, arguments, false);
    }
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
    let unit_statement =
        validation::unit_statement_call_is_admitted(program, machine, state, expression);
    retain_expression_call_arguments(
        program,
        operators,
        state,
        statement_ordinal,
        expression,
        scalar_parameters,
        parameters,
        parameter_types,
        locals,
        exact_integer_casts,
        expressions,
        proof_terms,
        source_bindings,
        binding_symbols,
    );
    if transfer.is_none()
        && !unit_statement
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
        super::record_scalar_value_pair(
            source_bindings,
            expressions,
            binding_symbols,
            scalar_parameters,
            locals,
            state.symbol,
            statement_ordinal,
            CheckedScalarExpressionRole::Return,
            symbols::SymbolHandle::invalid(),
            expression,
            return_expression,
        );
    }
}
