use crate::parser::context::ExpressionContext;
use crate::parse_error::ParseError;
use crate::parser::input::{parse_path, Input, ParseResult};
use omega_syntax_trees::expression::{
    BinaryExpression, BinaryOperator, CallExpression, CastExpression, Expression,
    IndexedExpression, MemberExpression, StructLiteral, StructLiteralField,
};
use omega_syntax_trees::identifier::{Identifier, IdentifierPath};
use omega_tokens::{KeywordKind, NumericLiteralKind, PunctuationKind, TokenKind};

pub(super) fn parse_expression<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Expression> {
    parse_expression_in(input, ExpressionContext::Default)
}

pub(super) fn parse_expression_without_struct_literals<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Expression> {
    parse_expression_in(input, ExpressionContext::NoStructLiteral)
}

fn parse_expression_in<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, Expression> {
    parse_or_expression(input, context)
}

pub(super) fn parse_argument_list_after_open_paren<'tokens, 'source>(
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<Expression>> {
    let mut arguments = Vec::new();

    if !input.at_punctuation(PunctuationKind::RightParen) {
        loop {
            let (expression, rest) = parse_expression(input)?;
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
    Ok((arguments, input))
}

fn parse_or_expression<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, Expression> {
    parse_binary_chain(input, context, parse_and_expression, &[(
        PunctuationKind::PipePipe,
        BinaryOperator::Or,
    )])
}

fn parse_and_expression<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, Expression> {
    parse_binary_chain(input, context, parse_equality_expression, &[(
        PunctuationKind::AndAnd,
        BinaryOperator::And,
    )])
}

fn parse_equality_expression<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, Expression> {
    parse_binary_chain(
        input,
        context,
        parse_comparison_expression,
        &[
            (PunctuationKind::EqualEqual, BinaryOperator::Equal),
            (PunctuationKind::ExclamationEqual, BinaryOperator::NotEqual),
        ],
    )
}

fn parse_comparison_expression<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, Expression> {
    parse_binary_chain(
        input,
        context,
        parse_shift_expression,
        &[
            (PunctuationKind::LessEqual, BinaryOperator::LessOrEqual),
            (PunctuationKind::GreaterEqual, BinaryOperator::GreaterOrEqual),
            (PunctuationKind::Less, BinaryOperator::Less),
            (PunctuationKind::Greater, BinaryOperator::Greater),
        ],
    )
}

fn parse_shift_expression<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, Expression> {
    parse_binary_chain(
        input,
        context,
        parse_add_expression,
        &[
            (PunctuationKind::LessLess, BinaryOperator::ShiftLeft),
            (PunctuationKind::GreaterGreater, BinaryOperator::ShiftRight),
        ],
    )
}

fn parse_add_expression<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, Expression> {
    parse_binary_chain(
        input,
        context,
        parse_multiply_expression,
        &[
            (PunctuationKind::Plus, BinaryOperator::Add),
            (PunctuationKind::Minus, BinaryOperator::Subtract),
        ],
    )
}

fn parse_multiply_expression<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, Expression> {
    parse_binary_chain(
        input,
        context,
        parse_unary_expression,
        &[
            (PunctuationKind::Asterisk, BinaryOperator::Multiply),
            (PunctuationKind::Slash, BinaryOperator::Divide),
            (PunctuationKind::Percent, BinaryOperator::Modulo),
        ],
    )
}

fn parse_binary_chain<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
    lower: fn(Input<'tokens, 'source>, ExpressionContext) -> ParseResult<'tokens, 'source, Expression>,
    operators: &[(PunctuationKind, BinaryOperator)],
) -> ParseResult<'tokens, 'source, Expression> {
    let (mut expression, mut input) = lower(input, context)?;

    loop {
        let Some((punctuation, operator)) = operators
            .iter()
            .find(|(punctuation, _)| input.at_punctuation(*punctuation))
            .copied()
        else {
            break;
        };

        input = input.take_punctuation(punctuation, punctuation_label(punctuation))?;
        let (right, rest) = lower(input, context)?;
        input = rest;
        expression = Expression::Binary(Box::new(BinaryExpression {
            left: expression,
            operator,
            right,
        }));
    }

    Ok((expression, input))
}

fn parse_unary_expression<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, Expression> {
    if input.at_punctuation(PunctuationKind::Ampersand) {
        let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
        if input.at_contextual("mut") || input.at_keyword(KeywordKind::State) {
            let input = if input.at_contextual("mut") {
                input.take_contextual("mut")?
            } else {
                input.take_keyword(KeywordKind::State, "state")?
            };
            let (expression, rest) = parse_unary_expression(input, context)?;
            return Ok((Expression::Mutable(Box::new(expression)), rest));
        }

        return parse_unary_expression(input, context);
    }

    parse_postfix_expression(input, context)
}

fn parse_postfix_expression<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, Expression> {
    let (mut expression, mut input) = parse_primary_expression(input, context)?;

    loop {
        if input.at_punctuation(PunctuationKind::LeftParen) {
            input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (arguments, rest) = parse_argument_list_after_open_paren(input)?;
            input = rest;
            expression = build_call_expression(expression, arguments)?;
            continue;
        }

        if input.at_punctuation(PunctuationKind::LeftBracket) {
            input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
            let (index, rest) = parse_expression_in(input, ExpressionContext::Default)?;
            input = rest.take_punctuation(PunctuationKind::RightBracket, "]")?;
            expression = Expression::Indexed(Box::new(IndexedExpression {
                collection: expression,
                index,
            }));
            continue;
        }

        if input.at_punctuation(PunctuationKind::Dot) {
            input = input.take_punctuation(PunctuationKind::Dot, ".")?;
            let (member, rest) = input.take_identifier()?;
            input = rest;
            expression = Expression::Member(Box::new(MemberExpression {
                receiver: expression,
                member,
            }));
            continue;
        }

        if input.at_keyword(KeywordKind::As) {
            input = input.take_keyword(KeywordKind::As, "as")?;
            let (target_type, rest) = parse_path(input)?;
            input = rest;
            expression = Expression::Cast(Box::new(CastExpression {
                value: expression,
                target_type,
            }));
            continue;
        }

        break;
    }

    Ok((expression, input))
}

fn parse_primary_expression<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, Expression> {
    if input.at_keyword(KeywordKind::True) {
        let input = input.take_keyword(KeywordKind::True, "true")?;
        return Ok((Expression::Boolean(true), input));
    }

    if input.at_keyword(KeywordKind::False) {
        let input = input.take_keyword(KeywordKind::False, "false")?;
        return Ok((Expression::Boolean(false), input));
    }

    if input
        .tokens
        .first()
        .is_some_and(|token| matches!(token.kind, TokenKind::NumericLiteral(NumericLiteralKind::Integer(_))))
    {
        let (value, input) = input.take_integer()?;
        return Ok((Expression::Integer(value), input));
    }

    if input
        .tokens
        .first()
        .is_some_and(|token| matches!(token.kind, TokenKind::NumericLiteral(NumericLiteralKind::Float(_))))
    {
        let (value, input) = input.take_float_text()?;
        return Ok((Expression::Float(value), input));
    }

    if input
        .tokens
        .first()
        .is_some_and(|token| token.kind == TokenKind::StringLiteral)
    {
        let (value, input) = input.take_string()?;
        return Ok((Expression::String(value.into()), input));
    }

    if input.at_punctuation(PunctuationKind::LeftParen) {
        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let (expression, input) = parse_expression_in(input, ExpressionContext::Default)?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        return Ok((expression, input));
    }

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
        let mut values = Vec::new();

        if !input.at_punctuation(PunctuationKind::RightBracket) {
            loop {
                let (value, rest) = parse_expression_in(input, ExpressionContext::Default)?;
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
        return Ok((Expression::ArrayLiteral(values), input));
    }

    if input.at_name_like() {
        let (path, input) = parse_path(input)?;

        if context != ExpressionContext::NoStructLiteral
            && input.at_punctuation(PunctuationKind::LeftBrace)
            && path.len() == 1
        {
            let type_name = path
                .as_slice()
                .first()
                .cloned()
                .expect("single-member path should have one member");
            return parse_struct_literal(type_name, input);
        }

        return Ok((Expression::Name(path), input));
    }

    Err(input.error_here("expected expression"))
}

fn parse_struct_literal<'tokens, 'source>(
    type_name: Identifier,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Expression> {
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut fields = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (name, rest) = input.take_identifier()?;
        input = rest.take_punctuation(PunctuationKind::Colon, ":")?;
        let (value, rest) = parse_expression(input)?;
        input = rest;
        fields.push(StructLiteralField { name, value });

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((Expression::StructLiteral(StructLiteral { type_name, fields }), input))
}

fn build_call_expression(
    expression: Expression,
    arguments: Vec<Expression>,
) -> Result<Expression, ParseError> {
    match expression {
        Expression::Name(path) => {
            let (receiver, target) = split_name_path(path)?;
            Ok(Expression::Call(Box::new(CallExpression {
                receiver: receiver.map(|receiver| Box::new(Expression::Name(receiver))),
                target,
                arguments,
            })))
        }
        Expression::Member(member) => Ok(Expression::Call(Box::new(CallExpression {
            receiver: Some(Box::new(member.receiver)),
            target: member.member,
            arguments,
        }))),
        _ => Err(ParseError::new("call target must be a path or member access")),
    }
}

fn split_name_path(
    path: IdentifierPath,
) -> Result<(Option<IdentifierPath>, Identifier), ParseError> {
    let mut members = path.as_slice().to_vec();
    let target = members.pop().ok_or_else(|| ParseError::new("missing call target"))?;
    let receiver = if members.is_empty() {
        None
    } else {
        Some(IdentifierPath::from(members))
    };
    Ok((receiver, target))
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
