//! A state's final expression: a returned value call's scalar arguments,
//! or the returned scalar itself (`Return`). Structural constructions retain
//! their operands through the shared value producer, not a return-only role.

use super::{
    CheckedScalarExpressionRole, ExpressionHandle, StatementPlanner, lower_return_expression,
};

pub(crate) fn plan(planner: &mut StatementPlanner<'_, '_>, expression: ExpressionHandle) {
    let unit_statement = validation::unit_statement_call_is_admitted(
        planner.program,
        planner.machine,
        planner.state,
        expression,
    );
    planner.retain_expression_call_arguments(expression);
    if !unit_statement
        && let Some(result_type) = planner.result_type
        && let Some(return_expression) = lower_return_expression(
            planner.program,
            planner.operators,
            expression,
            planner.scalar_parameters,
            planner.parameters,
            planner.parameter_types,
            planner.locals,
            result_type,
            planner.exact_integer_casts,
        )
    {
        planner.record_scalar_value_pair(
            CheckedScalarExpressionRole::Return,
            symbols::SymbolHandle::invalid(),
            expression,
            return_expression,
        );
    }
}
