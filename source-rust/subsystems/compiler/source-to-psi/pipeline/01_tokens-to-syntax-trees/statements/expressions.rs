use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::statement::TableCall;

pub(crate) fn reject_retired_root_receiver(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Result<(), crate::ParseError> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression) else {
        return Ok(());
    };
    if call.target.as_str(syntax_trees) == "bind"
        && !call.target_is_static
        && call.receiver.is_valid()
        && matches!(syntax_trees.expressions.expression(call.receiver),
            ExpressionNode::Member(member) if member.member.as_str(syntax_trees) == "roots")
    {
        return Err(crate::ParseError::at_source_span(
            "receiver-style root binding is retired; use Build::bind(builder, slot, entry)",
            call.target.source_span(),
        ));
    }
    Ok(())
}

pub(crate) fn expression_handle_to_statement_call(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<TableCall> {
    matches!(
        syntax_trees.expressions.expression(expression),
        ExpressionNode::Call(_)
    )
    .then_some(TableCall {
        expression,
        discards_result: false,
    })
}
