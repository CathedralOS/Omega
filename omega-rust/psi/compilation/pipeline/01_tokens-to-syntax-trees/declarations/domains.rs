use crate::contracts::facts::parse_required_logical_proof_facts_until;
use crate::expressions::postfix::try_parse_static_symbol_application;
use crate::parameters::generics::GenericParameterSyntax;
use crate::parameters::generics::parse_generic_parameters;
use crate::syntax_storage::tables::{exprs, items, type_refs_of};
use crate::token_input::cursor::{Input, ParseResult, parse_path_handle_span};
use crate::types::references::parse_domain_argument_handles;
use crate::types::references::parse_named_type_reference;
use arena::HandleSpan;
use language_core::DomainPredicatePresence;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{DomainAliasDefinition, DomainDefinition, ProofFact, ProofMembershipFact};
use syntax_trees::types::TypeReferenceHandle;
use syntax_trees::types::TypeReferenceNode;
use tokens::{KeywordKind, PunctuationKind};

pub(crate) fn parse_domain_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DomainDefinition> {
    let (generic_parameters, input) =
        parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::TypeAndConst)?;
    if !generic_parameters.lifetime_parameters.is_empty() {
        return Err(input.error_here(
            "domain families take a carrier type and proof-static const parameters, not lifetime parameters",
        ));
    }
    let (target_type, target_label, target_identifier, input) =
        if input.at_punctuation(PunctuationKind::LeftBracket) {
            let (handle, input) =
                crate::types::references::parse_type_reference_handle(syntax_trees, input)?;
            let label = type_reference_target_label(syntax_trees, handle);
            (handle, label, None, input)
        } else {
            let (target_name, input) = input.take_identifier()?;
            let (handle, input) =
                parse_named_type_reference(syntax_trees, target_name.clone(), input, false)?;
            (
                handle,
                target_name.as_str(syntax_trees).to_owned(),
                Some(target_name),
                input,
            )
        };
    let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
    let (first_segment, mut input) = input.take_identifier()?;
    let (first_arguments, rest) = parse_domain_argument_handles(syntax_trees, input)?;
    let mut name_segments = vec![(first_segment, first_arguments)];
    input = rest;
    while input.at_punctuation(PunctuationKind::ColonColon) {
        let rest = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (segment, rest) = rest.take_identifier()?;
        let (arguments, rest) = parse_domain_argument_handles(syntax_trees, rest)?;
        name_segments.push((segment, arguments));
        input = rest;
    }
    let domain_name = name_segments
        .last()
        .expect("a domain chain always holds its first segment")
        .0
        .clone();
    let index_arguments = name_segments
        .last()
        .expect("a domain chain always holds its first segment")
        .1;
    if input.at_punctuation(PunctuationKind::LeftParen) {
        return Err(input.error_here(
            "explicit domain subject binders are retired; use `self` in the domain's `requires` predicates",
        ));
    }
    let ((classification, classification_token_count), input) = parse_domain_classification(input)?;
    let (cleanup_selection, input) = parse_cleanup_selection(syntax_trees, input)?;
    let refinement_parent = if name_segments.len() > 1 {
        let parent_span = name_segments[name_segments.len() - 2].0.source_span();
        let subject_path = HandleSpan::from_parts(
            exprs(syntax_trees).append_identifier_path_member(Identifier::generated("self")),
            1,
        );
        let parent_path = items(syntax_trees).insert_identifier_path_members(
            target_identifier.iter().cloned().chain(
                name_segments[..name_segments.len() - 1]
                    .iter()
                    .map(|(segment, _)| segment.clone()),
            ),
        );
        Some((
            ProofMembershipFact {
                value: syntax_trees.expressions.insert(ExpressionNode::Name {
                    members: subject_path,
                    application: None,
                }),
                domain: parent_path,
                domain_arguments: name_segments[name_segments.len() - 2].1,
            },
            parent_span,
        ))
    } else {
        None
    };
    let name = if generic_parameters.type_parameters.is_empty() {
        Identifier::new(
            format!(
                "{target_label}::{}",
                name_segments
                    .iter()
                    .map(|(segment, _)| segment.as_str(syntax_trees))
                    .collect::<Vec<_>>()
                    .join("::")
            ),
            domain_name.source_span(),
        )
    } else {
        Identifier::new(
            name_segments
                .iter()
                .map(|(segment, _)| segment.as_str(syntax_trees))
                .collect::<Vec<_>>()
                .join("::"),
            domain_name.source_span(),
        )
    };
    let (
        alias,
        (authored_routes, access_policy),
        predicate_presence,
        facts,
        operators,
        semantic_clause_token_count,
        input,
    ) = if input.at_punctuation(PunctuationKind::Equal) {
        if refinement_parent.is_some() {
            return Err(input.error_here(
                "a domain refinement chain cannot be an alias; write the parent membership in the alias explicitly",
            ));
        }
        let (alias, input) = parse_domain_alias(syntax_trees, input)?;
        (
            Some(alias),
            (Vec::new(), None),
            DomainPredicatePresence::Absent,
            HandleSpan::empty(),
            HandleSpan::empty(),
            0,
            input,
        )
    } else {
        let parent_fact = refinement_parent.map(|(membership, parent_span)| {
            let handle = syntax_trees
                .items
                .append_proof_fact(ProofFact::Membership(membership));
            items(syntax_trees).set_proof_fact_source_span(handle, parent_span);
            handle
        });
        let ((predicate_presence, authored_facts, requires_token_count), input) =
            parse_domain_requires(syntax_trees, input)?;
        let (predicate_presence, facts) = match parent_fact {
            Some(handle) => (
                DomainPredicatePresence::Present,
                HandleSpan::from_parts(handle, 1 + authored_facts.count()),
            ),
            None => (predicate_presence, authored_facts),
        };
        let ((issuance, semantic_clause_token_count), input) =
            parse_domain_establishment(syntax_trees, input, predicate_presence.is_present())?;
        (
            None,
            issuance,
            predicate_presence,
            facts,
            HandleSpan::empty(),
            requires_token_count.saturating_add(semantic_clause_token_count),
            input,
        )
    };

    Ok((
        DomainDefinition {
            name,
            type_parameters: generic_parameters.type_parameters,
            target_type,
            index_arguments,
            is_public: false,
            alias,
            authored_routes,
            access_policy,
            cleanup_selection,
            classification,
            predicate_presence,
            facts,
            operators,
            semantic_clause_token_count: classification_token_count
                .saturating_add(semantic_clause_token_count),
        },
        input,
    ))
}

fn parse_domain_classification<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, (Option<language_core::DomainClassification>, usize)> {
    if !input.at_contextual("satisfies") {
        return Ok(((None, 0), input));
    }
    let start_tokens = input.tokens.len();
    let input = input.take_contextual("satisfies")?;
    let (name, input) = input.take_identifier()?;
    let classification = match name.as_str(&input) {
        "ProgressProfile" => language_core::DomainClassification::ProgressProfile,
        other => {
            return Err(input.error_here(format!(
                "unknown compiler-owned domain classification `{other}`; expected `ProgressProfile`"
            )));
        }
    };
    if input.at_contextual("satisfies") {
        return Err(
            input.error_here("a domain may select at most one compiler-owned classification")
        );
    }
    Ok((
        (
            Some(classification),
            start_tokens.saturating_sub(input.tokens.len()),
        ),
        input,
    ))
}

fn parse_domain_requires<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, (DomainPredicatePresence, HandleSpan<ProofFact>, usize)> {
    if !input.at_contextual("requires") {
        return Ok((
            (DomainPredicatePresence::Absent, HandleSpan::empty(), 0),
            input,
        ));
    }

    let input = input.take_contextual("requires")?;
    let ((facts, token_count), input) =
        parse_required_logical_proof_facts_until(syntax_trees, input, domain_requires_terminator)?;
    Ok((
        (DomainPredicatePresence::Present, facts, token_count),
        input,
    ))
}

fn domain_requires_terminator(input: Input<'_, '_>) -> bool {
    crate::declarations::items::named_implementation_prefix(input).is_some()
        || input.at_contextual("established")
        || input.at_contextual("access")
        || input.at_contextual("satisfies")
        || input.at_punctuation(PunctuationKind::LeftBrace)
        || input.tokens.is_empty()
        || input.at_keyword(KeywordKind::Public)
        || input.at_keyword(KeywordKind::Data)
        || input.at_keyword(KeywordKind::Machine)
        || crate::declarations::items::starts_machine_declaration(input)
        || input.at_keyword(KeywordKind::Use)
        || input.at_contextual("domain")
        || input.at_contextual("operator")
        || input.at_contextual("boundary")
        || input.at_contextual("trait")
        || input.at_contextual("const")
        || input.at_contextual("export")
        || input.at_contextual("module")
        || input.at_contextual("package")
}

fn parse_domain_alias<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DomainAliasDefinition> {
    let mut input = input.take_punctuation(PunctuationKind::Equal, "=")?;
    let mut constituents = Vec::new();

    loop {
        let (domain, rest) = parse_path_handle_span(input, |member| {
            items(syntax_trees).append_identifier_path_member(member)
        })?;
        constituents.push(domain);
        if !rest.at_punctuation(PunctuationKind::Ampersand) {
            input = rest;
            break;
        }
        input = rest.take_punctuation(PunctuationKind::Ampersand, "&")?;
    }

    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((DomainAliasDefinition { constituents }, input))
}

fn type_reference_target_label(syntax_trees: &SyntaxTrees, handle: TypeReferenceHandle) -> String {
    match type_refs_of(syntax_trees).type_reference(handle) {
        TypeReferenceNode::Slice { element_type } => {
            format!(
                "[{}]",
                type_reference_target_label(syntax_trees, *element_type)
            )
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            format!(
                "[{}; N]",
                type_reference_target_label(syntax_trees, *element_type)
            )
        }
        TypeReferenceNode::Named(name) => name.as_str(syntax_trees).to_owned(),
        _ => "?".to_owned(),
    }
}

/*
    Who may introduce the domain: the clause after the predicates. `access
    Policy` selects the named policy whose plan states the issuance
    (domains.md, "Domain-owned read and write authority"). The retired
    `established by` list is still read until its declarations migrate
    (`DOMAIN-ACCESS-AUTHORITY`); a domain writes one or the other.
*/
type DomainIssuanceClause = (
    Vec<Vec<Identifier>>,
    Option<syntax_trees::expression::StaticMachineArgument>,
);

const ACCESS_WITH_ESTABLISHED_BY: &str = "a domain with an `access` policy states its issuers \
        in that policy's plan; remove the `established by` clause";

fn parse_domain_establishment<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    requires_consumed: bool,
) -> ParseResult<'tokens, 'source, (DomainIssuanceClause, usize)> {
    if input.at_punctuation(PunctuationKind::Semicolon) {
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((((Vec::new(), None), 0), input));
    }

    if input.at_contextual("access") {
        let clause_start_tokens = input.tokens.len();
        let input = input.take_contextual("access")?;
        let (policy, input) = parse_static_selection(
            syntax_trees,
            input,
            "access requires a complete policy application",
        )?;
        if input.at_contextual("established") {
            return Err(input.error_here(ACCESS_WITH_ESTABLISHED_BY));
        }
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        let token_count = clause_start_tokens.saturating_sub(input.tokens.len());
        return Ok((((Vec::new(), Some(policy)), token_count), input));
    }

    if input.at_contextual("satisfies") {
        return Err(input.error_here(
            "a domain classification must appear immediately after the domain head, before `requires`",
        ));
    }

    if requires_consumed
        && !input.at_contextual("established")
        && !input.at_punctuation(PunctuationKind::LeftBrace)
        && domain_requires_terminator(input)
    {
        return Ok((((Vec::new(), None), 0), input));
    }

    if input.at_punctuation(PunctuationKind::LeftBrace) {
        let body = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        if body.at_punctuation(PunctuationKind::RightBrace) {
            return Err(
                input.error_here("empty domain bodies are retired; end the declaration with `;`")
            );
        }
        if at_legacy_authored_route(body) {
            return Err(input.error_here(
                "domain route bodies are retired; write \
                 `established by Trait::requirement, ...;` after the domain predicates",
            ));
        }
        if body.at_contextual("operator") || body.at_contextual("boundary") {
            return Err(body.error_here(
                "domain operators must be ordinary top-level declarations; write \
                 `operator Type::Domain::operation ...` outside the domain declaration",
            ));
        }
        return Err(body.error_here(
            "domain predicates must be written in `requires`; establishment routes use \
             `established by Trait::requirement, ...;`",
        ));
    }

    if !input.at_contextual("established") {
        return Err(
            input.error_here("expected `;` to end the domain declaration or an `access` clause")
        );
    }
    let mut input = input.take_contextual("established")?;
    input = input.take_contextual("by")?;
    let routes_start_tokens = input.tokens.len();
    let mut authored_routes = Vec::new();

    loop {
        let (route, rest) = parse_authored_route(input)?;
        authored_routes.push(route);
        input = rest;
        if !input.at_punctuation(PunctuationKind::Comma) {
            break;
        }
        input = input.take_punctuation(PunctuationKind::Comma, ",")?;
    }

    if input.at_contextual("access") {
        return Err(input.error_here(ACCESS_WITH_ESTABLISHED_BY));
    }
    input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    let token_count = routes_start_tokens.saturating_sub(input.tokens.len());
    Ok((((authored_routes, None), token_count), input))
}

fn at_legacy_authored_route(mut input: Input<'_, '_>) -> bool {
    let Ok((_, rest)) = input.take_identifier() else {
        return false;
    };
    input = rest;
    let mut members = 1usize;
    while input.at_punctuation(PunctuationKind::ColonColon) {
        let Ok(rest) = input.take_punctuation(PunctuationKind::ColonColon, "::") else {
            return false;
        };
        let Ok((_, rest)) = rest.take_identifier() else {
            return false;
        };
        input = rest;
        members += 1;
    }
    members >= 2 && input.at_punctuation(PunctuationKind::Semicolon)
}

fn parse_authored_route<'tokens, 'source>(
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<Identifier>> {
    let mut route = Vec::new();
    loop {
        let (member, rest) = input.take_identifier()?;
        route.push(member);
        input = rest;
        if !input.at_punctuation(PunctuationKind::ColonColon) {
            break;
        }
        input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
    }
    Ok((route, input))
}

pub(crate) fn parse_cleanup_selection<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Option<syntax_trees::expression::StaticMachineArgument>> {
    if !input.at_contextual("dropped") {
        return Ok((None, input));
    }
    let input = input.take_contextual("dropped")?.take_contextual("by")?;
    let (selection, input) = parse_static_selection(
        syntax_trees,
        input,
        "dropped by requires a complete static machine application",
    )?;
    Ok((Some(selection), input))
}

/// One declaration a domain clause selects, with the static arguments it
/// is applied at: the machine after `dropped by`, the policy after
/// `access`.
fn parse_static_selection<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    incomplete_application: &'static str,
) -> ParseResult<'tokens, 'source, syntax_trees::expression::StaticMachineArgument> {
    let (path, input) = parse_authored_route(input)?;
    let (application, input) = match try_parse_static_symbol_application(syntax_trees, input)? {
        Some((application, input)) => (Some(application), input),
        None if input.at_punctuation(PunctuationKind::Less) => {
            return Err(input.error_here(incomplete_application));
        }
        None => (None, input),
    };
    Ok((
        syntax_trees::expression::StaticMachineArgument {
            type_reference: Default::default(),
            path: path.into_boxed_slice(),
            application,
            const_literal: None,
            evidence_projection: None,
        },
        input,
    ))
}
