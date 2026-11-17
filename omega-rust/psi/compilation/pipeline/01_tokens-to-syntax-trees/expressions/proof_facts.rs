use crate::expressions::context::ExpressionContext;
use crate::expressions::parse::parse_expression_handle_in;
use crate::syntax_storage::tables::exprs_of;
use crate::token_input::cursor::{Input, ParseResult};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

pub(crate) fn parse<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    fact_context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let checkpoint = syntax.append_checkpoint();
    let expression_context = if fact_context.allows_logical_subject() {
        ExpressionContext::LogicalTypeExpressionNoStructLiteral
    } else {
        ExpressionContext::TypeExpressionNoStructLiteral
    };
    if let Ok((expression, rest)) = parse_expression_handle_in(syntax, input, expression_context) {
        let expressions = exprs_of(syntax);
        let span = expressions.source_span(expression).span;
        if matches!(
            expressions.expression(expression),
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
        ) && span.start < span.end
        {
            return Ok((expression, rest));
        }
    }
    syntax.rewind_appended(checkpoint);
    parse_expression_handle_in(syntax, input, fact_context)
}
