use crate::parser::context::ExpressionContext;
use crate::parser::input::{Input, ParseResult};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableUnaryExpression,
    UnaryOperator,
};
use omega_tokens::{KeywordKind, PunctuationKind};

mod membership;
mod postfix;
mod primary;

use membership::parse_membership_expression_handle;
pub(super) use postfix::parse_argument_list_after_open_paren_handle;
use postfix::parse_postfix_expression_handle;

pub(super) fn parse_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)
}

pub(super) fn parse_expression_handle_without_struct_literals<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_expression_handle_in(syntax_trees, input, ExpressionContext::NoStructLiteral)
}

pub(super) fn parse_expression_handle_without_struct_literals_or_membership<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_expression_handle_in(
        syntax_trees,
        input,
        ExpressionContext::NoStructLiteralOrMembership,
    )
}

fn parse_expression_handle_in<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_or_expression_handle(syntax_trees, input, context)
}

fn parse_or_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_and_expression_handle,
        &[(PunctuationKind::PipePipe, BinaryOperator::Or)],
    )
}

fn parse_and_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_equality_expression_handle,
        &[(PunctuationKind::AndAnd, BinaryOperator::And)],
    )
}

fn parse_equality_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_comparison_expression_handle,
        &[
            (PunctuationKind::EqualEqual, BinaryOperator::Equal),
            (PunctuationKind::ExclamationEqual, BinaryOperator::NotEqual),
        ],
    )
}

fn parse_comparison_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_membership_expression_handle,
        &[
            (PunctuationKind::LessEqual, BinaryOperator::LessOrEqual),
            (
                PunctuationKind::GreaterEqual,
                BinaryOperator::GreaterOrEqual,
            ),
            (PunctuationKind::Less, BinaryOperator::Less),
            (PunctuationKind::Greater, BinaryOperator::Greater),
        ],
    )
}

pub(super) fn parse_shift_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_add_expression_handle,
        &[
            (PunctuationKind::LessLess, BinaryOperator::ShiftLeft),
            (PunctuationKind::GreaterGreater, BinaryOperator::ShiftRight),
        ],
    )
}

fn parse_add_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_multiply_expression_handle,
        &[
            (PunctuationKind::Plus, BinaryOperator::Add),
            (PunctuationKind::Minus, BinaryOperator::Subtract),
        ],
    )
}

fn parse_multiply_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_unary_expression_handle,
        &[
            (PunctuationKind::Asterisk, BinaryOperator::Multiply),
            (PunctuationKind::Slash, BinaryOperator::Divide),
            (PunctuationKind::Percent, BinaryOperator::Modulo),
        ],
    )
}

fn parse_binary_chain_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
    lower: fn(
        &mut SyntaxTrees,
        Input<'tokens, 'source>,
        ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle>,
    operators: &[(PunctuationKind, BinaryOperator)],
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (mut expression, mut input) = lower(syntax_trees, input, context)?;

    loop {
        let Some((punctuation, operator)) = operators
            .iter()
            .find(|(punctuation, _)| input.at_punctuation(*punctuation))
            .copied()
        else {
            break;
        };

        input = input.take_punctuation(punctuation, punctuation_label(punctuation))?;
        let (right, rest) = lower(syntax_trees, input, context)?;
        input = rest;
        expression =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: expression,
                    operator,
                    right,
                }));
    }

    Ok((expression, input))
}

fn parse_unary_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    if input.at_punctuation(PunctuationKind::Ampersand) {
        let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
        if input.at_contextual("mut") || input.at_keyword(KeywordKind::State) {
            let input = if input.at_contextual("mut") {
                input.take_contextual("mut")?
            } else {
                input.take_keyword(KeywordKind::State, "state")?
            };
            let (expression, rest) = parse_unary_expression_handle(syntax_trees, input, context)?;
            return Ok((
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Mutable(expression)),
                rest,
            ));
        }

        return parse_unary_expression_handle(syntax_trees, input, context);
    }

    if input.at_punctuation(PunctuationKind::Exclamation) {
        let input = input.take_punctuation(PunctuationKind::Exclamation, "!")?;
        let (operand, rest) = parse_unary_expression_handle(syntax_trees, input, context)?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: UnaryOperator::LogicalNot,
                    operand,
                })),
            rest,
        ));
    }

    if input.at_contextual("move") {
        let input = input.take_contextual("move")?;
        // Ownership is currently inferred from the value flow itself. `move`
        // is accepted as explicit spelling for that move, then lowered to the
        // moved expression so the existing ownership lane remains the source of
        // truth.
        return parse_unary_expression_handle(syntax_trees, input, context);
    }

    parse_postfix_expression_handle(syntax_trees, input, context)
}

fn punctuation_label(punctuation: PunctuationKind) -> &'static str {
    match punctuation {
        PunctuationKind::PipePipe => "||",
        PunctuationKind::AndAnd => "&&",
        PunctuationKind::EqualEqual => "==",
        PunctuationKind::ExclamationEqual => "!=",
        PunctuationKind::LessEqual => "<=",
        PunctuationKind::GreaterEqual => ">=",
        PunctuationKind::Less => "<",
        PunctuationKind::Greater => ">",
        PunctuationKind::LessLess => "<<",
        PunctuationKind::GreaterGreater => ">>",
        PunctuationKind::Plus => "+",
        PunctuationKind::Minus => "-",
        PunctuationKind::Asterisk => "*",
        PunctuationKind::Slash => "/",
        PunctuationKind::Percent => "%",
        _ => "?",
    }
}
