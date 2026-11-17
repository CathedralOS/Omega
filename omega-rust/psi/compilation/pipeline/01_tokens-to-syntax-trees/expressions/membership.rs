use crate::expressions::context::ExpressionContext;
use crate::syntax_storage::tables::exprs;
use crate::token_input::cursor::{Input, ParseResult, parse_path_handle_span};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
    TableMembershipExpression,
};
use tokens::PunctuationKind;

/// Domain membership tests (`value in Domain & Other`) after a finished
/// arithmetic operand.
pub(crate) fn extend_membership<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut expression: ExpressionHandle,
    mut input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    while context.allows_membership() && input.at_contextual("in") {
        input = input.take_contextual("in")?;
        let subject = expression;
        let (domain, rest) = parse_path_handle_span(input, |member| {
            exprs(syntax_trees).append_identifier_path_member(member)
        })?;
        input = rest;
        expression =
            exprs(syntax_trees).insert(ExpressionNode::Membership(TableMembershipExpression {
                value: expression,
                domain,
            }));

        while input.at_punctuation(PunctuationKind::Ampersand)
            || input.at_punctuation(PunctuationKind::Pipe)
        {
            let (operator, after_operator) = if input.at_punctuation(PunctuationKind::Ampersand) {
                (
                    BinaryOperator::And,
                    input.take_punctuation(PunctuationKind::Ampersand, "&")?,
                )
            } else {
                (
                    BinaryOperator::Or,
                    input.take_punctuation(PunctuationKind::Pipe, "|")?,
                )
            };
            let (domain, rest) = parse_path_handle_span(after_operator, |member| {
                exprs(syntax_trees).append_identifier_path_member(member)
            })?;
            input = rest;
            let right =
                exprs(syntax_trees).insert(ExpressionNode::Membership(TableMembershipExpression {
                    value: subject,
                    domain,
                }));
            expression =
                exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    left: expression,
                    operator,
                    right,
                }));
        }
    }

    Ok((expression, input))
}
