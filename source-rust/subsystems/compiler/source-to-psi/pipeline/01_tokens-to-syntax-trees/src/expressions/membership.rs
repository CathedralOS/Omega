use crate::expressions::context::ExpressionContext;
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
        (expression, input) = parse_test(syntax_trees, subject, input)?;

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
            let (right, rest) = parse_test(syntax_trees, subject, after_operator)?;
            input = rest;
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        negation: false,
                        left: expression,
                        operator,
                        right,
                    }));
        }
    }

    Ok((expression, input))
}

fn parse_test<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    value: ExpressionHandle,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (domain, rest) = parse_path_handle_span(input, |member| {
        syntax_trees
            .expressions
            .append_identifier_path_member(member)
    })?;
    let (domain_arguments, rest) = if rest.at_expression_arguments() {
        crate::types::references::parse_domain_argument_handles(
            syntax_trees,
            rest.take_punctuation(PunctuationKind::ColonColon, "::")?,
        )?
    } else {
        (arena::HandleSpan::empty(), rest)
    };
    let test =
        syntax_trees
            .expressions
            .insert(ExpressionNode::Membership(TableMembershipExpression {
                value,
                domain,
                domain_arguments,
            }));
    syntax_trees
        .expressions
        .set_source_span(test, input.source_span_until(rest));
    Ok((test, rest))
}
