use crate::contracts::conformance::parse_conformance_trait_application;
use crate::contracts::facts::parse_required_logical_proof_facts_until;
use crate::parameters::generics::GenericParameterSyntax;
use crate::parameters::generics::parse_generic_parameters;
use crate::parameters::relevance::parse_binding_relevance_brackets;
use crate::parameters::requirements::parse_machine_parameter_contracts;
use crate::parameters::requirements::validate_machine_parameter_contracts;
use crate::token_input::cursor::{Input, ParseResult, parse_path_handle_span};
use crate::types::properties::parse_property_brackets;
use crate::types::references::{
    parse_type_reference_handle, parse_type_reference_handle_allowing_borrow,
};
use arena::HandleSpan;
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{
    DataDefinition, DataField, DataMember, DataVariant, Item, QuotientDefinition,
    QuotientEquivalenceSelection, TypeAliasDefinition,
};
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use tokens::PunctuationKind;

/*
    A data declaration and the data declarations nested in its body.

    `wiki/spec/language/data_and_literals.md`, "Records, sums and nested
    declarations": a nested declaration "introduces a name and nothing else"
    and is named `Owner::Name` elsewhere. The parser hoists each nested
    declaration to its own root item under that whole name, the way
    `domain Signal::Ready` is named, so every later stage sees an ordinary
    declaration. The owner comes first in the returned items.

    Inside the owner a nested name is visible on its own, "including before
    its text". Once an owner's text is parsed, each type name in it whose
    first segment names one of that owner's nested declarations is rewritten
    to the qualified name. Inner owners finish first, so the innermost
    declaration of a name wins, and a name an inner owner qualified is not
    qualified again by an enclosing one.
*/
pub(crate) fn parse_data_family<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<Item>> {
    let mut family = NestedData::default();
    let (owner, rest) = parse_data_definition(syntax_trees, input, None, &mut family)?;
    let mut items = Vec::with_capacity(family.declarations.len() + 1);
    items.push(owner);
    items.extend(family.declarations);
    Ok((items, rest))
}

/// The declarations nested anywhere in one top-level data declaration.
#[derive(Default)]
struct NestedData {
    declarations: Vec<Item>,
    /// Type names already given a nested declaration's qualified name.
    qualified: Vec<TypeReferenceHandle>,
}

fn parse_data_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    owner: Option<&str>,
    family: &mut NestedData,
) -> ParseResult<'tokens, 'source, Item> {
    let (leaf, mut input) = input.take_identifier()?;
    if leaf.as_str(syntax_trees) == "Slice" {
        return Err(input.error_here(
            "data name `Slice` is reserved: `Slice<T>` is the slice type (alias of `[T]`)",
        ));
    }
    let name = match owner {
        Some(owner) => Identifier::new(
            format!("{owner}::{}", leaf.as_str(syntax_trees)),
            leaf.source_span(),
        ),
        None => leaf,
    };
    let text_start = syntax_trees.type_references.node_count();
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
        let (carrier, next) = if input.at_keyword(tokens::KeywordKind::Machine) {
            let input = input.take_keyword(tokens::KeywordKind::Machine, "machine")?;
            if !input.at_punctuation(PunctuationKind::LeftParen)
                && !input.at_punctuation(PunctuationKind::Less)
            {
                return Err(input.error_here("expected the parameter list of a callable alias"));
            }
            let (signature, next) =
                crate::parameters::requirements::parse_structural_machine_contract(
                    syntax_trees,
                    name.clone(),
                    input,
                )?;
            if !next.at_punctuation(PunctuationKind::Semicolon) {
                return Err(next.error_here("expected `;` after the callable alias signature"));
            }
            let signature = syntax_trees.items.insert_state_signature(&signature);
            (
                syntax_trees
                    .type_references
                    .insert(syntax_trees::types::TypeReferenceNode::Callable { signature }),
                next,
            )
        } else {
            parse_type_reference_handle(syntax_trees, input)?
        };
        if next.at_punctuation(PunctuationKind::Semicolon) {
            // Typing checks an alias's asserted multiplicity. Cleanup belongs to its expansion.
            if cleanup_selection.is_some() {
                return Err(next.error_here(
                    "a transparent type alias inherits its expansion's cleanup; it may assert only `copy` or `linear`",
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
                    asserted_multiplicity: (properties.multiplicity
                        != language_core::Multiplicity::Affine)
                        .then_some(properties.multiplicity),
                }),
                next.take_punctuation(PunctuationKind::Semicolon, ";")?,
            ));
        }
        if properties.multiplicity == language_core::Multiplicity::Linear {
            return Err(next.error_here(
                "a quotient data declaration cannot declare linear multiplicity; the initial quotient surface excludes custody",
            ));
        }
        input = next.take_punctuation(PunctuationKind::Percent, "%")?;
        let (relation, next) = parse_path_handle_span(input, |member| {
            syntax_trees.items.append_identifier_path_member(member)
        })?;
        input = next;
        let equivalence = if input.at_contextual("requires") {
            input = input.take_contextual("requires")?;
            let (selected_relation, next) = parse_path_handle_span(input, |member| {
                syntax_trees.items.append_identifier_path_member(member)
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
    let mut default_domain_facts = arena::HandleSpan::empty();
    while input.at_contextual("requires") {
        input = input.take_contextual("requires")?;
        let (facts, rest) =
            parse_required_logical_proof_facts_until(syntax_trees, input, |input, _| {
                input.at_contextual("requires")
                    || input.at_punctuation(PunctuationKind::LeftBrace)
                    || input.tokens.is_empty()
            })?;
        default_domain_facts.extend_contiguous(facts);
        input = rest;
    }
    let requires_facts = default_domain_facts;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;

    if input.at_integer() {
        return Err(input.error_here(
            "the legacy numbered-field spelling `N: name: Type;` is retired; write `#N name: Type;`",
        ));
    }

    let qualified_name = name.as_str(syntax_trees).to_owned();
    let mut nested_names = Vec::new();
    let body = input;
    let (members, input) = parse_data_members(
        syntax_trees,
        input,
        &qualified_name,
        &mut nested_names,
        family,
    )?;
    // A nested declaration takes its owner's static parameters into its
    // identity, `Envelope<u32>::Body` (data_and_literals.md, "Generic
    // context"). Hoisting it under the bare owner name would give
    // `Envelope<u32>::Body` and `Envelope<bool>::Body` one identity.
    if !nested_names.is_empty() && (!type_parameters.is_empty() || !lifetime_parameters.is_empty())
    {
        return Err(body.error_here(
            "not supported yet: a data declaration nested in an owner with static parameters; \
             declare it at module scope and pass the parameters it uses",
        ));
    }
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    qualify_nested_names(
        syntax_trees,
        text_start,
        &qualified_name,
        &nested_names,
        family,
    );

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

// A retired identity starts with both tokens. The same word before a
// colon remains a field name in records and case payloads.
fn take_retired_identity<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Option<u64>> {
    if !input.at_contextual("retired") {
        return Ok((None, input));
    }
    let after_name = input.take_contextual("retired")?;
    if !after_name.at_punctuation(PunctuationKind::Hash) {
        return Ok((None, input));
    }
    let after_hash = after_name.take_punctuation(PunctuationKind::Hash, "#")?;
    let (identity, rest) = after_hash.take_identity()?;
    Ok((Some(identity), rest))
}

/*
    Rewrite the owner's type names that select one of its nested
    declarations. `text_start` is the type-reference frontier at the owner's
    name, so the scan covers its parameters, requirements and body, nested
    declarations included.
*/
fn qualify_nested_names(
    syntax_trees: &mut SyntaxTrees,
    text_start: syntax_trees::types::TypeReferenceFrontier,
    owner: &str,
    nested_names: &[Identifier],
    family: &mut NestedData,
) {
    if nested_names.is_empty() {
        return;
    }
    let nested: Vec<&str> = nested_names
        .iter()
        .map(|name| name.as_str(syntax_trees))
        .collect();
    let mut rewrites = Vec::new();
    for (handle, node) in syntax_trees.type_references.iter_type_references() {
        if !text_start.contains(handle) || family.qualified.contains(&handle) {
            continue;
        }
        let name = match node {
            TypeReferenceNode::Named(name) => name,
            TypeReferenceNode::Generic { base_name, .. } => base_name,
            _ => continue,
        };
        let spelled = name.as_str(syntax_trees);
        let head = spelled.split("::").next().unwrap_or(spelled);
        if !nested.contains(&head) {
            continue;
        }
        let qualified = Identifier::new(format!("{owner}::{spelled}"), name.source_span());
        let node = match node.clone() {
            TypeReferenceNode::Generic {
                lifetime_arguments,
                arguments,
                ..
            } => TypeReferenceNode::Generic {
                base_name: qualified,
                lifetime_arguments,
                arguments,
            },
            _ => TypeReferenceNode::Named(qualified),
        };
        rewrites.push((handle, node));
    }
    for (handle, node) in rewrites {
        syntax_trees
            .type_references
            .replace_type_reference(handle, node);
        family.qualified.push(handle);
    }
}

fn parse_data_members<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    owner: &str,
    nested_names: &mut Vec<Identifier>,
    family: &mut NestedData,
) -> ParseResult<'tokens, 'source, HandleSpan<DataMember>> {
    // A nested declaration appends its own members while this body is
    // parsed, so this body's members are appended together at its end.
    let mut parsed = Vec::new();
    let mut after_nested_declaration = false;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_keyword(tokens::KeywordKind::Public)
            && input
                .take_keyword(tokens::KeywordKind::Public, "public")?
                .at_keyword(tokens::KeywordKind::Data)
        {
            return Err(input.error_here(
                "a nested data declaration inherits its owner's visibility and takes no `public` of its own",
            ));
        }
        if input.at_keyword(tokens::KeywordKind::Data) {
            let after_data = input.take_keyword(tokens::KeywordKind::Data, "data")?;
            let (_, next) =
                parse_nested_data(syntax_trees, after_data, owner, nested_names, family)?;
            after_nested_declaration = true;
            input = next;
            continue;
        }
        // The spec writes nested declarations after the fields or cases.
        if after_nested_declaration {
            return Err(input.error_here(
                "a data member follows a nested data declaration; declare nested data after the fields or cases",
            ));
        }
        let (member, next) = parse_data_member(syntax_trees, input, owner, nested_names, family)?;
        input = next;
        parsed.push(member);
    }
    let mut members = arena::HandleSpan::empty();
    for member in parsed {
        members.push_contiguous(syntax_trees.items.append_data_member(member));
    }

    let members = members;
    validate_data_identity_modes(syntax_trees.items.data_members(members), input)?;
    Ok((members, input))
}

/// Parse a nested `data` declaration after its keyword and hoist it.
/// Returns the declaration's qualified name.
fn parse_nested_data<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    owner: &str,
    nested_names: &mut Vec<Identifier>,
    family: &mut NestedData,
) -> ParseResult<'tokens, 'source, Identifier> {
    let (leaf, _) = input.take_identifier()?;
    let (item, rest) = parse_data_definition(syntax_trees, input, Some(owner), family)?;
    let name = match &item {
        Item::Data(data) => data.name.clone(),
        Item::TypeAlias(alias) => alias.name.clone(),
        _ => unreachable!("a data declaration parses to data or a type alias"),
    };
    nested_names.push(leaf);
    family.declarations.push(item);
    Ok((name, rest))
}

/*
    `name: data Type { ... }` in a field or case-payload position is the
    member `name: Type` together with the nested declaration of `Type`.
    The inline form introduces no parameters of its own.
*/
fn parse_inline_nested_data<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    owner: &str,
    nested_names: &mut Vec<Identifier>,
    family: &mut NestedData,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    let declaration = input.take_keyword(tokens::KeywordKind::Data, "data")?;
    let (name, rest) = parse_nested_data(syntax_trees, declaration, owner, nested_names, family)?;
    let introduces_parameters = match family.declarations.last() {
        Some(Item::Data(data)) => {
            data.quotient.is_some()
                || !data.type_parameters.is_empty()
                || !data.lifetime_parameters.is_empty()
        }
        _ => true,
    };
    if introduces_parameters {
        return Err(declaration.error_here(
            "a type declared with its field takes no parameters and has a `{ ... }` body; declare it separately and apply it at the field",
        ));
    }
    let reference = syntax_trees.type_references.insert_named(name);
    family.qualified.push(reference);
    Ok((reference, rest))
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
    input: Input<'tokens, 'source>,
    owner: &str,
    nested_names: &mut Vec<Identifier>,
    family: &mut NestedData,
) -> ParseResult<'tokens, 'source, DataMember> {
    let (retired, after_retired) = take_retired_identity(input)?;
    if let Some(identity) = retired {
        let input = after_retired.take_punctuation(PunctuationKind::Semicolon, ";")?;
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
            return parse_case_member(
                syntax_trees,
                after_case,
                identity,
                owner,
                nested_names,
                family,
            );
        }
    }

    let (field_name, next) = input.take_identifier()?;
    let (relevance, next) = parse_binding_relevance_brackets(next, "data-field")?;
    input = next;

    if input.at_punctuation(PunctuationKind::Colon) {
        input = input.take_punctuation(PunctuationKind::Colon, ":")?;
        let (type_reference, next) = if input.at_keyword(tokens::KeywordKind::Data) {
            parse_inline_nested_data(syntax_trees, input, owner, nested_names, family)?
        } else {
            parse_type_reference_handle_allowing_borrow(syntax_trees, input)?
        };
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
    owner: &str,
    nested_names: &mut Vec<Identifier>,
    family: &mut NestedData,
) -> ParseResult<'tokens, 'source, DataMember> {
    let (case_name, mut input) = input.take_identifier()?;

    let (payload, retired_payload_identities) = if input.at_punctuation(PunctuationKind::LeftParen)
    {
        let (payload, next) =
            parse_case_payload_fields(syntax_trees, input, owner, nested_names, family)?;
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
    owner: &str,
    nested_names: &mut Vec<Identifier>,
    family: &mut NestedData,
) -> ParseResult<'tokens, 'source, (HandleSpan<DataField>, Vec<u64>)> {
    let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    // An inline nested declaration appends its own payload fields, so this
    // payload's fields are appended together at its end.
    let mut fields = Vec::new();

    let mut retired_identities = Vec::new();
    while !input.at_punctuation(PunctuationKind::RightParen) {
        let (retired, next) = take_retired_identity(input)?;
        if let Some(identity) = retired {
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
        // A type declared in a case payload is named under the enclosing
        // data; a case is not a scope.
        let (type_reference, next) = if input.at_keyword(tokens::KeywordKind::Data) {
            parse_inline_nested_data(syntax_trees, input, owner, nested_names, family)?
        } else {
            parse_type_reference_handle_allowing_borrow(syntax_trees, input)?
        };
        input = next;

        fields.push(DataField {
            identity,
            name: field_name,
            relevance,
            type_reference,
        });

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        } else {
            break;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let mut payload = arena::HandleSpan::empty();
    for field in fields {
        payload.push_contiguous(syntax_trees.items.append_data_payload_field(field));
    }
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
