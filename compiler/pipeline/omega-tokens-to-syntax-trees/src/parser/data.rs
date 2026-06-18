use crate::parser::expression::parse_expression_handle;
use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_type_reference_handle;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{
    DataDefinition, DataField, DataMember, DataProperties, DataVariant, TypeParameter,
    TypeParameterKind,
};
use omega_tokens::PunctuationKind;

pub(super) fn parse_data_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataDefinition> {
    let (name, mut input) = input.take_identifier()?;
    // CONCURRENCY STAGE 1: `Join` is reserved as a data-type name so the
    // parser's `Join<T>` -> `T` erasure (type_reference.rs) can never collide
    // with a user generic.
    if name.as_str() == "Join" {
        return Err(input.error_here(
            "data name `Join` is reserved: `Join<T>` is the spawn handle type (chapter 17)",
        ));
    }
    let (type_parameters, next) = parse_type_parameters(syntax_trees, input)?;
    input = next;
    let (properties, next) = parse_property_brackets(input)?;
    input = next;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let (members, input) = parse_data_members(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    Ok((
        DataDefinition {
            name,
            type_parameters,
            properties,
            members,
        },
        input,
    ))
}

/// Parse an optional declared-property bracket list. The same list attaches
/// to a data declaration (`data Point [copy, zero_init] { ... }`, frozen
/// decision 8) and to a type parameter (`data Box<T [copy]>`, frozen decision
/// 13) — brackets attach to what they follow, everywhere. The property set is
/// closed, so unknown names, duplicates, and the computed-only `sized` are
/// rejected here rather than in validation.
fn parse_property_brackets<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataProperties> {
    let mut properties = DataProperties::default();
    if !input.at_punctuation(PunctuationKind::LeftBracket) {
        return Ok((properties, input));
    }

    let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
    loop {
        let (name, next) = input.take_identifier()?;
        let flag = match name.as_str() {
            "copy" => &mut properties.copy,
            "zero_init" => &mut properties.zero_init,
            "send" => &mut properties.send,
            "sized" => {
                return Err(next.error_here(
                    "type property `sized` is computed from the data shape and cannot be declared",
                ));
            }
            other => {
                return Err(next.error_here(format!(
                    "unknown type property `{other}`; declared properties are `copy`, `zero_init`, `send`"
                )));
            }
        };
        if *flag {
            return Err(next.error_here(format!(
                "duplicate type property `{}`",
                name.as_str()
            )));
        }
        *flag = true;
        input = next;

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
        break;
    }

    let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
    Ok((properties, input))
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
        // A leading bracket is the attribute-prefix spelling, which decision
        // 13 rejects: brackets attach to what they FOLLOW.
        if input.at_punctuation(PunctuationKind::LeftBracket) {
            return Err(input.error_here(
                "property brackets attach to the name they follow: write the bounds after the type parameter, like `T [copy]`",
            ));
        }

        // A lifetime parameter (`<'buf>`); frozen decision 15 stage 2. Accepted
        // and consumed here. Uses are linked to it by NAME through the
        // reference-type lifetime tag, so the declaration is not yet recorded
        // downstream — undeclared-lifetime validation is a stage-2 hardening
        // follow-on, not a correctness requirement for the borrow linkage.
        if input.at_punctuation(PunctuationKind::Apostrophe) {
            let after_tick = input.take_punctuation(PunctuationKind::Apostrophe, "'")?;
            let (_lifetime_name, next) = after_tick.take_identifier()?;
            input = next;

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

        // Rust-style `<T: copy>` is rejected with the bracket spelling
        // suggested: a colon bound would split the property spelling system.
        if matches!(kind, TypeParameterKind::Type)
            && input.at_punctuation(PunctuationKind::Colon)
        {
            let after_colon = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let parameter = name.as_str();
            let bound = after_colon
                .take_identifier()
                .map(|(bound, _)| bound.as_str().to_owned())
                .unwrap_or_else(|_| "copy".to_owned());
            return Err(input.error_here(format!(
                "type parameter `{parameter}` takes property bounds in brackets after its name: write `{parameter} [{bound}]`, not `{parameter}: {bound}`"
            )));
        }

        // Brackets after a const parameter never reach here: they attach to
        // the const's TYPE as a constraint list (`const N: usize [range ...]`).
        let bounds = if input.at_punctuation(PunctuationKind::LeftBracket) {
            let (bounds, next) = parse_property_brackets(input)?;
            input = next;
            bounds
        } else {
            DataProperties::default()
        };

        let handle = syntax_trees
            .items
            .append_type_parameter(TypeParameter { name, kind, bounds });
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
