use crate::parse_error::ParseError;
use crate::parser::context::ExpressionContext;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use omega_core::arena::HandleSpan;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCallExpression,
    TableCastExpression, TableIndexedExpression, TableMemberExpression, TableMembershipExpression,
    TableStructLiteral, TableStructLiteralField,
};
use omega_syntax_trees::identifier::Identifier;
use omega_tokens::{KeywordKind, NumericLiteralKind, PunctuationKind, TokenKind};

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

pub(super) fn parse_argument_list_after_open_paren_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<ExpressionHandle>> {
    let mut arguments = Vec::new();

    if !input.at_punctuation(PunctuationKind::RightParen) {
        loop {
            let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
            arguments.push(expression);
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                if input.at_punctuation(PunctuationKind::RightParen) {
                    break;
                }
                continue;
            }

            break;
        }
    }

    input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let arguments = syntax_trees
        .expressions
        .insert_expression_handles(arguments);
    Ok((arguments, input))
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

fn parse_membership_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (mut expression, mut input) = parse_shift_expression_handle(syntax_trees, input, context)?;

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

fn parse_shift_expression_handle<'tokens, 'source>(
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

    parse_postfix_expression_handle(syntax_trees, input, context)
}

fn parse_postfix_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (mut expression, mut input) =
        parse_primary_expression_handle(syntax_trees, input, context)?;

    loop {
        if input.at_punctuation(PunctuationKind::LeftParen) {
            input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (arguments, rest) =
                parse_argument_list_after_open_paren_handle(syntax_trees, input)?;
            input = rest;
            expression = build_call_expression_handle(syntax_trees, expression, arguments)?;
            continue;
        }

        if input.at_punctuation(PunctuationKind::LeftBracket) {
            input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
            let (index, rest) =
                parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
            input = rest.take_punctuation(PunctuationKind::RightBracket, "]")?;
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Indexed(TableIndexedExpression {
                        collection: expression,
                        index,
                    }));
            continue;
        }

        if input.at_punctuation(PunctuationKind::Dot) {
            input = input.take_punctuation(PunctuationKind::Dot, ".")?;
            let (member, rest) = input.take_identifier()?;
            input = rest;
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Member(TableMemberExpression {
                        receiver: expression,
                        member,
                    }));
            continue;
        }

        if input.at_keyword(KeywordKind::As) {
            input = input.take_keyword(KeywordKind::As, "as")?;
            let (target_type, rest) = parse_path_handle_span(input, |member| {
                syntax_trees
                    .expressions
                    .append_identifier_path_member(member)
            })?;
            input = rest;
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Cast(TableCastExpression {
                        value: expression,
                        target_type,
                    }));
            continue;
        }

        break;
    }

    Ok((expression, input))
}

fn parse_primary_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    if input.at_keyword(KeywordKind::True) {
        let input = input.take_keyword(KeywordKind::True, "true")?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::Boolean(true)),
            input,
        ));
    }

    if input.at_keyword(KeywordKind::False) {
        let input = input.take_keyword(KeywordKind::False, "false")?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::Boolean(false)),
            input,
        ));
    }

    if input.at_keyword(KeywordKind::SelfValue) {
        let input = input.take_keyword(KeywordKind::SelfValue, "self")?;
        return Ok((
            syntax_trees.expressions.insert(ExpressionNode::SelfValue),
            input,
        ));
    }

    if input.tokens.first().is_some_and(|token| {
        matches!(
            token.kind,
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(_))
        )
    }) {
        let (value, input) = input.take_integer()?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::Integer(value)),
            input,
        ));
    }

    if input.tokens.first().is_some_and(|token| {
        matches!(
            token.kind,
            TokenKind::NumericLiteral(NumericLiteralKind::Float(_))
        )
    }) {
        let (value, input) = input.take_float_text()?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::Float(value)),
            input,
        ));
    }

    if input
        .tokens
        .first()
        .is_some_and(|token| token.kind == TokenKind::StringLiteral)
    {
        let (value, input) = input.take_string()?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::String(value.into())),
            input,
        ));
    }

    if input.at_punctuation(PunctuationKind::LeftParen) {
        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let (expression, input) =
            parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        return Ok((expression, input));
    }

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
        let mut values = Vec::new();

        if !input.at_punctuation(PunctuationKind::RightBracket) {
            loop {
                let (value, rest) =
                    parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
                values.push(value);
                input = rest;

                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }

                break;
            }
        }

        let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
        let values = syntax_trees.expressions.insert_expression_handles(values);
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::ArrayLiteral(values)),
            input,
        ));
    }

    if input.at_name_like() {
        let (path, input) = parse_path_handle_span(input, |member| {
            syntax_trees
                .expressions
                .append_identifier_path_member(member)
        })?;

        if context.allows_struct_literal()
            && input.at_punctuation(PunctuationKind::LeftBrace)
            && path.count() == 1
        {
            let type_name = syntax_trees
                .expressions
                .identifier_path_members(path)
                .first()
                .cloned()
                .expect("single-member path should have one member");
            return parse_struct_literal_handle(syntax_trees, type_name, input);
        }

        return Ok((
            syntax_trees.expressions.insert(ExpressionNode::Name(path)),
            input,
        ));
    }

    Err(input.error_here("expected expression"))
}

fn parse_struct_literal_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_name: Identifier,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut fields = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (name, rest) = input.take_identifier()?;
        input = rest.take_punctuation(PunctuationKind::Colon, ":")?;
        let (value, rest) = parse_expression_handle(syntax_trees, input)?;
        input = rest;
        fields.push(TableStructLiteralField { name, value });

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let fields = syntax_trees.expressions.insert_struct_fields(fields);
    Ok((
        syntax_trees
            .expressions
            .insert(ExpressionNode::StructLiteral(TableStructLiteral {
                type_name,
                fields,
            })),
        input,
    ))
}

fn build_call_expression_handle(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    arguments: HandleSpan<ExpressionHandle>,
) -> Result<ExpressionHandle, ParseError> {
    let expression = syntax_trees.expressions.expression(expression).clone();
    match expression {
        ExpressionNode::Name(path) => {
            let members = syntax_trees.expressions.identifier_path_members(path);
            let target = members
                .last()
                .cloned()
                .ok_or_else(|| ParseError::new("missing call target"))?;
            let receiver = if members.len() <= 1 {
                ExpressionHandle::invalid()
            } else {
                let receiver_path = syntax_trees
                    .expressions
                    .copy_identifier_path_prefix(path, members.len() - 1);
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Name(receiver_path))
            };

            Ok(syntax_trees
                .expressions
                .insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target,
                    arguments,
                })))
        }
        ExpressionNode::Member(member) => {
            Ok(syntax_trees
                .expressions
                .insert(ExpressionNode::Call(TableCallExpression {
                    receiver: member.receiver,
                    target: member.member,
                    arguments,
                })))
        }
        _ => Err(ParseError::new(
            "call target must be a path or member access",
        )),
    }
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
