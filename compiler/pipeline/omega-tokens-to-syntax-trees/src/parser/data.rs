use crate::parser::expression::parse_expression_handle;
use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_type_reference_handle;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{
    DataDefinition, DataField, DataMember, DataVariant, TypeParameter, TypeParameterKind,
};
use omega_tokens::PunctuationKind;

pub(super) fn parse_data_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataDefinition> {
    let (name, mut input) = input.take_identifier()?;
    let (type_parameters, next) = parse_type_parameters(syntax_trees, input)?;
    input = next;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let (members, input) = parse_data_members(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    Ok((
        DataDefinition {
            name,
            type_parameters,
            members,
        },
        input,
    ))
}

fn parse_data_members<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<DataMember>> {
    let mut member_start = Handle::invalid();
    let mut member_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (member, next) = parse_data_member(syntax_trees, input)?;
        input = next;
        let handle = syntax_trees.items.append_data_member(member);
        if member_count == 0 {
            member_start = handle;
        }
        member_count = member_count
            .checked_add(1)
            .expect("data member span count overflow");
    }

    let members = if member_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(member_start, member_count)
    };
    Ok((members, input))
}

fn parse_data_member<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataMember> {
    if input.at_contextual("version") {
        input = input.take_contextual("version")?;
        let (name, next) = input.take_identifier()?;
        input = next.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        let (members, next) = parse_data_members(syntax_trees, input)?;
        let input = next.take_punctuation(PunctuationKind::RightBrace, "}")?;
        return Ok((
            DataMember::Version(omega_syntax_trees::item::DataVersion { name, members }),
            input,
        ));
    }

    if input.at_contextual("case") {
        // Distinguish a `case Name;` member from a field named `case`
        // (`case: i32;`) by what follows the contextual keyword.
        let after_case = input.take_contextual("case")?;
        if after_case.at_name_like() {
            return parse_case_member(syntax_trees, after_case);
        }
    }

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
            (
                omega_syntax_trees::expression::ExpressionHandle::invalid(),
                input,
            )
        };
        input = if next.at_punctuation(PunctuationKind::Semicolon) {
            next.take_punctuation(PunctuationKind::Semicolon, ";")?
        } else {
            next
        };
        return Ok((
            DataMember::Field(DataField {
                name: field_name,
                type_reference,
                initial_value,
            }),
            input,
        ));
    }

    // A bare `Name;` member (the pre-`case` variant spelling) is retired:
    // `case Name;` is the canonical alternative member.
    Err(input.error_here(format!(
        "expected `:` after data field `{}` (alternatives are spelled `case {};`)",
        field_name.as_str(),
        field_name.as_str()
    )))
}

fn parse_case_member<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataMember> {
    let (case_name, mut input) = input.take_identifier()?;

    let payload = if input.at_punctuation(PunctuationKind::LeftParen) {
        let (payload, next) = parse_case_payload_fields(syntax_trees, input)?;
        input = next;
        payload
    } else {
        HandleSpan::empty()
    };

    input = if input.at_punctuation(PunctuationKind::Semicolon) {
        input.take_punctuation(PunctuationKind::Semicolon, ";")?
    } else {
        input
    };
    Ok((
        DataMember::Variant(DataVariant {
            name: case_name,
            payload,
        }),
        input,
    ))
}

/// Parse the named payload field list of a case member:
/// `case Say(text: String, repeat: i32);`. Fields are name-and-type only;
/// payload fields take no default initializer (a case payload only exists once
/// the case is constructed).
fn parse_case_payload_fields<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<DataField>> {
    let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let mut payload_start = Handle::invalid();
    let mut payload_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightParen) {
        let (field_name, next) = input.take_identifier()?;
        input = next.take_punctuation(PunctuationKind::Colon, ":")?;
        let (type_reference, next) = parse_type_reference_handle(syntax_trees, input)?;
        input = next;

        let handle = syntax_trees.items.append_data_payload_field(DataField {
            name: field_name,
            type_reference,
            initial_value: omega_syntax_trees::expression::ExpressionHandle::invalid(),
        });
        if payload_count == 0 {
            payload_start = handle;
        }
        payload_count = payload_count
            .checked_add(1)
            .expect("case payload field span count overflow");

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        } else {
            break;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let payload = if payload_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(payload_start, payload_count)
    };
    Ok((payload, input))
}

pub(super) fn parse_type_parameters<'tokens, 'source>(
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
        let (name, kind, next) = if input.at_contextual("const") {
            let input = input.take_contextual("const")?;
            let (name, input) = input.take_identifier()?;
            let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
            (name, TypeParameterKind::Const { type_reference }, input)
        } else {
            let (name, input) = input.take_identifier()?;
            (name, TypeParameterKind::Type, input)
        };
        input = next;
        let handle = syntax_trees
            .items
            .append_type_parameter(TypeParameter { name, kind });
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
