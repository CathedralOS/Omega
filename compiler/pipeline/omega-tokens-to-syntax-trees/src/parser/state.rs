use crate::parser::input::{Input, ParseResult};
use crate::parser::statement::parse_statement;
use crate::parser::transition::{parse_transition_block, parse_transition_statement};
use crate::parser::type_reference::parse_type_reference_allowing_borrow;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{State, StateParameter, StateSignature};
use omega_syntax_trees::types::TypeReference;
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_state_signature<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StateSignature> {
    let (name, input) = input.take_identifier()?;
    let (parameters, input) = parse_optional_state_parameters(input)?;
    let (return_type, input) = parse_optional_return_type(input)?;

    Ok((
        StateSignature {
            name,
            parameters,
            return_type,
        },
        input,
    ))
}

pub(super) fn parse_state<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    allow_unnamed_entry: bool,
) -> ParseResult<'tokens, 'source, State> {
    let (name, input) = if allow_unnamed_entry && input.at_punctuation(PunctuationKind::LeftParen) {
        (Identifier::generated("entry"), input)
    } else {
        input.take_identifier()?
    };

    let (parameters, input) = parse_optional_state_parameters(input)?;
    let (return_type, mut input) = parse_optional_return_type(input)?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut statements = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_punctuation(PunctuationKind::Arrow) {
            let next = input.take_punctuation(PunctuationKind::Arrow, "->")?;
            let (statement, rest) = parse_transition_statement(next)?;
            statements.push(statement);
            input = rest;
        } else if input.at_keyword(KeywordKind::Transition) || input.at_keyword(KeywordKind::Match)
        {
            let next = if input.at_keyword(KeywordKind::Transition) {
                input.take_keyword(KeywordKind::Transition, "transition")?
            } else {
                input.take_keyword(KeywordKind::Match, "match")?
            };
            let (new_statements, rest) = parse_transition_block(next)?;
            statements.extend(new_statements);
            input = rest;
        } else {
            let (statement, rest) = parse_statement(input)?;
            statements.push(statement);
            input = rest;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((
        State {
            name,
            parameters,
            return_type,
            statements,
        },
        input,
    ))
}

pub(super) fn parse_optional_state_parameters<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<StateParameter>> {
    if !input.at_punctuation(PunctuationKind::LeftParen) {
        return Ok((Vec::new(), input));
    }

    let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let mut parameters = Vec::new();

    if !input.at_punctuation(PunctuationKind::RightParen) {
        loop {
            let (parameter, rest) = parse_state_parameter(input)?;
            parameters.push(parameter);
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            break;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    Ok((parameters, input))
}

pub(super) fn parse_optional_return_type<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Option<TypeReference>> {
    if !input.at_punctuation(PunctuationKind::Arrow) {
        return Ok((None, input));
    }

    let input = input.take_punctuation(PunctuationKind::Arrow, "->")?;
    let (type_reference, input) = parse_type_reference_allowing_borrow(input)?;
    Ok((Some(type_reference), input))
}

fn parse_state_parameter<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StateParameter> {
    let (is_const, input) = if input.at_contextual("const") {
        (true, input.take_contextual("const")?)
    } else {
        (false, input)
    };

    if input.at_punctuation(PunctuationKind::Ampersand) {
        let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
        let (is_mutable, input) = if input.at_contextual("mut") {
            (true, input.take_contextual("mut")?)
        } else {
            (false, input)
        };

        if input.at_keyword(KeywordKind::SelfValue) || input.at_contextual("self") {
            let input = if input.at_keyword(KeywordKind::SelfValue) {
                input.take_keyword(KeywordKind::SelfValue, "self")?
            } else {
                input.take_contextual("self")?
            };

            return Ok((
                StateParameter {
                    name: Identifier::generated("self"),
                    type_reference: TypeReference::named("Self"),
                    is_const,
                    is_mutable,
                    is_self: true,
                },
                input,
            ));
        }

        let (name, input) = input.take_identifier()?;
        let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
        let (type_reference, input) = parse_type_reference_allowing_borrow(input)?;
        return Ok((
            StateParameter {
                name,
                type_reference,
                is_const,
                is_mutable,
                is_self: false,
            },
            input,
        ));
    }

    if input.at_keyword(KeywordKind::SelfValue) || input.at_contextual("self") {
        let input = if input.at_keyword(KeywordKind::SelfValue) {
            input.take_keyword(KeywordKind::SelfValue, "self")?
        } else {
            input.take_contextual("self")?
        };

        return Ok((
            StateParameter {
                name: Identifier::generated("self"),
                type_reference: TypeReference::named("Self"),
                is_const,
                is_mutable: false,
                is_self: true,
            },
            input,
        ));
    }

    let (name, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, input) = parse_type_reference_allowing_borrow(input)?;
    Ok((
        StateParameter {
            name,
            type_reference,
            is_const,
            is_mutable: false,
            is_self: false,
        },
        input,
    ))
}
