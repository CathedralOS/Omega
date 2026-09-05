use crate::parser::context::ExpressionContext;
use crate::parser::expression::parse_bitwise_or_expression_handle;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
    TableMembershipExpression,
};
use tokens::PunctuationKind;

pub(super) fn parse_membership_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (mut expression, mut input) =
        parse_bitwise_or_expression_handle(syntax_trees, input, context)?;

    while context.allows_membership() && input.at_contextual("in") {
        input = input.take_contextual("in")?;
        let subject = expression;
        let (domain, rest) = parse_path_handle_span(input, |member| {
            syntax_trees
                .expressions
                .append_identifier_path_member(member)
        })?;
        input = rest;
        let membership = syntax_trees.expressions.insert(ExpressionNode::Membership(
            TableMembershipExpression {
                value: expression,
                domain,
            },
        ));
        expression = membership;

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
                syntax_trees
                    .expressions
                    .append_identifier_path_member(member)
            })?;
            input = rest;
            let right = syntax_trees.expressions.insert(ExpressionNode::Membership(
                TableMembershipExpression {
                    value: subject,
                    domain,
                },
            ));
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left: expression,
                        operator,
                        right,
                    }));
        }
    }

    Ok((expression, input))
}
