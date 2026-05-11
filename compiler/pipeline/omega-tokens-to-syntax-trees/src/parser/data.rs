use crate::parser::expression::parse_expression;
use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_type_reference;
use omega_syntax_trees::item::{DataDefinition, DataField, DataMember, DataVariant};
use omega_tokens::PunctuationKind;

pub(super) fn parse_data_definition<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataDefinition> {
    let (name, mut input) = input.take_identifier()?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut members = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (field_name, next) = input.take_identifier()?;
        input = next;

        if input.at_punctuation(PunctuationKind::Colon) {
            input = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, next) = parse_type_reference(input)?;
            input = next;
            let (initial_value, next) = if input.at_punctuation(PunctuationKind::Equal) {
                let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
                let (expression, input) = parse_expression(input)?;
                (Some(expression), input)
            } else {
                (None, input)
            };
            input = if next.at_punctuation(PunctuationKind::Semicolon) {
                next.take_punctuation(PunctuationKind::Semicolon, ";")?
            } else {
                next
            };
            members.push(DataMember::Field(DataField {
                name: field_name,
                type_reference,
                initial_value,
            }));
        } else {
            input = if input.at_punctuation(PunctuationKind::Semicolon) {
                input.take_punctuation(PunctuationKind::Semicolon, ";")?
            } else {
                input
            };
            members.push(DataMember::Variant(DataVariant { name: field_name }));
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((DataDefinition { name, members }, input))
}

pub(super) fn parse_enum_definition<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataDefinition> {
    let (name, mut input) = input.take_identifier()?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut members = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (variant_name, next) = input.take_identifier()?;
        input = next;
        members.push(DataMember::Variant(DataVariant { name: variant_name }));

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        } else if input.at_punctuation(PunctuationKind::Semicolon) {
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((DataDefinition { name, members }, input))
}
