use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::proof_fact::parse_proof_facts_until;
use psi_arena::HandleSpan;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{DomainAliasDefinition, DomainDefinition, ProofFact};
use psi_syntax_trees::types::TypeReferenceNode;
use psi_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_domain_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DomainDefinition> {
    let (generic_parameters, input) =
        crate::parser::data::parse_type_parameters(syntax_trees, input)?;
    if !generic_parameters.lifetime_parameters.is_empty() {
        return Err(input.error_here(
            "domain families take a carrier type and proof-static const parameters, not lifetime parameters",
        ));
    }
    // The domain TARGET is normally a named type (`domain String::Utf8`), but it
    // may be a slice/array carrier (`domain [u8]::Utf8`; encoding domains over the
    // `[u8]` slice). A bracket-prefixed target is parsed as a full type reference;
    // every other target stays the bare-identifier path, so existing named-target
    // declarations are completely unchanged (zero fallout).
    let (target_type, target_label, input) = if input.at_punctuation(PunctuationKind::LeftBracket) {
        let (handle, input) =
            crate::parser::type_reference::parse_type_reference_handle(syntax_trees, input)?;
        let label = type_reference_target_label(syntax_trees, handle);
        (handle, label, input)
    } else {
        let (target_name, input) = input.take_identifier()?;
        let handle = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(target_name.clone()));
        (handle, target_name.to_string(), input)
    };
    let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
    let (domain_name, input) = input.take_identifier()?;
    let (index_arguments, input) =
        crate::parser::type_reference::parse_domain_argument_handles(syntax_trees, input)?;
    let name = if generic_parameters.type_parameters.is_empty() {
        Identifier::generated(format!("{target_label}::{domain_name}"))
    } else {
        domain_name
    };
    let (
        alias,
        authored_routes,
        predicate_body,
        facts,
        operators,
        semantic_clause_token_count,
        input,
    ) = if input.at_punctuation(PunctuationKind::Equal) {
        let (alias, input) = parse_domain_alias(syntax_trees, input)?;
        (
            Some(alias),
            Vec::new(),
            psi_language_core::DomainPredicateBody::Bodyless,
            HandleSpan::empty(),
            HandleSpan::empty(),
            0,
            input,
        )
    } else {
        let ((predicate_body, facts, requires_token_count), input) =
            parse_domain_requires(syntax_trees, input)?;
        let ((authored_routes, semantic_clause_token_count), input) =
            parse_domain_establishment(input, predicate_body.is_present())?;
        (
            None,
            authored_routes,
            predicate_body,
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
            predicate_body,
            facts,
            operators,
            semantic_clause_token_count,
        },
        input,
    ))
}

fn parse_domain_requires<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<
    'tokens,
    'source,
    (
        psi_language_core::DomainPredicateBody,
        HandleSpan<ProofFact>,
        usize,
    ),
> {
    if !input.at_contextual("requires") {
        return Ok((
            (
                psi_language_core::DomainPredicateBody::Bodyless,
                HandleSpan::empty(),
                0,
            ),
            input,
        ));
    }

    let input = input.take_contextual("requires")?;
    let ((facts, token_count), input) =
        parse_proof_facts_until(syntax_trees, input, domain_requires_terminator)?;
    if facts.is_empty() {
        return Err(input.error_here("domain `requires` must contain at least one proposition"));
    }
    Ok((
        (
            psi_language_core::DomainPredicateBody::Present,
            facts,
            token_count,
        ),
        input,
    ))
}

fn domain_requires_terminator(input: Input<'_, '_>) -> bool {
    input.at_contextual("established")
        || input.at_punctuation(PunctuationKind::LeftBrace)
        || input.tokens.is_empty()
        || input.at_keyword(KeywordKind::Pub)
        || input.at_keyword(KeywordKind::Data)
        || input.at_keyword(KeywordKind::Machine)
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
            syntax_trees.items.append_identifier_path_member(member)
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

/// A readable label for a domain TARGET type, used to build the domain's name
/// (`[u8]::Utf8`). Covers the carriers an encoding domain attaches to; a named
/// target uses its identifier.
fn type_reference_target_label(
    syntax_trees: &SyntaxTrees,
    handle: psi_syntax_trees::types::TypeReferenceHandle,
) -> String {
    match syntax_trees.type_references.type_reference(handle) {
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
        TypeReferenceNode::Named(name) => name.to_string(),
        _ => "?".to_owned(),
    }
}

fn parse_domain_establishment<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    requires_consumed: bool,
) -> ParseResult<'tokens, 'source, (Vec<Vec<Identifier>>, usize)> {
    if input.at_punctuation(PunctuationKind::Semicolon) {
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok(((Vec::new(), 0), input));
    }

    // A `requires` clause may own the declaration's final semicolon. When its
    // proof-fact parser has already consumed that token, the next root item is
    // the caller's input and this domain has no establishment clause.
    if requires_consumed
        && !input.at_contextual("established")
        && !input.at_punctuation(PunctuationKind::LeftBrace)
        && domain_requires_terminator(input)
    {
        return Ok(((Vec::new(), 0), input));
    }

    if input.at_punctuation(PunctuationKind::LeftBrace) {
        let body = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
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

    input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    let token_count = routes_start_tokens.saturating_sub(input.tokens.len());
    Ok(((authored_routes, token_count), input))
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
    if route.len() < 2 {
        return Err(
            input.error_here("domain establishment routes must name an exact `Trait::requirement`")
        );
    }
    Ok((route, input))
}
