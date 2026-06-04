use crate::parser::expression::parse_expression_handle_without_struct_literals_or_membership;
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
    let (target_name, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
    let (domain_name, input) = input.take_identifier()?;
    let target_type = syntax_trees
        .type_references
        .insert(TypeReferenceNode::Named(target_name.clone()));
    let name = Identifier::generated(format!("{target_name}::{domain_name}"));
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
    parse_expression_handle_without_struct_literals_or_membership(syntax_trees, input)
}

fn parse_domain_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, (HandleSpan<ProofFact>, HandleSpan<OperatorDefinition>, usize)> {
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
