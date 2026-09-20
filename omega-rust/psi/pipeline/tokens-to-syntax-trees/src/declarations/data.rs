use crate::input::token_cursor::{Input, ParseResult, parse_path_handle_span};
use crate::parameters::binding_properties::parse_binding_relevance_brackets;
use crate::parameters::parse_generic_parameters::GenericParameterSyntax;
use crate::parameters::parse_generic_parameters::parse_generic_parameters;
use crate::type_syntax::parse_type::{
    parse_type_reference_handle, parse_type_reference_handle_allowing_borrow,
};
use crate::type_syntax::properties::parse_property_brackets;
use arena::{Handle, HandleSpan};
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{
    DataDefinition, DataField, DataMember, DataProperties, DataVariant, QuotientDefinition,
    QuotientEquivalenceSelection,
};
use tokens::PunctuationKind;

pub(super) fn parse_data_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataDefinition> {
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
    let (generic_parameters, next) =
        parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::DataDeclaration)?;
    input = next;
    let type_parameters = generic_parameters.type_parameters;
    let lifetime_parameters = generic_parameters.lifetime_parameters;
    let (properties, next) = parse_property_brackets(input)?;
    input = next;
    let ((), next) = crate::parameters::contracts::parse_machine_parameter_contracts(
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
            let ((trait_name, trait_lifetime_arguments, trait_arguments), next) =
                crate::contracts::conformance::parse_conformance::parse_conformance_trait_application(
                    syntax_trees,
                    input,
                )?;
            if !trait_lifetime_arguments.is_empty() {
                return Err(next.error_here(
                    "quotient equivalence selection does not yet retain lifetime arguments; the application must remain lifetime-free",
                ));
            }
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
            DataDefinition {
                name,
                is_public: false,
                supply_mode: language_core::DataSupplyMode::CheckedShape,
                lifetime_parameters,
                type_parameters,
                generic_instance: None,
                properties,
                quotient: Some(QuotientDefinition {
                    carrier,
                    relation,
                    equivalence,
                }),
                where_facts: HandleSpan::empty(),
                members: HandleSpan::empty(),
            },
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
            crate::contracts::facts::parse_proof_facts_until(syntax_trees, input, |input| {
                input.at_punctuation(PunctuationKind::LeftBrace) || input.tokens.is_empty()
            })?;
        where_facts = facts;
        input = rest;
    }
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;

    if input.at_integer() {
        return Err(input.error_here(
            "the legacy numbered-field spelling `N: name: Type;` is retired; write `#N name: Type;`",
        ));
    }

    let (members, input) = parse_data_members(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    Ok((
        DataDefinition {
            name,
            is_public: false,
            supply_mode: language_core::DataSupplyMode::CheckedShape,
            lifetime_parameters,
            type_parameters,
            generic_instance: None,
            properties,
            quotient: None,
            where_facts,
            members,
        },
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
    let (generic_parameters, input) =
        parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::StaticBinders)?;
    let type_parameters = generic_parameters.type_parameters;
    let lifetime_parameters = generic_parameters.lifetime_parameters;
    let (properties, input) = parse_property_brackets(input)?;
    let ((), input) = crate::parameters::contracts::parse_machine_parameter_contracts(
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
            supply_mode: language_core::DataSupplyMode::BoundaryOpaque,
            lifetime_parameters,
            type_parameters,
            generic_instance: None,
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
) -> Result<(), crate::diagnostics::parse_error::ParseError> {
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
) -> Result<(), crate::diagnostics::parse_error::ParseError> {
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

    if input.at_contextual("reserved") {
        let after_reserved = input.take_contextual("reserved")?;
        if after_reserved.at_integer() {
            return Err(input.error_here(
                "`reserved` is retired: tombstone an identity number with `retired #N;`",
            ));
        }
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
    let (relevance, next) = parse_binding_relevance_brackets(next, "data-field")?;
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

    // CASE-CONSTRAINTS: a case-local `where` clause after the payload list --
    // `case Range(lo: u64, hi: u64) where lo <= hi;` -- names payload and
    // common fields; the fact list ends at the case's `;`.
    let mut where_facts = HandleSpan::empty();
    if input.at_contextual("where") {
        input = input.take_contextual("where")?;
        let ((facts, _token_count), rest) =
            crate::contracts::facts::parse_proof_facts_until(syntax_trees, input, |input| {
                input.at_punctuation(PunctuationKind::Semicolon) || input.tokens.is_empty()
            })?;
        where_facts = facts;
        input = rest;
    }

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
            where_facts,
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
        let (relevance, next) = parse_binding_relevance_brackets(next, "data-field")?;
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
