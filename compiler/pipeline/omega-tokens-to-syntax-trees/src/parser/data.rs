use crate::parser::expression::parse_expression_handle;
use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_type_reference_handle;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::item::{DataDefinition, DataField, DataMember, DataVariant, TypeParameter};
use omega_syntax_trees::SyntaxTrees;
use omega_tokens::PunctuationKind;

pub(super) fn parse_data_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataDefinition> {
    let (name, mut input) = input.take_identifier()?;
    let (type_parameters, next) = parse_type_parameters(syntax_trees, input)?;
    input = next;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut members = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (field_name, next) = input.take_identifier()?;
        input = next;

        if input.at_punctuation(PunctuationKind::Colon) {
            input = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, next) = parse_type_reference_handle(syntax_trees, input)?;
            input = next;
            let (initial_value, next) = if input.at_punctuation(PunctuationKind::Equal) {
                let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
                let (expression, input) = parse_expression_handle(syntax_trees, input)?;
                (expression, input)
            } else {
                (omega_syntax_trees::expression::ExpressionHandle::invalid(), input)
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
    Ok((DataDefinition { name, type_parameters, members }, input))
}

pub(super) fn parse_enum_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataDefinition> {
    let (name, mut input) = input.take_identifier()?;
    let (type_parameters, next) = parse_type_parameters(syntax_trees, input)?;
    input = next;
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
    Ok((DataDefinition { name, type_parameters, members }, input))
}

fn parse_type_parameters<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<TypeParameter>> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok((HandleSpan::empty(), input));
    }

    input = input.take_punctuation(PunctuationKind::Less, "<")?;
    let mut type_parameter_start = Handle::invalid();
    let mut type_parameter_count = 0u32;

    loop {
        let (name, next) = input.take_identifier()?;
        input = next;
        let handle = syntax_trees.items.append_type_parameter(TypeParameter { name });
        if type_parameter_count == 0 {
            type_parameter_start = handle;
        }
        type_parameter_count = type_parameter_count
            .checked_add(1)
            .expect("data type parameter span count overflow");

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }

        input = input.take_punctuation(PunctuationKind::Greater, ">")?;
        let type_parameters = if type_parameter_count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(type_parameter_start, type_parameter_count)
        };
        return Ok((type_parameters, input));
    }
}
