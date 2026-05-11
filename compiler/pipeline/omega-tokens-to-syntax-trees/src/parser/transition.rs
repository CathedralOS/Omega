use crate::parser::expression::parse_expression;
use crate::parser::input::{Input, ParseResult};
use crate::parse_error::ParseError;
use omega_syntax_trees::expression::{BinaryExpression, BinaryOperator, CallExpression, Expression, MemberExpression};
use omega_syntax_trees::identifier::IdentifierPath;
use omega_syntax_trees::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_transition_statement<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Statement> {
    let (target, mut input) = parse_transition_target(input)?;
    let continuation;
    if input.at_punctuation(PunctuationKind::Arrow) {
        input = input.take_punctuation(PunctuationKind::Arrow, "->")?;
        let (next_target, rest) = parse_transition_target(input)?;
        continuation = Some(next_target);
        input = rest;
    } else {
        continuation = None;
    }

    let guard;
    if input.at_keyword(KeywordKind::When) || input.at_contextual("when") {
        let input = if input.at_keyword(KeywordKind::When) {
            input.take_keyword(KeywordKind::When, "when")?
        } else {
            input.take_contextual("when")?
        };
        let (expression, rest) = parse_expression(input)?;
        let input = if rest.at_punctuation(PunctuationKind::Semicolon) {
            rest.take_punctuation(PunctuationKind::Semicolon, ";")?
        } else {
            rest
        };
        guard = TransitionGuard::When(expression);
        return Ok((
            Statement::Transition(Transition {
                target,
                continuation,
                guard,
            }),
            input,
        ));
    }

    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    guard = TransitionGuard::Always;
    Ok((
        Statement::Transition(Transition {
            target,
            continuation,
            guard,
        }),
        input,
    ))
}

pub(super) fn parse_transition_block<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<Statement>> {
    let (subject, mut input) = if input.at_punctuation(PunctuationKind::LeftBrace) {
        (None, input.take_punctuation(PunctuationKind::LeftBrace, "{")?)
    } else {
        let (expression, rest) = parse_expression_until_punctuation(input, PunctuationKind::LeftBrace)?;
        let input = rest.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        (Some(expression), input)
    };

    let mut statements = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (guard, rest) = if input.at_contextual("_") {
            (TransitionGuard::Always, input.take_contextual("_")?)
        } else {
            let (pattern, rest) = parse_expression(input)?;
            if let Some(subject) = &subject {
                (
                    TransitionGuard::When(Expression::Binary(Box::new(BinaryExpression {
                        left: subject.clone(),
                        operator: BinaryOperator::Equal,
                        right: pattern,
                    }))),
                    rest,
                )
            } else {
                (TransitionGuard::When(pattern), rest)
            }
        };
        input = rest.take_punctuation(PunctuationKind::Arrow, "->")?;

        let (target, rest) = if input.at_punctuation(PunctuationKind::LeftBrace) {
            let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
            let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
            (TransitionTarget::Terminal, input)
        } else {
            parse_transition_target(input)?
        };
        input = rest;

        statements.push(Statement::Transition(Transition {
            target,
            continuation: None,
            guard,
        }));
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((statements, input))
}

pub(super) fn parse_transition_target<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TransitionTarget> {
    let (expression, rest) = parse_expression(input)?;
    Ok((classify_transition_target(expression), rest))
}

fn parse_expression_until_punctuation<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    delimiter: PunctuationKind,
) -> Result<(Expression, Input<'tokens, 'source>), ParseError> {
    let split_index = find_top_level_punctuation(input, delimiter)
        .ok_or_else(|| input.error_here("expected transition block delimiter"))?;
    let (expression_tokens, rest_tokens) = input.tokens.split_at(split_index);
    let expression_input = Input::new(input.source_id, expression_tokens);
    let (expression, rest) = parse_expression(expression_input)?;

    if !rest.tokens.is_empty() {
        return Err(rest.error_here("expected transition subject expression"));
    }

    Ok((expression, Input::new(input.source_id, rest_tokens)))
}

fn find_top_level_punctuation<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    delimiter: PunctuationKind,
) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    for (index, token) in input.tokens.iter().enumerate() {
        match token.punctuation() {
            Some(PunctuationKind::LeftParen) => paren_depth += 1,
            Some(PunctuationKind::RightParen) => paren_depth = paren_depth.saturating_sub(1),
            Some(PunctuationKind::LeftBracket) => bracket_depth += 1,
            Some(PunctuationKind::RightBracket) => bracket_depth = bracket_depth.saturating_sub(1),
            Some(PunctuationKind::LeftBrace) => {
                if delimiter == PunctuationKind::LeftBrace
                    && paren_depth == 0
                    && bracket_depth == 0
                    && brace_depth == 0
                {
                    return Some(index);
                }
                brace_depth += 1;
            }
            Some(PunctuationKind::RightBrace) => brace_depth = brace_depth.saturating_sub(1),
            Some(punctuation)
                if punctuation == delimiter
                    && paren_depth == 0
                    && bracket_depth == 0
                    && brace_depth == 0 =>
            {
                return Some(index);
            }
            _ => {}
        }
    }

    None
}

fn classify_transition_target(expression: Expression) -> TransitionTarget {
    match expression {
        Expression::Call(call) => classify_call_target(*call),
        Expression::Name(path) if path.len() == 1 && path.as_slice()[0].as_str() == "self" => {
            TransitionTarget::SelfTarget
        }
        value => TransitionTarget::Value(value),
    }
}

fn classify_call_target(call: CallExpression) -> TransitionTarget {
    let path = match call.receiver.as_deref() {
        None => IdentifierPath::from(vec![call.target.clone()]),
        Some(receiver) => {
            let Some(mut path) = expression_to_identifier_path(receiver) else {
                return TransitionTarget::Value(Expression::Call(Box::new(call)));
            };
            path.push(call.target.clone());
            path
        }
    };

    if path.len() == 1 && path.as_slice()[0].as_str() == "self" {
        TransitionTarget::SelfTarget
    } else {
        TransitionTarget::Named {
            path,
            arguments: call.arguments,
        }
    }
}

fn expression_to_identifier_path(expression: &Expression) -> Option<IdentifierPath> {
    match expression {
        Expression::Name(path) => Some(path.clone()),
        Expression::Member(member) => {
            let MemberExpression { receiver, member } = member.as_ref();
            let mut path = expression_to_identifier_path(receiver)?;
            path.push(member.clone());
            Some(path)
        }
        _ => None,
    }
}
