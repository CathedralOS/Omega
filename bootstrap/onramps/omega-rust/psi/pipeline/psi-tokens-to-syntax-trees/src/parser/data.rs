use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::type_reference::{
    parse_type_reference_handle, parse_type_reference_handle_allowing_borrow,
};
use psi_arena::{Handle, HandleSpan};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{
    DataDefinition, DataField, DataMember, DataProperties, DataVariant, QuotientDefinition,
    QuotientEquivalenceSelection, TypeParameter, TypeParameterKind,
};
use psi_tokens::{PunctuationKind, TokenKind};
use std::collections::HashSet;

/// A parsed `data` declaration: plain, or IDENTITY-NUMBERED (ch20 -- fields
/// carry optional identity numbers, `retired #N;` tombstones one; such a
/// declaration is the schema the identity-keyed grammars consume, and it
/// lowers through the wire-schema representation).
pub(super) enum ParsedDataDefinition {
    Plain(DataDefinition),
    Numbered(psi_syntax_trees::item::WireDataDefinition),
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
    // N7: proof-data families may be indexed by static machine symbols using
    // the same parameter/contract pair as generic machines. The selected
    // symbol is metadata only; no field stores a callable value.
    let (generic_parameters, next) = parse_machine_type_parameters(syntax_trees, input)?;
    input = next;
    let type_parameters = generic_parameters.type_parameters;
    let lifetime_parameters = generic_parameters.lifetime_parameters;
    let (properties, next) = parse_property_brackets(input)?;
    input = next;
    let ((), next) = crate::parser::machine::parse_machine_parameter_contracts(
        syntax_trees,
        type_parameters,
        input,
    )?;
    input = next;
    // N6: a quotient is the bodyless data form
    // `data Real = CauchySeq % converges_together;`. The carrier remains a
    // normal type reference (including a bare generic family); `%` is the
    // quotient former here rather than the expression-level modulo operator.
    if input.at_punctuation(PunctuationKind::Equal) {
        if properties != DataProperties::default() {
            return Err(input.error_here(
                "a quotient data declaration cannot declare runtime data properties; its values are proof-only equivalence classes",
            ));
        }
        input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (carrier, next) = parse_type_reference_handle(syntax_trees, input)?;
        input = next.take_punctuation(PunctuationKind::Percent, "%")?;
        let (relation, next) = parse_path_handle_span(input, |member| {
            syntax_trees.items.append_identifier_path_member(member)
        })?;
        input = next;
        let equivalence = if input.at_contextual("where") {
            input = input.take_contextual("where")?;
            let (selected_relation, next) = parse_path_handle_span(input, |member| {
                syntax_trees.items.append_identifier_path_member(member)
            })?;
            input = next.take_contextual("satisfies")?;
            let ((trait_name, trait_arguments), next) =
                crate::parser::item::parse_conformance_trait_application(syntax_trees, input)?;
            input = next.take_contextual("as")?;
            let (conformance_name, next) = input.take_identifier()?;
            input = next;
            Some(QuotientEquivalenceSelection {
                relation: selected_relation,
                trait_name,
                trait_arguments,
                conformance_name,
            })
        } else {
            None
        };
        input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            ParsedDataDefinition::Plain(DataDefinition {
                name,
                is_public: false,
                supply_mode: psi_language_core::DataSupplyMode::CheckedShape,
                lifetime_parameters,
                type_parameters,
                properties,
                quotient: Some(QuotientDefinition {
                    carrier,
                    relation,
                    equivalence,
                }),
                where_facts: HandleSpan::empty(),
                members: HandleSpan::empty(),
            }),
            input,
        ));
    }
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
    // `#` (`#1 seed: u64;`) or `retired` decides the form; numbers are
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
            .is_some_and(|inner| {
                inner.at_punctuation(PunctuationKind::Hash) || inner.at_contextual("retired")
            });
    if input.at_integer() {
        return Err(input.error_here(
            "the legacy numbered-field spelling `N: name: Type;` is retired; \
             write `#N name: Type;`",
        ));
    }
    let uses_legacy_wire_lowering = type_parameters.is_empty()
        && lifetime_parameters.is_empty()
        && properties == DataProperties::default()
        && where_facts.is_empty()
        && !body_contains_top_level_case(input);
    if uses_legacy_wire_lowering
        && (input.at_punctuation(PunctuationKind::Hash)
            || input.at_contextual("retired")
            || leading_version_is_numbered)
    {
        let (definition, input) =
            crate::parser::item::parse_identity_data_body(syntax_trees, name, input)?;
        return Ok((ParsedDataDefinition::Numbered(definition), input));
    }

    let (members, input) = parse_data_members(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    Ok((
        ParsedDataDefinition::Plain(DataDefinition {
            name,
            is_public: false,
            supply_mode: psi_language_core::DataSupplyMode::CheckedShape,
            lifetime_parameters,
            type_parameters,
            properties,
            quotient: None,
            where_facts,
            members,
        }),
        input,
    ))
}

fn body_contains_top_level_case(input: Input<'_, '_>) -> bool {
    let mut braces = 0usize;
    let mut brackets = 0usize;
    let mut parentheses = 0usize;

    for token in input.tokens {
        if token.is_non_semantic() {
            continue;
        }
        match token.punctuation() {
            Some(PunctuationKind::LeftBrace) => braces += 1,
            Some(PunctuationKind::RightBrace) if braces == 0 => break,
            Some(PunctuationKind::RightBrace) => braces -= 1,
            Some(PunctuationKind::LeftBracket) => brackets += 1,
            Some(PunctuationKind::RightBracket) => brackets = brackets.saturating_sub(1),
            Some(PunctuationKind::LeftParen) => parentheses += 1,
            Some(PunctuationKind::RightParen) => parentheses = parentheses.saturating_sub(1),
            _ if braces == 0
                && brackets == 0
                && parentheses == 0
                && matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_))
                && token.lexeme.as_str() == "case" =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
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
    let (generic_parameters, input) = parse_machine_type_parameters(syntax_trees, input)?;
    let type_parameters = generic_parameters.type_parameters;
    let lifetime_parameters = generic_parameters.lifetime_parameters;
    let (properties, input) = parse_property_brackets(input)?;
    let ((), input) = crate::parser::machine::parse_machine_parameter_contracts(
        syntax_trees,
        type_parameters,
        input,
    )?;
    if input.at_contextual("where") {
        return Err(input.error_here(
            "opaque `boundary data` has no visible fields for a default-domain `where` clause",
        ));
    }
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((
        DataDefinition {
            name,
            is_public: false,
            supply_mode: psi_language_core::DataSupplyMode::BoundaryOpaque,
            lifetime_parameters,
            type_parameters,
            properties,
            quotient: None,
            where_facts: HandleSpan::empty(),
            members: HandleSpan::empty(),
        },
        input,
    ))
}

/// Parse an optional declared-property bracket list. The same list attaches
/// to a data declaration (`data Point [copy] { ... }`) and to a type parameter
/// (`data Box<T [copy]>`) — brackets attach to what they follow, everywhere.
/// The property set is
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
                    psi_language_core::Multiplicity::Unrestricted
                } else {
                    psi_language_core::Multiplicity::Linear
                };
                input = next;
            }
            "zero_init" => {
                return Err(next.error_here(
                    "type property `[zero_init]` is retired; whether zeroed storage establishes this type is derived from its default domain and zero-case payload",
                ));
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
                    "unknown type property `{other}`; declared properties are `copy`, `linear`, `carry(...)`"
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
) -> ParseResult<'tokens, 'source, psi_language_core::CarryPolicy> {
    use psi_language_core::{
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
    validate_data_identity_modes(syntax_trees.items.data_members(members), input)?;
    Ok((members, input))
}

fn validate_data_identity_modes(
    members: &[DataMember],
    input: Input<'_, '_>,
) -> Result<(), crate::parse_error::ParseError> {
    let fields: Vec<Option<u64>> = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field.identity),
            _ => None,
        })
        .collect();
    let cases: Vec<Option<u64>> = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(case) => Some(case.identity),
            _ => None,
        })
        .collect();
    let retired: Vec<u64> = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Retired(identity) => Some(*identity),
            _ => None,
        })
        .collect();

    if !retired.is_empty() && !fields.is_empty() && !cases.is_empty() {
        return Err(input.error_here(
            "`retired #N;` is ambiguous in mixed field-and-case data; publish separate record \
             and sum shapes so each retired identity has one structural scope",
        ));
    }
    validate_identity_scope(
        "record fields",
        &fields,
        if cases.is_empty() { &retired } else { &[] },
        input,
    )?;
    validate_identity_scope(
        "sum cases",
        &cases,
        if fields.is_empty() { &retired } else { &[] },
        input,
    )
}

fn validate_identity_scope(
    scope: &str,
    identities: &[Option<u64>],
    retired: &[u64],
    input: Input<'_, '_>,
) -> Result<(), crate::parse_error::ParseError> {
    let numbered = identities
        .iter()
        .filter(|identity| identity.is_some())
        .count();
    if numbered > 0 && numbered != identities.len() {
        return Err(input.error_here(format!(
            "stable identities are all-or-nothing for {scope}: number every member with `#N` \
             or number none of them"
        )));
    }
    if !retired.is_empty() && identities.iter().any(Option::is_none) {
        return Err(input.error_here(format!(
            "`retired #N;` enters numbered mode for {scope}: every live member also needs `#N`"
        )));
    }

    let mut seen = HashSet::new();
    for identity in identities
        .iter()
        .flatten()
        .copied()
        .chain(retired.iter().copied())
    {
        if !seen.insert(identity) {
            return Err(input.error_here(format!(
                "stable identity #{identity} is declared more than once among {scope} and their retired identities"
            )));
        }
    }
    Ok(())
}

fn parse_data_member<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataMember> {
    if input.at_contextual("retired") {
        input = input.take_contextual("retired")?;
        input = input.take_punctuation(PunctuationKind::Hash, "#")?;
        let (identity, input) = input.take_identity()?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((DataMember::Retired(identity), input));
    }

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
        if after_case.at_name_like() || after_case.at_punctuation(PunctuationKind::Hash) {
            return parse_case_member(syntax_trees, after_case);
        }
    }

    let identity = if input.at_punctuation(PunctuationKind::Hash) {
        input = input.take_punctuation(PunctuationKind::Hash, "#")?;
        let (identity, next) = input.take_identity()?;
        input = next;
        Some(identity)
    } else {
        None
    };
    let (field_name, next) = input.take_identifier()?;
    let (relevance, next) = parse_field_relevance_brackets(next)?;
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
                identity,
                name: field_name,
                relevance,
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
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataMember> {
    let identity = if input.at_punctuation(PunctuationKind::Hash) {
        input = input.take_punctuation(PunctuationKind::Hash, "#")?;
        let (identity, next) = input.take_identity()?;
        input = next;
        Some(identity)
    } else {
        None
    };
    let (case_name, mut input) = input.take_identifier()?;

    let (payload, retired_payload_identities) = if input.at_punctuation(PunctuationKind::LeftParen)
    {
        let (payload, next) = parse_case_payload_fields(syntax_trees, input)?;
        input = next;
        payload
    } else {
        (HandleSpan::empty(), Vec::new())
    };

    input = if input.at_punctuation(PunctuationKind::Semicolon) {
        input.take_punctuation(PunctuationKind::Semicolon, ";")?
    } else {
        input
    };
    Ok((
        DataMember::Variant(DataVariant {
            identity,
            name: case_name,
            payload,
            retired_payload_identities,
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
) -> ParseResult<'tokens, 'source, (HandleSpan<DataField>, Vec<u64>)> {
    let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let mut payload_start = Handle::invalid();
    let mut payload_count = 0u32;

    let mut retired_identities = Vec::new();
    while !input.at_punctuation(PunctuationKind::RightParen) {
        if input.at_contextual("retired") {
            input = input.take_contextual("retired")?;
            input = input.take_punctuation(PunctuationKind::Hash, "#")?;
            let (identity, next) = input.take_identity()?;
            retired_identities.push(identity);
            input = next;
            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }
            break;
        }
        let identity = if input.at_punctuation(PunctuationKind::Hash) {
            input = input.take_punctuation(PunctuationKind::Hash, "#")?;
            let (identity, next) = input.take_identity()?;
            input = next;
            Some(identity)
        } else {
            None
        };
        let (field_name, next) = input.take_identifier()?;
        let (relevance, next) = parse_field_relevance_brackets(next)?;
        input = next.take_punctuation(PunctuationKind::Colon, ":")?;
        // Case payloads may also carry borrows (decision 15 stage 2).
        let (type_reference, next) =
            parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
        input = next;

        let handle = syntax_trees.items.append_data_payload_field(DataField {
            identity,
            name: field_name,
            relevance,
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
    let identities: Vec<Option<u64>> = syntax_trees
        .items
        .data_payload_fields(payload)
        .iter()
        .map(|field| field.identity)
        .collect();
    validate_identity_scope(
        "structured case-payload fields",
        &identities,
        &retired_identities,
        input,
    )?;
    Ok(((payload, retired_identities), input))
}

/// Parse the closed property set that attaches to one data-field binding.
///
/// Binding properties are intentionally distinct from data/type properties:
/// `proof [erased]: Evidence` marks only `proof`, never `Evidence` itself.
pub(super) fn parse_field_relevance_brackets<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, psi_language_core::BindingRelevance> {
    if !input.at_punctuation(PunctuationKind::LeftBracket) {
        return Ok((psi_language_core::BindingRelevance::Relevant, input));
    }

    let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
    let mut relevance = psi_language_core::BindingRelevance::Relevant;
    let mut declared_erased = false;
    while !input.at_punctuation(PunctuationKind::RightBracket) {
        let (name, next) = input.take_identifier()?;
        match name.as_str() {
            "erased" => {
                if declared_erased {
                    return Err(next.error_here("duplicate binding property `erased`"));
                }
                declared_erased = true;
                relevance = psi_language_core::BindingRelevance::Erased;
                input = next;
            }
            other => {
                return Err(next.error_here(format!(
                    "unknown data-field binding property `{other}`; declared binding properties are `erased`"
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
    Ok((relevance, input))
}

#[derive(Default)]
pub(super) struct ParsedGenericParameters {
    pub(super) lifetime_parameters: Vec<Identifier>,
    pub(super) type_parameters: HandleSpan<TypeParameter>,
    pub(super) conformance_bounds: Vec<psi_syntax_trees::item::GenericConformanceBound>,
}

pub(super) fn parse_type_parameters<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedGenericParameters> {
    parse_type_parameters_in(syntax_trees, input, false, false, false)
}

pub(super) fn parse_proposition_type_parameters<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedGenericParameters> {
    parse_type_parameters_in(syntax_trees, input, false, true, true)
}

pub(super) fn parse_machine_type_parameters<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedGenericParameters> {
    parse_type_parameters_in(syntax_trees, input, true, false, false)
}

pub(super) fn parse_machine_declaration_parameters<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedGenericParameters> {
    parse_type_parameters_in(syntax_trees, input, true, false, true)
}

fn parse_type_parameters_in<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    allow_machine_parameters: bool,
    allow_proposition_parameters: bool,
    allow_conformance_binders: bool,
) -> ParseResult<'tokens, 'source, ParsedGenericParameters> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok((ParsedGenericParameters::default(), input));
    }

    input = input.take_punctuation(PunctuationKind::Less, "<")?;
    let mut type_parameter_start = Handle::invalid();
    let mut type_parameter_count = 0u32;
    let mut lifetime_parameters = Vec::new();
    let mut conformance_bounds = Vec::new();
    let mut declared_names = Vec::<String>::new();
    let mut saw_runtime_parameter = false;

    loop {
        // A leading bracket is the attribute-prefix spelling, which decision
        // 13 rejects: brackets attach to what they FOLLOW.
        if input.at_punctuation(PunctuationKind::LeftBracket) {
            return Err(input.error_here(
                "property brackets attach to the name they follow: write the bounds after the type parameter, like `T [copy]`",
            ));
        }

        // A lifetime parameter (`<'buf>`); frozen decision 15 stage 2. It is
        // an erased borrow-region binder, stored separately from ordinary
        // type/const/machine parameters so runtime generic arity and
        // monomorphization never count it.
        if input.at_punctuation(PunctuationKind::Apostrophe) {
            if saw_runtime_parameter {
                return Err(input.error_here(
                    "lifetime parameters precede type, const, and machine parameters",
                ));
            }
            let after_tick = input.take_punctuation(PunctuationKind::Apostrophe, "'")?;
            let (lifetime_name, next) = after_tick.take_identifier()?;
            if declared_names
                .iter()
                .any(|declared| declared == lifetime_name.as_str())
            {
                return Err(next.error_here(format!(
                    "duplicate generic parameter `{}`",
                    lifetime_name.as_str()
                )));
            }
            declared_names.push(lifetime_name.as_str().to_owned());
            lifetime_parameters.push(lifetime_name);
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
            return Ok((
                ParsedGenericParameters {
                    lifetime_parameters,
                    type_parameters,
                    conformance_bounds,
                },
                input,
            ));
        }

        let (name, kind, next) = if input.at_contextual("const") {
            let input = input.take_contextual("const")?;
            let (name, input) = input.take_identifier()?;
            let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
            (name, TypeParameterKind::Const { type_reference }, input)
        } else if input.at_keyword(psi_tokens::KeywordKind::Machine) {
            if !allow_machine_parameters {
                return Err(input.error_here(
                    "`<machine M>` is a static machine parameter and is only legal on a machine or conformance declaration",
                ));
            }
            let input = input.take_keyword(psi_tokens::KeywordKind::Machine, "machine")?;
            let (name, input) = input.take_identifier()?;
            (name, TypeParameterKind::Machine { contract: None }, input)
        } else if input.at_contextual("proposition") {
            if !allow_proposition_parameters {
                return Err(input.error_here(
                    "`<proposition Relation>` is currently legal only on a trait declaration",
                ));
            }
            let input = input.take_contextual("proposition")?;
            let (name, input) = input.take_identifier()?;
            (
                name,
                TypeParameterKind::Proposition { contract: None },
                input,
            )
        } else {
            let (name, input) = input.take_identifier()?;
            (name, TypeParameterKind::Type, input)
        };
        saw_runtime_parameter = true;
        input = next;
        if declared_names
            .iter()
            .any(|declared| declared == name.as_str())
        {
            return Err(
                input.error_here(format!("duplicate generic parameter `{}`", name.as_str()))
            );
        }
        declared_names.push(name.as_str().to_owned());

        if matches!(kind, TypeParameterKind::Type)
            && allow_conformance_binders
            && input.at_punctuation(PunctuationKind::Colon)
        {
            let rest = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let (subject, rest) = rest.take_identifier()?;
            let rest = rest.take_contextual("satisfies")?;
            let (carrier, rest) = rest.take_identifier()?;
            let (arguments, rest) =
                crate::parser::machine::parse_optional_satisfies_type_arguments(
                    syntax_trees,
                    rest,
                )?;
            conformance_bounds.push(psi_syntax_trees::item::GenericConformanceBound {
                binder: Some(name),
                subject,
                carrier,
                arguments,
                conformance: None,
            });
            input = rest;

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
            return Ok((
                ParsedGenericParameters {
                    lifetime_parameters,
                    type_parameters,
                    conformance_bounds,
                },
                input,
            ));
        }

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
        if matches!(kind, TypeParameterKind::Proposition { .. })
            && input.at_punctuation(PunctuationKind::LeftBracket)
        {
            return Err(input.error_here(
                "a proposition parameter takes its signature in a mandatory `where proposition Name(...)` clause, not property brackets",
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
        return Ok((
            ParsedGenericParameters {
                lifetime_parameters,
                type_parameters,
                conformance_bounds,
            },
            input,
        ));
    }
}
