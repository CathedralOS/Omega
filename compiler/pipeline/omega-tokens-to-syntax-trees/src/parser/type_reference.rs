use crate::parser::expression::parse_expression_without_struct_literals;
use crate::parser::input::{Input, ParseResult};
use crate::parse_error::ParseError;
use omega_syntax_trees::expression::Expression;
use omega_syntax_trees::types::{TypeConstraint, TypeReference};
use omega_tokens::PunctuationKind;

pub(super) fn parse_type_reference<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReference> {
    if input.at_punctuation(PunctuationKind::LeftParen) {
        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        return Ok((TypeReference::Unit, input));
    }

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
        let (element_type, input) = parse_type_reference(input)?;
        if input.at_punctuation(PunctuationKind::Semicolon) {
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            let (length, input) = input.take_integer()?;
            let length = usize::try_from(length)
                .map_err(|_| input.error_here("expected non-negative array length"))?;
            let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
            return Ok((
                TypeReference::FixedArray {
                    element_type: Box::new(element_type),
                    length,
                },
                input,
            ));
        }

        let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
        return Ok((
            TypeReference::Slice {
                element_type: Box::new(element_type),
            },
            input,
        ));
    }

    let (base_name, mut input) = input.take_identifier()?;
    let mut type_reference = if input.at_punctuation(PunctuationKind::Less) {
        input = input.take_punctuation(PunctuationKind::Less, "<")?;
        let mut arguments = Vec::new();

        loop {
            let (argument, rest) = parse_type_reference(input)?;
            arguments.push(argument);
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            break;
        }

        input = input.take_punctuation(PunctuationKind::Greater, ">")?;
        TypeReference::Generic {
            base_name,
            arguments,
        }
    } else {
        TypeReference::Named(base_name)
    };

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let (constraints, rest) = parse_type_constraints(input)?;
        input = rest;
        type_reference = TypeReference::Constrained {
            base_type: Box::new(type_reference),
            constraints,
        };
    }

    Ok((type_reference, input))
}

pub(super) fn parse_type_reference_allowing_borrow<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReference> {
    let (is_reference, is_mutable, input) = if input.at_punctuation(PunctuationKind::Ampersand) {
        let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
        if input.at_contextual("mut") {
            (true, true, input.take_contextual("mut")?)
        } else {
            (true, false, input)
        }
    } else {
        (false, false, input)
    };

    let (type_reference, input) = parse_type_reference(input)?;
    let type_reference = if is_reference {
        TypeReference::Reference {
            referee: Box::new(type_reference),
            is_mutable,
        }
    } else {
        type_reference
    };

    Ok((type_reference, input))
}

pub(super) fn parse_type_constraints<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<TypeConstraint>> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
    let mut constraints = Vec::new();

    if !input.at_punctuation(PunctuationKind::RightBracket) {
        loop {
            if input.at_contextual("range") {
                input = input.take_contextual("range")?;
                input = input.take_punctuation(PunctuationKind::Less, "<")?;
                let (minimum, rest) =
                    parse_expression_until_punctuation(input, PunctuationKind::Comma)?;
                input = rest.take_punctuation(PunctuationKind::Comma, ",")?;
                let (maximum, rest) =
                    parse_expression_until_punctuation(input, PunctuationKind::Greater)?;
                input = rest.take_punctuation(PunctuationKind::Greater, ">")?;
                constraints.push(TypeConstraint::Range { minimum, maximum });
            } else {
                let (name, rest) = input.take_identifier()?;
                input = rest;
                constraints.push(TypeConstraint::Named(name));
            }

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            break;
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
    Ok((constraints, input))
}

fn parse_expression_until_punctuation<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    delimiter: PunctuationKind,
) -> Result<(Expression, Input<'tokens, 'source>), ParseError> {
    let (expression_input, rest) =
        input.split_at_top_level_punctuation(delimiter, "expected constrained type delimiter")?;
    let (expression, rest_after_expression) =
        parse_expression_without_struct_literals(expression_input)?;

    if !rest_after_expression.tokens.is_empty() {
        return Err(rest_after_expression.error_here("expected constrained type expression"));
    }

    Ok((expression, rest))
}
