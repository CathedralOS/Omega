use crate::parser::expression::parse_expression;
use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_type_reference_allowing_borrow;
use omega_syntax_trees::expression::{CallExpression, Expression, MemberExpression};
use omega_syntax_trees::identifier::IdentifierPath;
use omega_syntax_trees::statement::{Assignment, Call, LocalData, Statement};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_statement<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Statement> {
    if input.at_keyword(KeywordKind::Let) {
        let input = input.take_keyword(KeywordKind::Let, "let")?;
        return parse_local_data_statement(input);
    }

    let (expression, input) = parse_expression(input)?;

    if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (value, input) = parse_expression(input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            Statement::Assignment(Assignment {
                target: expression,
                value,
            }),
            input,
        ));
    }

    if input.at_punctuation(PunctuationKind::RightBrace) {
        return Ok((Statement::Expression(expression), input));
    }

    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    if let Some(call) = expression_to_statement_call(&expression) {
        Ok((Statement::Call(call), input))
    } else {
        Ok((Statement::Expression(expression), input))
    }
}

fn parse_local_data_statement<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Statement> {
    let (name, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, input) = parse_type_reference_allowing_borrow(input)?;
    let (initial_value, input) = if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (expression, input) = parse_expression(input)?;
        (Some(expression), input)
    } else {
        (None, input)
    };
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;

    Ok((
        Statement::LocalData(LocalData {
            name,
            type_reference,
            initial_value,
        }),
        input,
    ))
}

fn expression_to_statement_call(expression: &Expression) -> Option<Call> {
    let Expression::Call(call) = expression else {
        return None;
    };

    let (receiver, target) = split_expression_call(call)?;
    Some(Call {
        receiver,
        target,
        arguments: call.arguments.clone(),
    })
}

fn split_expression_call(
    call: &CallExpression,
) -> Option<(Option<IdentifierPath>, omega_syntax_trees::identifier::Identifier)> {
    let receiver = match call.receiver.as_deref() {
        None => None,
        Some(expression) => Some(expression_to_identifier_path(expression)?),
    };

    Some((receiver, call.target.clone()))
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
