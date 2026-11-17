use crate::contracts::conformance::parse_conformance_trait_application;
use crate::contracts::facts::parse_required_logical_proof_facts_until;
use crate::parameters::generics::GenericParameterSyntax;
use crate::parameters::generics::parse_generic_parameters;
use crate::parameters::relevance::parse_binding_relevance_brackets;
use crate::parameters::requirements::parse_machine_parameter_contracts;
use crate::parameters::requirements::validate_machine_parameter_contracts;
use crate::syntax_storage::tables::{items, items_of};
use crate::token_input::cursor::{Input, ParseResult, parse_path_handle_span};
use crate::types::properties::parse_property_brackets;
use crate::types::references::{
    parse_type_reference_handle, parse_type_reference_handle_allowing_borrow,
};
use arena::HandleSpan;
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{
    DataDefinition, DataField, DataMember, DataVariant, Item, QuotientDefinition,
    QuotientEquivalenceSelection, TypeAliasDefinition,
};
use tokens::PunctuationKind;

pub(crate) fn parse_data_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Item> {
    let (name, mut input) = input.take_identifier()?;
    if name.as_str(syntax_trees) == "Slice" {
        return Err(input.error_here(
            "data name `Slice` is reserved: `Slice<T>` is the slice type (alias of `[T]`)",
        ));
    }
    let (generic_parameters, next) =
        parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::DataDeclaration)?;
    input = next;
    let type_parameters = generic_parameters.type_parameters;
    let lifetime_parameters = generic_parameters.lifetime_parameters;
    let (properties, next) = parse_property_brackets(input)?;
    input = next;
    let ((), next) = parse_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
    validate_machine_parameter_contracts(syntax_trees, type_parameters, next)?;
    input = next;
    let (cleanup_selection, next) =
        crate::declarations::domains::parse_cleanup_selection(syntax_trees, input)?;
    input = next;
    if input.at_punctuation(PunctuationKind::Equal) {
        input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (carrier, next) = parse_type_reference_handle(syntax_trees, input)?;
        if next.at_punctuation(PunctuationKind::Semicolon) {
            if properties.multiplicity != language_core::Multiplicity::Affine
                || properties.carry.is_some()
                || cleanup_selection.is_some()
            {
                return Err(next.error_here(
                    "a transparent type alias inherits its expansion's properties and cleanup; it cannot declare storage properties",
                ));
            }
            return Ok((
                Item::TypeAlias(TypeAliasDefinition {
                    name,
                    is_public: false,
                    lifetime_parameters,
                    type_parameters,
                    conformance_bounds: generic_parameters.conformance_bounds,
                    authored_expansion: carrier,
                    expansion: carrier,
                }),
                next.take_punctuation(PunctuationKind::Semicolon, ";")?,
            ));
        }
        if properties.multiplicity == language_core::Multiplicity::Linear
            || properties.carry.is_some()
        {
            return Err(next.error_here(
                "a quotient data declaration cannot declare linear or carry properties; the initial quotient surface excludes custody",
            ));
        }
        input = next.take_punctuation(PunctuationKind::Percent, "%")?;
        let (relation, next) = parse_path_handle_span(input, |member| {
            items(syntax_trees).append_identifier_path_member(member)
        })?;
        input = next;
        let equivalence = if input.at_contextual("requires") {
            input = input.take_contextual("requires")?;
            let (selected_relation, next) = parse_path_handle_span(input, |member| {
                items(syntax_trees).append_identifier_path_member(member)
            })?;
            input = next.take_contextual("satisfies")?;
            let ((trait_name, trait_lifetime_arguments, trait_arguments), next) =
                parse_conformance_trait_application(syntax_trees, input)?;
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
            Item::Data(DataDefinition {
                name,
                is_public: false,
                supply_mode: language_core::DataSupplyMode::CheckedShape,
                lifetime_parameters,
                type_parameters,
                conformance_bounds: generic_parameters.conformance_bounds,
                generic_instance: None,
                properties,
                cleanup_selection,
                quotient: Some(QuotientDefinition {
                    carrier,
                    relation,
                    equivalence,
                }),
                requires_facts: HandleSpan::empty(),
                members: HandleSpan::empty(),
            }),
            input,
        ));
    }
    let mut default_domain_facts = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();
    while input.at_contextual("requires") {
        input = input.take_contextual("requires")?;
        let ((facts, _token_count), rest) =
            parse_required_logical_proof_facts_until(syntax_trees, input, |input| {
                input.at_contextual("requires")
                    || input.at_punctuation(PunctuationKind::LeftBrace)
                    || input.tokens.is_empty()
            })?;
        default_domain_facts.push_span(facts);
        input = rest;
    }
    let requires_facts = default_domain_facts.finish();
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;

    if input.at_integer() {
        return Err(input.error_here(
            "the legacy numbered-field spelling `N: name: Type;` is retired; write `#N name: Type;`",
        ));
    }

    let (members, input) = parse_data_members(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    Ok((
        Item::Data(DataDefinition {
            name,
            is_public: false,
            supply_mode: language_core::DataSupplyMode::CheckedShape,
            lifetime_parameters,
            type_parameters,
            conformance_bounds: generic_parameters.conformance_bounds,
            generic_instance: None,
            properties,
            cleanup_selection,
            quotient: None,
            requires_facts,
            members,
        }),
        input,
    ))
}

pub(crate) fn parse_boundary_data_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataDefinition> {
    let (name, input) = input.take_identifier()?;
    if name.as_str(syntax_trees) == "Slice" {
        return Err(input.error_here(
            "data name `Slice` is reserved: `Slice<T>` is the slice type (alias of `[T]`)",
        ));
    }
    let (generic_parameters, input) =
        parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::DataDeclaration)?;
    let type_parameters = generic_parameters.type_parameters;
    let lifetime_parameters = generic_parameters.lifetime_parameters;
    let (properties, input) = parse_property_brackets(input)?;
    let ((), input) = parse_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
    validate_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
    if input.at_contextual("requires") {
        return Err(input.error_here(
            "opaque `boundary data` has no visible fields for default-domain predicates",
        ));
    }
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((
        DataDefinition {
            name,
            is_public: false,
            cleanup_selection: None,
            supply_mode: language_core::DataSupplyMode::BoundaryOpaque,
            lifetime_parameters,
            type_parameters,
            conformance_bounds: generic_parameters.conformance_bounds,
            generic_instance: None,
            properties,
            quotient: None,
            requires_facts: HandleSpan::empty(),
            members: HandleSpan::empty(),
        },
        input,
    ))
}

fn parse_data_members<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<DataMember>> {
    let mut members = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (member, next) = parse_data_member(syntax_trees, input)?;
        input = next;
        members.push(items(syntax_trees).append_data_member(member));
    }

    let members = members.finish();
    validate_data_identity_modes(items_of(syntax_trees).data_members(members), input)?;
    Ok((members, input))
}

fn validate_data_identity_modes(
    members: &[DataMember],
    input: Input<'_, '_>,
) -> Result<(), crate::errors::parse_error::ParseError> {
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
) -> Result<(), crate::errors::parse_error::ParseError> {
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

fn take_optional_identity<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Option<u64>> {
    if !input.at_punctuation(PunctuationKind::Hash) {
        return Ok((None, input));
    }
    let after_hash = input.take_punctuation(PunctuationKind::Hash, "#")?;
    let (identity, rest) = after_hash.take_identity()?;
    Ok((Some(identity), rest))
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

    let (identity, mut input) = take_optional_identity(input)?;
    if input.at_contextual("case") {
        let after_case = input.take_contextual("case")?;
        if after_case.at_punctuation(PunctuationKind::Hash) {
            return Err(after_case
                .error_here("numbered cases are spelled `#N case Name`, not `case #N Name`"));
        }
        if after_case.at_punctuation(PunctuationKind::LeftBrace) {
            return Err(after_case.error_here(
                "case declaration blocks are not supported; declare each `case Name` separately",
            ));
        }
        if after_case.at_name_like() {
            return parse_case_member(syntax_trees, after_case, identity);
        }
    }

    let (field_name, next) = input.take_identifier()?;
    let (relevance, next) = parse_binding_relevance_brackets(next, "data-field")?;
    input = next;

    if input.at_punctuation(PunctuationKind::Colon) {
        input = input.take_punctuation(PunctuationKind::Colon, ":")?;
        let (type_reference, next) =
            parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
        input = next;
        if input.at_punctuation(PunctuationKind::Equal) {
            return Err(input.error_here(format!(
                "data field `{}` declares a default initializer -- field defaults \
                 are retired: every field is zero-initialized (ZII), and a \
                 constructed default belongs in an ordinary constructor machine \
                 that writes the field",
                field_name.as_str(syntax_trees)
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

    Err(input.error_here(format!(
        "expected `:` after data field `{}` (alternatives are spelled `case {};`)",
        field_name.as_str(syntax_trees),
        field_name.as_str(syntax_trees)
    )))
}

fn parse_case_member<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    identity: Option<u64>,
) -> ParseResult<'tokens, 'source, DataMember> {
    let (case_name, mut input) = input.take_identifier()?;

    let (payload, retired_payload_identities) = if input.at_punctuation(PunctuationKind::LeftParen)
    {
        let (payload, next) = parse_case_payload_fields(syntax_trees, input)?;
        input = next;
        payload
    } else {
        (HandleSpan::empty(), Vec::new())
    };

    if input.at_contextual("requires") {
        return Err(input.error_here(
            "case-local predicate clauses are retired; write the predicate in the data declaration's default-domain `requires`",
        ));
    }
    let requires_facts = HandleSpan::empty();

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
            requires_facts,
            retired_payload_identities,
        }),
        input,
    ))
}

fn parse_case_payload_fields<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, (HandleSpan<DataField>, Vec<u64>)> {
    let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let mut payload = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();

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
        let (identity, after_identity) = take_optional_identity(input)?;
        let (field_name, next) = after_identity.take_identifier()?;
        let (relevance, next) = parse_binding_relevance_brackets(next, "data-field")?;
        input = next.take_punctuation(PunctuationKind::Colon, ":")?;
        let (type_reference, next) =
            parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
        input = next;

        let handle = items(syntax_trees).append_data_payload_field(DataField {
            identity,
            name: field_name,
            relevance,
            type_reference,
        });
        payload.push(handle);

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        } else {
            break;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let payload = payload.finish();
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
