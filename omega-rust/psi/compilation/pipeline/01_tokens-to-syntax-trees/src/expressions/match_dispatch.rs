//! Expression dispatch retains the subject capture and authored pattern shape.
//! Arm aliases are reads of that capture, not synthetic owning declarations.
//! Checking decides case identity, completeness, access and selected effects;
//! parsing only binds lexical spellings inside the corresponding guard/body.

use super::parse_expression::{
    parse_expression_handle, parse_expression_handle_without_struct_literals,
};
use crate::input::token_cursor::{Input, ParseResult};
use crate::patterns::bindings::{DestructureBinding, rewrite_destructure_guard_expression};
use arena::Handle;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern, MatchSubjectCapture,
    TableDestructurePattern, TableMatchArm, TableMatchExpression, TableSavedSubject,
};
use tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_match_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let start = input;
    let input = input.take_keyword(KeywordKind::Match, "match")?;
    let (subject, input) = parse_expression_handle_without_struct_literals(syntax_trees, input)?;
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut capture = Handle::<MatchSubjectCapture>::invalid();
    let mut arms = Vec::new();
    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let arm_start = input;
        let (mut pattern, rest) = parse_pattern(syntax_trees, input)?;
        let rest = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
        let (mut value, rest) = parse_expression_handle(syntax_trees, rest)?;
        if let MatchPattern::Destructure(destructure) = &mut pattern {
            if !capture.is_valid() {
                capture = syntax_trees.expressions.insert_match_subject(subject);
            }
            let bindings = syntax_trees
                .expressions
                .match_fields(destructure.fields)
                .iter()
                .filter_map(|field| {
                    let MatchFieldAction::Bind(binding) = &field.action else {
                        return None;
                    };
                    Some(DestructureBinding {
                        binding: binding.clone(),
                        member: field.member.clone(),
                        case_variant: None,
                    })
                })
                .collect::<Vec<_>>();
            for (position, binding) in bindings.iter().enumerate() {
                if bindings[..position].iter().any(|prior| {
                    prior.binding.as_str(syntax_trees) == binding.binding.as_str(syntax_trees)
                }) || syntax_trees
                    .statements
                    .outcome_proof_selectors(destructure.proof_selectors)
                    .iter()
                    .any(|selector| {
                        selector.binding.as_str(syntax_trees)
                            == binding.binding.as_str(syntax_trees)
                    })
                {
                    return Err(arm_start.error_here(format!(
                        "pattern binding `{}` is introduced more than once in this arm",
                        binding.binding.as_str(syntax_trees)
                    )));
                }
            }
            let saved = syntax_trees
                .expressions
                .insert(ExpressionNode::SavedSubject(TableSavedSubject {
                    capture,
                    pattern_head: destructure.head,
                }));
            let source_span = syntax_trees.expressions.source_span(subject);
            syntax_trees.expressions.set_source_span(saved, source_span);
            value = rewrite_destructure_guard_expression(syntax_trees, value, saved, &bindings);
            if destructure.guard.is_valid() {
                destructure.guard = rewrite_destructure_guard_expression(
                    syntax_trees,
                    destructure.guard,
                    saved,
                    &bindings,
                );
            }
        }
        arms.push(TableMatchArm {
            pattern,
            value,
            source_span: arm_start.source_span_until(rest),
        });
        input = if rest.at_punctuation(PunctuationKind::Comma) {
            rest.take_punctuation(PunctuationKind::Comma, ",")?
        } else {
            rest
        };
    }
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    if arms.is_empty() {
        return Err(input.error_here("match expression must have at least one arm"));
    }
    let arms = syntax_trees.expressions.insert_match_arms(arms);
    let expression = syntax_trees
        .expressions
        .insert(ExpressionNode::Match(TableMatchExpression {
            subject,
            capture,
            arms,
        }));
    syntax_trees
        .expressions
        .set_source_span(expression, start.source_span_until(input));
    Ok((expression, input))
}

fn parse_pattern<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, MatchPattern> {
    if input.at_contextual("_") {
        return Ok((MatchPattern::Wildcard, input.take_contextual("_")?));
    }
    let (head, rest) = parse_expression_handle_without_struct_literals(syntax_trees, input)?;
    if !rest.at_punctuation(PunctuationKind::LeftBrace)
        || !matches!(
            syntax_trees.expressions.expression(head),
            ExpressionNode::Name(_)
        )
    {
        return Ok((MatchPattern::Value(head), rest));
    }
    let (fields, rest) = crate::patterns::parse_destructure_fields(syntax_trees, rest)?;
    let fields_span = syntax_trees.expressions.insert_match_fields(fields.fields);
    let proof_selectors = syntax_trees
        .statements
        .insert_outcome_proof_selectors(fields.proof_selectors);
    let (guard, rest) = if rest.at_keyword(KeywordKind::If) {
        let rest = rest.take_keyword(KeywordKind::If, "if")?;
        parse_expression_handle_without_struct_literals(syntax_trees, rest)?
    } else {
        (ExpressionHandle::invalid(), rest)
    };
    Ok((
        MatchPattern::Destructure(TableDestructurePattern {
            head,
            fields: fields_span,
            has_rest: fields.has_rest,
            guard,
            proof_selectors,
        }),
        rest,
    ))
}
