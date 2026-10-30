use crate::input::token_cursor::{Input, ParseResult, parse_path_handle_span};
use crate::parameters::binding_properties::parse_binding_relevance_brackets;
use crate::parameters::parse_generic_parameters::GenericParameterSyntax;
use crate::parameters::parse_generic_parameters::parse_generic_parameters;
use crate::type_syntax::parse_type::{
    parse_type_reference_handle, parse_type_reference_handle_allowing_borrow,
};
use crate::type_syntax::properties::parse_property_brackets;
use arena::HandleSpan;
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use syntax_trees::item::{
    DataDefinition, DataField, DataMember, DataVariant, ProofFact, QuotientDefinition,
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
    if name.as_str(syntax_trees) == "Slice" {
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
    let (cleanup_selection, next) = super::domain::parse_cleanup_selection(syntax_trees, input)?;
    input = next;
    // N6: a quotient is the bodyless data form
    // `data Real = CauchySeq % converges_together;`. The carrier remains a
    // normal type reference (including a bare generic family); `%` is the
    // quotient former here rather than the expression-level modulo operator.
    if input.at_punctuation(PunctuationKind::Equal) {
        if properties.multiplicity == language_core::Multiplicity::Linear
            || properties.carry.is_some()
        {
            return Err(input.error_here(
                "a quotient data declaration cannot declare linear or carry properties; the initial quotient surface excludes custody",
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
                cleanup_selection,
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
    // Both spellings still share the default-domain carrier during source
    // migration. Requires clauses conjoin in declaration order; no machine
    // entry contract or additional logical subject is introduced.
    let mut default_domain_facts = crate::handle_spans::HandleSpanBuilder::new();
    if input.at_contextual("where") {
        input = input.take_contextual("where")?;
        let ((facts, _token_count), rest) =
            crate::contracts::facts::parse_logical_proof_facts_until(
                syntax_trees,
                input,
                |input| input.at_punctuation(PunctuationKind::LeftBrace) || input.tokens.is_empty(),
            )?;
        default_domain_facts.push_span(facts);
        input = rest;
    }
    while input.at_contextual("requires") {
        input = input.take_contextual("requires")?;
        let ((facts, _token_count), rest) =
            crate::contracts::facts::parse_required_logical_proof_facts_until(
                syntax_trees,
                input,
                |input| {
                    input.at_contextual("requires")
                        || input.at_punctuation(PunctuationKind::LeftBrace)
                        || input.tokens.is_empty()
                },
            )?;
        default_domain_facts.push_span(facts);
        input = rest;
    }
    let where_facts = default_domain_facts.finish();
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;

    if input.at_integer() {
        return Err(input.error_here(
            "the legacy numbered-field spelling `N: name: Type;` is retired; write `#N name: Type;`",
        ));
    }

    let (members, input) = parse_data_members(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    project_case_predicate_facts(syntax_trees, &name, where_facts, members);

    Ok((
        DataDefinition {
            name,
            is_public: false,
            supply_mode: language_core::DataSupplyMode::CheckedShape,
            lifetime_parameters,
            type_parameters,
            generic_instance: None,
            properties,
            cleanup_selection,
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
    if name.as_str(syntax_trees) == "Slice" {
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
    let mut members = crate::handle_spans::HandleSpanBuilder::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (member, next) = parse_data_member(syntax_trees, input)?;
        input = next;
        members.push(syntax_trees.items.append_data_member(member));
    }

    let members = members.finish();
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

    let identity = if input.at_punctuation(PunctuationKind::Hash) {
        input = input.take_punctuation(PunctuationKind::Hash, "#")?;
        let (identity, next) = input.take_identity()?;
        input = next;
        Some(identity)
    } else {
        None
    };
    if input.at_contextual("case") {
        // `case` stays a legal field name. The declaration kind follows the
        // optional identity, while payload fields retain their own scope.
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

    // A bare `Name;` member (the pre-`case` variant spelling) is retired:
    // `case Name;` is the canonical alternative member.
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

    // CASE-CONSTRAINTS: a case-local `where` clause after the payload list --
    // `case Range(lo: u64, hi: u64) where lo <= hi;` -- names payload and
    // common fields; the fact list ends at the case's `;`. The
    // case-constraints-through-default-domain migration spells the same
    // proposition in the data's `requires` disjunction; both views land on
    // this `where_facts` span (see `project_case_predicate_facts`).
    let mut where_facts = HandleSpan::empty();
    if input.at_contextual("where") {
        input = input.take_contextual("where")?;
        let ((facts, _token_count), rest) =
            crate::contracts::facts::parse_logical_proof_facts_until(
                syntax_trees,
                input,
                |input| input.at_punctuation(PunctuationKind::Semicolon) || input.tokens.is_empty(),
            )?;
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
    let mut payload = crate::handle_spans::HandleSpanBuilder::new();

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

/// CASE-CONSTRAINTS (case-constraints-through-default-domain): the data
/// `requires` disjunction's `(self in Data::Case) && <guard>` branches carry
/// the same proposition a retired case-local `where` spelled. Project each
/// recognized branch's remaining conjuncts onto its named variant so the
/// resolved `where_facts` view stays populated for construction, matching and
/// coverage consumers once the authored clause retires.
fn project_case_predicate_facts(
    syntax_trees: &mut SyntaxTrees,
    data_name: &syntax_trees::identifier::Identifier,
    where_facts: arena::HandleSpan<ProofFact>,
    members: arena::HandleSpan<DataMember>,
) {
    if where_facts.is_empty() {
        return;
    }
    let data_name = data_name.as_str(syntax_trees).to_owned();
    let roots: Vec<ExpressionHandle> = syntax_trees
        .items
        .proof_facts(where_facts)
        .iter()
        .filter_map(|fact| match fact {
            ProofFact::Expression(root) => Some(*root),
            _ => None,
        })
        .collect();
    let mut extracted: Vec<(String, Vec<ExpressionHandle>)> = Vec::new();
    for root in roots {
        let mut disjuncts = Vec::new();
        flatten_or(syntax_trees, root, &mut disjuncts);
        for disjunct in disjuncts {
            project_disjunct_onto_case(syntax_trees, &data_name, disjunct, &mut extracted);
        }
    }
    if extracted.is_empty() {
        return;
    }
    let member_count = syntax_trees.items.data_members(members).len();
    for (case_name, fact_handles) in extracted {
        let member_index = (0..member_count).find(|index| {
            matches!(
                syntax_trees.items.data_members(members)[*index],
                DataMember::Variant(ref variant)
                    if variant.name.as_str(syntax_trees) == case_name
            )
        });
        let Some(member_index) = member_index else {
            continue;
        };
        let mut facts = crate::handle_spans::HandleSpanBuilder::new();
        // An authored clause and a projected branch may coexist while the
        // surface retires; the union preserves both.
        let authored: Vec<ProofFact> = match &syntax_trees.items.data_members(members)[member_index]
        {
            DataMember::Variant(variant) => variant
                .where_facts
                .handles()
                .map(|handle| syntax_trees.items.proof_fact(handle).clone())
                .collect(),
            _ => Vec::new(),
        };
        for fact in authored {
            facts.push(syntax_trees.items.append_proof_fact(fact));
        }
        for handle in fact_handles {
            facts.push(
                syntax_trees
                    .items
                    .append_proof_fact(ProofFact::Expression(handle)),
            );
        }
        let facts = facts.finish();
        if facts.is_empty() {
            continue;
        }
        if let DataMember::Variant(variant) =
            &mut syntax_trees.items.data_members_mut(members)[member_index]
        {
            variant.where_facts = facts;
        }
    }
}

/// One `||`-level disjunct of a `requires` fact: flatten its `&&` conjuncts,
/// recognize the case-membership atom `self in Data::Case` (or the unqualified
/// `self in Case`), and record the remaining conjuncts on that case. A
/// disjunct carrying zero or more than one recognized case marker is left to
/// the `requires` judgment alone.
fn project_disjunct_onto_case(
    syntax_trees: &mut SyntaxTrees,
    data_name: &str,
    disjunct: ExpressionHandle,
    extracted: &mut Vec<(String, Vec<ExpressionHandle>)>,
) {
    let mut atoms = Vec::new();
    flatten_and(syntax_trees, disjunct, &mut atoms);
    let mut case_atom: Option<String> = None;
    let mut kept = Vec::new();
    for atom in atoms {
        if let Some(case_name) = case_membership_name(syntax_trees, data_name, atom) {
            match &case_atom {
                None => {
                    case_atom = Some(case_name);
                    continue;
                }
                // A repeated marker for the same case is redundant; a marker
                // for another case makes attribution ambiguous, so the whole
                // disjunct stays with the requires judgment alone.
                Some(existing) if *existing == case_name => continue,
                Some(_) => return,
            }
        }
        kept.push(atom);
    }
    let Some(case_name) = case_atom else {
        return;
    };
    // A conjunct still bound on `self` (`self.method()`, a bare `self`
    // leaf, or a self member under a node the rewriter does not rebuild)
    // has no spelling in the variant's payload scope; it remains a
    // premise of the `requires` judgment alone.
    let mut kept: Vec<ExpressionHandle> = kept
        .into_iter()
        .map(|atom| rewrite_self_member_accesses(syntax_trees, atom))
        .collect();
    kept.retain(|atom| !mentions_self(syntax_trees, *atom));
    match extracted.iter_mut().find(|(name, _)| *name == case_name) {
        Some((_, handles)) => handles.extend(kept),
        None => extracted.push((case_name, kept)),
    }
}

/// Rewrite `self.<field>` member accesses inside an extracted case conjunct to
/// the bare payload-field spelling the retired case `where` clause produced:
/// `where lo <= hi` parsed `lo` as a one-member `Name` path while the
/// `requires` disjunct spells the same leaf `self.lo` as a `Member` access.
/// Rebuilding that leaf as `Name(lo)` presents every downstream `where_facts`
/// consumer the identical internal form. Subtrees without a `self` member
/// share their handles; only rewritten paths allocate.
fn rewrite_self_member_accesses(
    syntax: &mut SyntaxTrees,
    handle: ExpressionHandle,
) -> ExpressionHandle {
    let node = syntax.expressions.expression(handle).clone();
    let rewritten = match node {
        ExpressionNode::Member(member)
            if member.case_variant.is_none() && is_self_subject(syntax, member.receiver) =>
        {
            let first = syntax
                .expressions
                .append_identifier_path_member(member.member);
            ExpressionNode::Name(HandleSpan::from_parts(first, 1))
        }
        ExpressionNode::Member(member) => {
            let receiver = rewrite_self_member_accesses(syntax, member.receiver);
            if receiver == member.receiver {
                return handle;
            }
            ExpressionNode::Member(syntax_trees::expression::TableMemberExpression {
                receiver,
                ..member
            })
        }
        ExpressionNode::Binary(binary) => {
            let left = rewrite_self_member_accesses(syntax, binary.left);
            let right = rewrite_self_member_accesses(syntax, binary.right);
            if left == binary.left && right == binary.right {
                return handle;
            }
            ExpressionNode::Binary(syntax_trees::expression::TableBinaryExpression {
                left,
                right,
                ..binary
            })
        }
        ExpressionNode::Unary(unary) => {
            let operand = rewrite_self_member_accesses(syntax, unary.operand);
            if operand == unary.operand {
                return handle;
            }
            ExpressionNode::Unary(syntax_trees::expression::TableUnaryExpression {
                operand,
                ..unary
            })
        }
        ExpressionNode::Cast(cast) => {
            let value = rewrite_self_member_accesses(syntax, cast.value);
            if value == cast.value {
                return handle;
            }
            ExpressionNode::Cast(syntax_trees::expression::TableCastExpression {
                value,
                ..cast
            })
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = rewrite_self_member_accesses(syntax, indexed.collection);
            let index = rewrite_self_member_accesses(syntax, indexed.index);
            if collection == indexed.collection && index == indexed.index {
                return handle;
            }
            ExpressionNode::Indexed(syntax_trees::expression::TableIndexedExpression {
                collection,
                index,
            })
        }
        ExpressionNode::Membership(membership) => {
            let value = rewrite_self_member_accesses(syntax, membership.value);
            if value == membership.value {
                return handle;
            }
            ExpressionNode::Membership(
                syntax_trees::expression::TableMembershipExpression { value, ..membership },
            )
        }
        ExpressionNode::Borrow(borrow) => {
            let target = rewrite_self_member_accesses(syntax, borrow.target);
            if target == borrow.target {
                return handle;
            }
            ExpressionNode::Borrow(syntax_trees::expression::TableBorrowExpression {
                target,
                ..borrow
            })
        }
        ExpressionNode::Range(range) => {
            let start = rewrite_self_member_accesses(syntax, range.start);
            let end = rewrite_self_member_accesses(syntax, range.end);
            if start == range.start && end == range.end {
                return handle;
            }
            ExpressionNode::Range(syntax_trees::expression::TableRangeExpression {
                start,
                end,
                ..range
            })
        }
        ExpressionNode::Call(call) => {
            let receiver = rewrite_self_member_accesses(syntax, call.receiver);
            let arguments = syntax
                .expressions
                .expression_handles(call.arguments)
                .to_vec();
            let mut changed = receiver != call.receiver;
            let rewritten: Vec<ExpressionHandle> = arguments
                .into_iter()
                .map(|argument| {
                    let rewritten = rewrite_self_member_accesses(syntax, argument);
                    changed |= rewritten != argument;
                    rewritten
                })
                .collect();
            if !changed {
                return handle;
            }
            ExpressionNode::Call(syntax_trees::expression::TableCallExpression {
                receiver,
                arguments: syntax.expressions.insert_expression_handles(rewritten),
                ..call
            })
        }
        _ => return handle,
    };
    let source_span = syntax.expressions.source_span(handle);
    let rewritten_handle = syntax.expressions.insert(rewritten);
    syntax
        .expressions
        .set_source_span(rewritten_handle, source_span);
    rewritten_handle
}

/// The declared-case name a membership atom binds: `self in Data::Case` or
/// `self in Case` where the last domain segment is a variant of this data.
/// Domain memberships (`self in Data::Admitted`) never name a declared case,
/// so they are left as ordinary conjuncts.
fn case_membership_name(
    syntax_trees: &SyntaxTrees,
    data_name: &str,
    atom: ExpressionHandle,
) -> Option<String> {
    let ExpressionNode::Membership(membership) = syntax_trees.expressions.expression(atom) else {
        return None;
    };
    if !is_self_subject(syntax_trees, membership.value) {
        return None;
    }
    let domain = syntax_trees
        .expressions
        .identifier_path_members(membership.domain);
    match domain {
        [case_name] => Some(case_name.as_str(syntax_trees).to_owned()),
        [head, case_name] if head.as_str(syntax_trees) == data_name => {
            Some(case_name.as_str(syntax_trees).to_owned())
        }
        _ => None,
    }
}

fn is_self_subject(syntax_trees: &SyntaxTrees, value: ExpressionHandle) -> bool {
    let ExpressionNode::Name(path) = syntax_trees.expressions.expression(value) else {
        return false;
    };
    matches!(
        syntax_trees.expressions.identifier_path_members(*path),
        [name] if name.as_str(syntax_trees) == "self"
    )
}

fn flatten_or(
    syntax_trees: &SyntaxTrees,
    handle: ExpressionHandle,
    out: &mut Vec<ExpressionHandle>,
) {
    if let ExpressionNode::Binary(binary) = syntax_trees.expressions.expression(handle) {
        if binary.operator == BinaryOperator::Or {
            flatten_or(syntax_trees, binary.left, out);
            flatten_or(syntax_trees, binary.right, out);
            return;
        }
    }
    out.push(handle);
}

fn flatten_and(
    syntax_trees: &SyntaxTrees,
    handle: ExpressionHandle,
    out: &mut Vec<ExpressionHandle>,
) {
    if let ExpressionNode::Binary(binary) = syntax_trees.expressions.expression(handle) {
        if binary.operator == BinaryOperator::And {
            flatten_and(syntax_trees, binary.left, out);
            flatten_and(syntax_trees, binary.right, out);
            return;
        }
    }
    out.push(handle);
}

/// Whether the subtree still binds the `self` receiver -- a bare `self` name
/// or a `self.<member>` access the caller declined to rewrite.
fn mentions_self(syntax: &SyntaxTrees, handle: ExpressionHandle) -> bool {
    let node = syntax.expressions.expression(handle);
    if is_self_subject(syntax, handle) {
        return true;
    }
    match node {
        ExpressionNode::Member(member) => mentions_self(syntax, member.receiver),
        ExpressionNode::Binary(binary) => {
            mentions_self(syntax, binary.left) || mentions_self(syntax, binary.right)
        }
        ExpressionNode::Unary(unary) => mentions_self(syntax, unary.operand),
        ExpressionNode::Cast(cast) => mentions_self(syntax, cast.value),
        ExpressionNode::Indexed(indexed) => {
            mentions_self(syntax, indexed.collection) || mentions_self(syntax, indexed.index)
        }
        ExpressionNode::Membership(membership) => mentions_self(syntax, membership.value),
        ExpressionNode::Borrow(borrow) => mentions_self(syntax, borrow.target),
        ExpressionNode::Range(range) => {
            mentions_self(syntax, range.start) || mentions_self(syntax, range.end)
        }
        ExpressionNode::Call(call) => {
            mentions_self(syntax, call.receiver)
                || syntax
                    .expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| mentions_self(syntax, *argument))
        }
        _ => false,
    }
}
