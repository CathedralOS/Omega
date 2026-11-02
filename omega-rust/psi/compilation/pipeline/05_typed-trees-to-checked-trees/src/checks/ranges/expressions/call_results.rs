//! Index bounds and captured values consume the same selected normal-return
//! contract projector. Only declaration-owned builtin result comparisons
//! contribute; neither a same-spelled result nor a private dispatched body
//! supplies the public call's evidence.
use typed_trees::expression::ExpressionHandle;

pub(in crate::checks::ranges) fn ensured_call_result_bounds(
    program: &dyn ::validation::CheckingInput,
    caller: symbols::SymbolHandle,
    expression: ExpressionHandle,
) -> Option<(Option<i64>, Option<i64>)> {
    validation::normal_return_integer_bounds(
        program,
        caller,
        expression,
        program.expression_table.call_node(expression)?,
    )
}
