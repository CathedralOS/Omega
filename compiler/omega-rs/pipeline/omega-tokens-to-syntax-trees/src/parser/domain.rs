use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::operator::parse_operator_definition;
use crate::parser::proof_fact::parse_proof_facts_until;
use omega_core::arena::HandleSpan;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{
    DomainAliasDefinition, DomainDefinition, OperatorDefinition, ProofFact,
};
use omega_syntax_trees::types::TypeReferenceNode;
use omega_tokens::PunctuationKind;

pub(super) fn parse_domain_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DomainDefinition> {
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
    let name = Identifier::generated(format!("{target_label}::{domain_name}"));
    let (alias, predicate_body, facts, operators, body_token_count, input) =
        if input.at_punctuation(PunctuationKind::Equal) {
            let (alias, input) = parse_domain_alias(syntax_trees, input)?;
            (
                Some(alias),
                omega_core::semantics::DomainPredicateBody::Bodyless,
                HandleSpan::empty(),
                HandleSpan::empty(),
                0,
                input,
            )
        } else {
            let ((predicate_body, facts, operators, body_token_count), input) =
                parse_domain_body(syntax_trees, input)?;
            (
                None,
                predicate_body,
                facts,
                operators,
                body_token_count,
                input,
            )
        };

    Ok((
        DomainDefinition {
            name,
            target_type,
            is_public: false,
            alias,
            predicate_body,
            facts,
            operators,
            body_token_count,
        },
        input,
    ))
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
    handle: omega_syntax_trees::types::TypeReferenceHandle,
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

fn parse_domain_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<
    'tokens,
    'source,
    (
        omega_core::semantics::DomainPredicateBody,
        HandleSpan<ProofFact>,
        HandleSpan<OperatorDefinition>,
        usize,
    ),
> {
    if input.at_punctuation(PunctuationKind::Semicolon) {
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            (
                omega_core::semantics::DomainPredicateBody::Bodyless,
                HandleSpan::empty(),
                HandleSpan::empty(),
                0,
            ),
            input,
        ));
    }

    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let body_start_tokens = input.tokens.len();
    let mut facts = HandleSpan::empty();
    let mut operators = HandleSpan::empty();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_contextual("operator") {
            input = input.take_contextual("operator")?;
            let (operator, rest) = parse_operator_definition(syntax_trees, input, false)?;
            let handle = syntax_trees.items.append_operator(operator);
            operators.push_contiguous(handle);
            input = rest;
            continue;
        }

        if input.at_contextual("boundary") {
            input = input.take_contextual("boundary")?;
            input = input.take_contextual("operator")?;
            let (operator, rest) = parse_operator_definition(syntax_trees, input, true)?;
            let handle = syntax_trees.items.append_operator(operator);
            operators.push_contiguous(handle);
            input = rest;
            continue;
        }

        let ((parsed_facts, _), rest) = parse_proof_facts_until(syntax_trees, input, |input| {
            input.at_punctuation(PunctuationKind::RightBrace)
                || input.at_contextual("operator")
                || input.at_contextual("boundary")
        })?;
        facts = merge_contiguous_fact_spans(facts, parsed_facts);
        input = rest;
    }

    let body_token_count = body_start_tokens.saturating_sub(input.tokens.len());
    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let predicate_body = if facts.is_empty() {
        omega_core::semantics::DomainPredicateBody::Bodyless
    } else {
        omega_core::semantics::DomainPredicateBody::Present
    };

    Ok(((predicate_body, facts, operators, body_token_count), input))
}

fn merge_contiguous_fact_spans(
    left: HandleSpan<ProofFact>,
    right: HandleSpan<ProofFact>,
) -> HandleSpan<ProofFact> {
    if left.is_empty() {
        return right;
    }
    if right.is_empty() {
        return left;
    }

    let expected_index = left
        .start()
        .arena_index()
        .checked_add(left.count())
        .expect("proof fact span index overflow");
    assert_eq!(
        right.start().arena_index(),
        expected_index,
        "domain fact spans should remain contiguous across operator declarations"
    );
    HandleSpan::from_parts(
        left.start(),
        left.count()
            .checked_add(right.count())
            .expect("proof fact span count overflow"),
    )
}
