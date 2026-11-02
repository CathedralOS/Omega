//! Compound facts use ordinary operator precedence, including membership on
//! either side of a Boolean operator. Standalone membership remains with the
//! fact parser, which owns indexed domains and the `&`/`|` domain bundles.

use super::context::ExpressionContext;
use super::parse_expression::parse_expression_handle_in;
use crate::storage::tables::exprs_of;
use crate::tokens::token_cursor::{Input, ParseResult};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn parse<'tokens, 'source>(
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
        // Authored binary operators carry their own token span. Domain-list
        // conjunction/disjunction is synthesized without that span and must
        // keep the fact parser's declaration/application records instead.
        let expressions = exprs_of(syntax);
        let span = expressions.source_span(expression).span;
        if matches!(
            expressions.expression(expression),
            ExpressionNode::Binary(_)
        ) && span.start < span.end
        {
            return Ok((expression, rest));
        }
    }
    syntax.rewind_appended(checkpoint);
    parse_expression_handle_in(syntax, input, fact_context)
}
