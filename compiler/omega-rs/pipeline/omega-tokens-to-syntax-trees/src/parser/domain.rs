use crate::parser::expression::parse_expression_handle_without_struct_literals;
use crate::parser::input::{Input, ParseResult};
use crate::parser::operator::parse_operator_definition;
use crate::parser::proof_fact::parse_proof_facts_until;
use omega_core::arena::HandleSpan;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{DomainDefinition, OperatorDefinition, ProofFact};
use omega_syntax_trees::types::TypeReferenceNode;
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_domain_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DomainDefinition> {
    // The domain TARGET is normally a named type (`domain String::Utf8`), but it
    // may be a slice/array carrier (`domain [u8]::Utf8`; encoding domains over the
    // `[u8]` slice). A bracket-prefixed target is parsed as a full type reference;
    // every other target stays the bare-identifier path, so existing named-target
    // declarations are completely unchanged (zero fallout).
    let (target_type, target_label, input) = if input.at_punctuation(PunctuationKind::LeftBracket)
    {
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
    let (classifier, input) = parse_optional_domain_classifier(syntax_trees, input)?;
    let ((facts, operators, body_token_count), input) = parse_domain_body(syntax_trees, input)?;

    Ok((
        DomainDefinition {
            name,
            target_type,
            classifier,
            facts,
            operators,
            body_token_count,
        },
        input,
    ))
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
            format!("[{}]", type_reference_target_label(syntax_trees, *element_type))
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            format!("[{}; N]", type_reference_target_label(syntax_trees, *element_type))
        }
        TypeReferenceNode::Named(name) => name.to_string(),
        _ => "?".to_owned(),
    }
}

fn parse_optional_domain_classifier<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, omega_syntax_trees::expression::ExpressionHandle> {
    if !input.at_keyword(KeywordKind::When) {
        return Ok((
            omega_syntax_trees::expression::ExpressionHandle::invalid(),
            input,
        ));
    }

    let input = input.take_keyword(KeywordKind::When, "when")?;
    // Membership is legal in a classifier: the case-subset domain form is
    // `when self in Type::A | Type::B` (chapter 1 "Cases Are Domains").
    // Struct literals stay excluded -- their brace would swallow the body.
    parse_expression_handle_without_struct_literals(syntax_trees, input)
}

fn parse_domain_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, (HandleSpan<ProofFact>, HandleSpan<OperatorDefinition>, usize)> {
    // A classifier-only domain may end at `;` instead of an empty braced
    // body: `domain Command::Interactive when self in Command::Move |
    // Command::Say;` is the canonical case-subset spelling.
    if input.at_punctuation(PunctuationKind::Semicolon) {
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok(((HandleSpan::empty(), HandleSpan::empty(), 0), input));
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

    Ok(((facts, operators, body_token_count), input))
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
