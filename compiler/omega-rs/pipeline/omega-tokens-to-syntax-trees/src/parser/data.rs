use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::{
    parse_type_reference_handle, parse_type_reference_handle_allowing_borrow,
};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{
    DataDefinition, DataField, DataMember, DataProperties, DataVariant, TypeParameter,
    TypeParameterKind,
};
use omega_tokens::PunctuationKind;

/// A parsed `data` declaration: plain, or IDENTITY-NUMBERED (ch20 -- fields
/// carry optional identity numbers, `retired N;` tombstones one; such a
/// declaration is the schema the identity-keyed grammars consume, and it
/// lowers through the wire-schema representation).
pub(super) enum ParsedDataDefinition {
    Plain(DataDefinition),
    Numbered(omega_syntax_trees::item::WireDataDefinition),
}

pub(super) fn parse_data_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedDataDefinition> {
    let (name, mut input) = input.take_identifier()?;
    // `Slice` is reserved as a data-type name so the parser's `Slice<T>` -> slice
    // fold (type_reference.rs) never collides with a user generic; `Slice<T>` is
    // the canonical spelling of the `[T]` slice type.
    if name.as_str() == "Slice" {
        return Err(input.error_here(
            "data name `Slice` is reserved: `Slice<T>` is the slice type (alias of `[T]`)",
        ));
    }
    let (type_parameters, next) = parse_type_parameters(syntax_trees, input)?;
    input = next;
    let (properties, next) = parse_property_brackets(input)?;
    input = next;
    // R2 rung 1 (ch12 "Dependent Data"): the DEFAULT-DOMAIN facts --
    // `data M where count * stride <= len, { ... }` -- bare field names,
    // comma-separated, ending at the body brace (trailing comma tolerated
    // by the fact parser).
    let mut where_facts = HandleSpan::empty();
    if input.at_contextual("where") {
        input = input.take_contextual("where")?;
        let ((facts, _token_count), rest) =
            crate::parser::proof_fact::parse_proof_facts_until(syntax_trees, input, |input| {
                input.at_punctuation(PunctuationKind::LeftBrace) || input.tokens.is_empty()
            })?;
        where_facts = facts;
        input = rest;
    }
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;

    // An IDENTITY-NUMBERED data (ch20): the first member starting with an
    // integer (`1: seed: u64;`) or `retired` decides the form; numbers are
    // all-or-nothing within one declaration (guided error otherwise, inside
    // the member parser). A numbered schema may start with a historical
    // `version vN { N: field: Type; }` block, so peek inside that first block;
    // an identity-numbered or retired inner member selects the schema parser.
    // The input cursor is Copy, so this lookahead consumes nothing.
    let leading_version_is_numbered = input.at_contextual("version")
        && input
            .take_contextual("version")
            .ok()
            .and_then(|after| after.take_identifier().ok())
            .and_then(|(_, after)| after.take_punctuation(PunctuationKind::LeftBrace, "{").ok())
            .is_some_and(|inner| inner.at_integer() || inner.at_contextual("retired"));
    if input.at_integer() || input.at_contextual("retired") || leading_version_is_numbered {
        if !type_parameters.is_empty() {
            return Err(input.error_here(
                "identity-numbered data does not take type parameters yet (the schema the \
                 tagged grammar consumes is concrete)",
            ));
        }
        if properties != DataProperties::default() {
            return Err(
                input.error_here("identity-numbered data does not take declared properties yet")
            );
        }
        let (definition, input) =
            crate::parser::item::parse_identity_data_body(syntax_trees, name, input)?;
        return Ok((ParsedDataDefinition::Numbered(definition), input));
    }

    let (members, input) = parse_data_members(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    Ok((
        ParsedDataDefinition::Plain(DataDefinition {
            name,
            supply_mode: omega_core::semantics::DataSupplyMode::CheckedShape,
            type_parameters,
            properties,
            where_facts,
            members,
        }),
        input,
    ))
}

/// An opaque carrier supplied by a boundary provider. It has no source-visible
/// representation and therefore ends in `;`, never a body. Property claims are
/// retained for the admission pass; validation fails closed where that path is
/// not implemented yet.
pub(super) fn parse_boundary_data_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataDefinition> {
    let (name, input) = input.take_identifier()?;
    if name.as_str() == "Slice" {
        return Err(input.error_here(
            "data name `Slice` is reserved: `Slice<T>` is the slice type (alias of `[T]`)",
        ));
    }
    let (type_parameters, input) = parse_type_parameters(syntax_trees, input)?;
    let (properties, input) = parse_property_brackets(input)?;
    if input.at_contextual("where") {
        return Err(input.error_here(
            "opaque `boundary data` has no visible fields for a default-domain `where` clause",
        ));
    }
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((
        DataDefinition {
            name,
            supply_mode: omega_core::semantics::DataSupplyMode::BoundaryOpaque,
            type_parameters,
            properties,
            where_facts: HandleSpan::empty(),
            members: HandleSpan::empty(),
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
    let mut declared_multiplicity: Option<&'static str> = None;
    if !input.at_punctuation(PunctuationKind::LeftBracket) {
        return Ok((properties, input));
    }

    let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
    while !input.at_punctuation(PunctuationKind::RightBracket) {
        let (name, next) = input.take_identifier()?;
        match name.as_str() {
            "copy" | "linear" => {
                let spelling = if name.as_str() == "copy" {
                    "copy"
                } else {
                    "linear"
                };
                if let Some(previous) = declared_multiplicity {
                    let message = if previous == spelling {
                        format!("duplicate type property `{spelling}`")
                    } else {
                        format!(
                            "type properties `[copy]` and `[linear]` are mutually exclusive; \
                             `{previous}` was already declared"
                        )
                    };
                    return Err(next.error_here(message));
                }
                declared_multiplicity = Some(spelling);
                properties.multiplicity = if spelling == "copy" {
                    omega_core::semantics::Multiplicity::Unrestricted
                } else {
                    omega_core::semantics::Multiplicity::Linear
                };
                input = next;
            }
            "zero_init" => {
                if properties.zero_init {
                    return Err(next.error_here("duplicate type property `zero_init`"));
                }
                properties.zero_init = true;
                input = next;
            }
            "carry" => {
                if properties.carry.is_some() {
                    return Err(next.error_here("duplicate type property `carry`"));
                }
                let (policy, rest) = parse_carry_policy(next)?;
                properties.carry = Some(policy);
                input = rest;
            }
            "send" => {
                return Err(next.error_here(
                    "type property `[send]` is retired; declare the required four-axis `[carry(...)]` policy",
                ));
            }
            "sized" => {
                return Err(next.error_here(
                    "type property `sized` is computed from the data shape and cannot be declared",
                ));
            }
            other => {
                return Err(next.error_here(format!(
                    "unknown type property `{other}`; declared properties are `copy`, `linear`, `zero_init`, `carry(...)`"
                )));
            }
        }

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
        break;
    }

    let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
    Ok((properties, input))
}

fn parse_carry_policy<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, omega_core::semantics::CarryPolicy> {
    use omega_core::semantics::{
        CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension,
    };

    let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let mut suspension = None;
    let mut cpu = None;
    let mut host_thread = None;
    let mut address = None;

    while !input.at_punctuation(PunctuationKind::RightParen) {
        let (axis, next) = input.take_identifier()?;
        let next = next.take_punctuation(PunctuationKind::Colon, ":")?;
        let (value, rest) = next.take_identifier()?;
        match axis.as_str() {
            "suspension" => {
                if suspension.is_some() {
                    return Err(rest.error_here("duplicate carry axis `suspension`"));
                }
                suspension = Some(match value.as_str() {
                    "forbidden" => CarrySuspension::Forbidden,
                    "allowed" => CarrySuspension::Allowed,
                    other => {
                        return Err(rest.error_here(format!(
                            "unknown carry suspension `{other}`; expected `forbidden` or `allowed`"
                        )));
                    }
                });
            }
            "cpu" => {
                if cpu.is_some() {
                    return Err(rest.error_here("duplicate carry axis `cpu`"));
                }
                cpu = Some(match value.as_str() {
                    "same" => CarryCpu::Origin,
                    "any" => CarryCpu::Any,
                    other => {
                        return Err(rest.error_here(format!(
                            "unknown carry CPU affinity `{other}`; expected `same` or `any`"
                        )));
                    }
                });
            }
            "thread" => {
                if host_thread.is_some() {
                    return Err(rest.error_here("duplicate carry axis `thread`"));
                }
                host_thread = Some(match value.as_str() {
                    "same" => CarryHostThread::Origin,
                    "any" => CarryHostThread::Any,
                    other => {
                        return Err(rest.error_here(format!(
                            "unknown carry host-thread affinity `{other}`; expected `same` or `any`"
                        )));
                    }
                });
            }
            "address" => {
                if address.is_some() {
                    return Err(rest.error_here("duplicate carry axis `address`"));
                }
                address = Some(match value.as_str() {
                    "stable" => CarryAddress::Stable,
                    "movable" => CarryAddress::Movable,
                    other => {
                        return Err(rest.error_here(format!(
                            "unknown carry address policy `{other}`; expected `stable` or `movable`"
                        )));
                    }
                });
            }
            other => {
                return Err(rest.error_here(format!(
                    "unknown carry axis `{other}`; expected `suspension`, `cpu`, `thread`, or `address`"
                )));
            }
        }
        input = rest;
        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
        break;
    }

    let missing = [
        ("suspension", suspension.is_none()),
        ("cpu", cpu.is_none()),
        ("thread", host_thread.is_none()),
        ("address", address.is_none()),
    ]
    .into_iter()
    .filter_map(|(axis, absent)| absent.then_some(axis))
    .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(input.error_here(format!(
            "carry policy must state all four axes; missing {}",
            missing.join(", ")
        )));
    }

    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    Ok((
        CarryPolicy {
            suspension: suspension.expect("checked above"),
            cpu: cpu.expect("checked above"),
            host_thread: host_thread.expect("checked above"),
            address: address.expect("checked above"),
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
        // Retire only the old `version Era { ... }` MEMBER shape. `version`
        // remains an ordinary identifier, so `version: u32;` must reach the
        // normal field parser below.
        let after_version = input.take_contextual("version")?;
        if after_version.at_name_like() {
            let (_, after_name) = after_version.take_identifier()?;
            if after_name.at_punctuation(PunctuationKind::LeftBrace) {
                return Err(input.error_here(
                    "data `version` blocks are retired; declare immutable era data types and an ordinary sum envelope",
                ));
            }
        }
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
        // Borrow-carrying data (decision 15 stage 2): a field may be a reference
        // (`body: &'buf string`). The borrow checker bounds the holding value's
        // lifetime by the borrowed source (see `checks::borrows::escape`).
        let (type_reference, next) =
            parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
        input = next;
        // Field defaults are RETIRED (owner ruling 2026-07-17): data
        // declarations carry no initializers -- ZII zero-initializes every
        // field, and constructed defaults belong in an ordinary constructor
        // machine. The spelling refuses LOUDLY here so an initializer can
        // never parse and then silently disappear (the old aggregate-literal
        // default bug class).
        if input.at_punctuation(PunctuationKind::Equal) {
            return Err(input.error_here(format!(
                "data field `{}` declares a default initializer -- field defaults \
                 are retired: every field is zero-initialized (ZII), and a \
                 constructed default belongs in an ordinary constructor machine \
                 that writes the field",
                field_name.as_str()
            )));
        }
        input = if input.at_punctuation(PunctuationKind::Semicolon) {
            input.take_punctuation(PunctuationKind::Semicolon, ";")?
        } else {
            input
        };
        return Ok((
            DataMember::Field(DataField {
                name: field_name,
                type_reference,
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
        // Case payloads may also carry borrows (decision 15 stage 2).
        let (type_reference, next) =
            parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
        input = next;

        let handle = syntax_trees.items.append_data_payload_field(DataField {
            name: field_name,
            type_reference,
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
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<TypeParameter>> {
    parse_type_parameters_in(syntax_trees, input, false)
}

pub(super) fn parse_machine_type_parameters<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<TypeParameter>> {
    parse_type_parameters_in(syntax_trees, input, true)
}

fn parse_type_parameters_in<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    allow_machine_parameters: bool,
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
        } else if input.at_keyword(omega_tokens::KeywordKind::Machine) {
            if !allow_machine_parameters {
                return Err(input.error_here(
                    "`<machine M>` is a static machine parameter and is only legal on a machine declaration",
                ));
            }
            let input = input.take_keyword(omega_tokens::KeywordKind::Machine, "machine")?;
            let (name, input) = input.take_identifier()?;
            (name, TypeParameterKind::Machine { contract: None }, input)
        } else {
            let (name, input) = input.take_identifier()?;
            (name, TypeParameterKind::Type, input)
        };
        input = next;

        // Rust-style `<T: copy>` is rejected with the bracket spelling
        // suggested: a colon bound would split the property spelling system.
        if matches!(kind, TypeParameterKind::Type) && input.at_punctuation(PunctuationKind::Colon) {
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
        if matches!(kind, TypeParameterKind::Machine { .. })
            && input.at_punctuation(PunctuationKind::LeftBracket)
        {
            return Err(input.error_here(
                "a machine parameter takes its callable contract in a mandatory `where machine M(...) -> Result` clause, not property brackets",
            ));
        }
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
