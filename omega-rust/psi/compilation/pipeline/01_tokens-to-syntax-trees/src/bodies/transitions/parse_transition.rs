use crate::diagnostics::parse_error::ParseError;
use crate::input::token_cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{
    StatementHandle, StatementNode, TableTransition, TransitionTargetHandle, TransitionTargetNode,
};
use tokens::PunctuationKind;

use super::guards::{parse_transition_expression_list, parse_transition_guard_node};
use super::targets::parse_target::parse_transition_block_target_with_bindings;

pub(crate) fn parse_transition_block_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let (mut subject, mut input) = if input.at_punctuation(PunctuationKind::LeftBrace) {
        (
            Vec::new(),
            input.take_punctuation(PunctuationKind::LeftBrace, "{")?,
        )
    } else {
        let (expressions, rest) = parse_expression_list_until_punctuation(
            syntax_trees,
            input,
            PunctuationKind::LeftBrace,
        )?;
        let input = rest.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        (expressions, input)
    };

    // Every dispatch subject completes once, in authored order, before its
    // arms. Retain that purpose until typed preparation can distinguish a
    // copyable value snapshot from a noncopy storage observation. Parsing
    // neither moves affine storage nor schedules selected address operands.
    let mut subject_captures: Vec<(Identifier, ExpressionHandle)> = Vec::new();
    // Capture is a property of subject evaluation, not of the arm's spelling.
    // In particular, ordinary case payloads and proof-bearing payloads retain
    // the same owner while their selected fields are transferred separately.
    for subject_expression in &mut subject {
        // A closed boolean subject needs no capture: there is no evaluation to
        // perform once and no declared pattern carrier to infer. Capturing it
        // would hide the literal behind a generated name, and the exhaustiveness
        // proof reads the subject's own value to close
        // `transition true { true -> X }` like `_` instead of counting one arm
        // of a boolean axis.
        if expression_is_closed_boolean(syntax_trees, *subject_expression) {
            continue;
        }
        // Typed preparation chooses value snapshots versus storage observation.
        // Parsing retains one subject sequence and never extracts selectors.
        let captured = *subject_expression;
        let name =
            Identifier::generated(format!("__transition_subject#{}", captured.arena_index()));
        let path_start = syntax_trees
            .expressions
            .append_identifier_path_member(name.clone());
        let path = HandleSpan::from_parts(path_start, 1);
        *subject_expression = syntax_trees.expressions.insert(ExpressionNode::Name(path));
        subject_captures.push((name, captured));
    }

    let mut statements = crate::handle_spans::HandleSpanBuilder::new();
    let mut arm_statements: Vec<StatementHandle> = Vec::new();
    let mut arm_bool_tuples: Vec<Option<Vec<Option<bool>>>> = Vec::new();
    // Parsed arms are COLLECTED first and their Transition statements
    // appended after the loop, so exhaustiveness MARKER lets (below) can
    // precede every arm statement in the item list -- the returned span
    // must be one contiguous run.
    let mut parsed_arms: Vec<(
        syntax_trees::statement::TransitionGuardNode,
        TransitionTargetHandle,
        arena::HandleSpan<syntax_trees::statement::TableOutcomeProofSelector>,
        source::SourceSpan,
    )> = Vec::new();
    // (marker name, subject) per destructure arm. Computed transition subjects
    // have already become generated places above, so every arm has a declared
    // type carrier for the missing/unknown-field law. Deduplicated by name
    // (identical patterns share one marker; duplicate local names would
    // collide in symbol resolution).
    let mut pattern_markers: Vec<(String, ExpressionHandle)> = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (guard, bindings, bool_tuple, rest) =
            parse_transition_guard_node(syntax_trees, input, &subject)?;
        let source_span = rest
            .tokens
            .first()
            .map(|token| rest.source_span(token))
            .expect("recognized transition arrow has a source token");
        input = rest.take_punctuation(PunctuationKind::Arrow, "->")?;

        let (target, rest) = if input.at_punctuation(PunctuationKind::LeftBrace) {
            let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
            let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
            (
                syntax_trees
                    .statements
                    .insert_transition_target(TransitionTargetNode::Terminal),
                input,
            )
        } else {
            parse_transition_block_target_with_bindings(syntax_trees, input, &bindings)?
        };
        input = rest;

        // Exhaustiveness law (owner spec 2026-07-18): a `..`-free destructure
        // arm must spell every field of its record/case. The spelled set is
        // encoded into a marker let (`__arm_destructure#V=<variant>#<f1>...`;
        // `#`/`=` cannot appear in identifiers, so the split is unambiguous)
        // whose initializer is the subject place -- the typed-stage
        // validation resolves it against the data definition.
        for arm_binding in &bindings {
            let Some(spelling) = arm_binding.spelling.as_ref() else {
                continue;
            };
            let mut marker_name = String::from("__arm_destructure#V=");
            if let Some(variant) = spelling.variant.as_ref() {
                marker_name.push_str(variant.as_str(syntax_trees));
            }
            for member in &spelling.members {
                marker_name.push('#');
                marker_name.push_str(member.as_str(syntax_trees));
            }
            // Two tuple axes may spell the same pattern. Keep validation
            // carriers keyed by subject without exposing the generated id as
            // an authored field name; validation strips this sentinel.
            marker_name.push_str("#~subject=");
            marker_name.push_str(&arm_binding.subject.arena_index().to_string());
            // `..` opts out of the MISSING-field law but spelled fields must
            // still EXIST -- the trailing `#~rest` sentinel tells validation
            // to skip only the exhaustiveness half (`~` cannot appear in an
            // identifier).
            if spelling.has_rest {
                marker_name.push_str("#~rest");
            }
            if !pattern_markers
                .iter()
                .any(|(name, subject)| *name == marker_name && *subject == arm_binding.subject)
            {
                pattern_markers.push((marker_name, arm_binding.subject));
            }
        }

        let proof_selectors = bindings
            .first()
            .map(|bindings| {
                syntax_trees
                    .statements
                    .insert_outcome_proof_selectors(bindings.proof_selectors.iter().cloned())
            })
            .unwrap_or_else(HandleSpan::empty);
        parsed_arms.push((guard, target, proof_selectors, source_span));
        arm_bool_tuples.push(bool_tuple);
    }

    // An empty block lowers to zero statements, so no later stage can tell a
    // `transition {}` from no transition at all -- refuse it here while the
    // braces are still visible.
    if parsed_arms.is_empty() {
        return Err(input.error_here(
            "a transition block must carry at least one arm; an empty dispatch can never proceed",
        ));
    }

    for (name, initial_value) in subject_captures {
        let capture = syntax_trees.statements.insert(StatementNode::LocalData(
            syntax_trees::statement::TableLocalData {
                role: language_core::LocalBindingRole::DispatchSubject,
                name,
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                initial_value,
                is_mutable: false,
                relevance: language_core::BindingRelevance::Relevant,
            },
        ));
        let handle = syntax_trees.items.append_statement_handle(capture);
        statements.push(handle);
    }

    for (marker_name, subject_place) in pattern_markers {
        let marker = syntax_trees.statements.insert(StatementNode::LocalData(
            syntax_trees::statement::TableLocalData {
                role: language_core::LocalBindingRole::ArmPattern,
                name: syntax_trees::identifier::Identifier::generated(marker_name),
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                initial_value: subject_place,
                is_mutable: false,
                relevance: language_core::BindingRelevance::Relevant,
            },
        ));
        let handle = syntax_trees.items.append_statement_handle(marker);
        statements.push(handle);
    }

    for (guard, target, proof_selectors, source_span) in parsed_arms {
        let statement =
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation: TransitionTargetHandle::invalid(),
                    guard,
                    proof_selectors,
                    exit: Default::default(),
                    ends_dispatch: false,
                    source_span,
                }));
        let handle = syntax_trees.items.append_statement_handle(statement);
        arm_statements.push(statement);
        statements.push(handle);
    }

    // Keep the authored block boundary after flattening its arms. A later
    // crash, return or another dispatch cannot cover this block's no-match path.
    let last = *arm_statements
        .last()
        .expect("empty dispatch rejected above");
    if let StatementNode::Transition(mut transition) =
        syntax_trees.statements.statement(last).clone()
    {
        transition.ends_dispatch = true;
        syntax_trees
            .statements
            .replace_statement(last, StatementNode::Transition(transition));
    }

    // A two-arm Boolean subject needs only the first test: the opposite arm
    // is the fall-through. Retaining both tests would evaluate a computed
    // subject twice. Anonymous guards have no shared subject and do not qualify.
    let opposite_boolean_pair = subject.len() == 1
        && matches!(arm_bool_tuples.as_slice(), [Some(first), Some(second)]
            if matches!((first.as_slice(), second.as_slice()),
                ([Some(left)], [Some(right)]) if left != right));

    // BOOL-TUPLE exhaustiveness (ch4): when every arm is a bool-literal /
    // wildcard tuple and the matrix covers {true,false}^N, the LAST arm's
    // guard rewrites to the fall-through -- dispatch tests arms in order, so
    // an input reaching the last arm of a covering matrix must match it.
    // Non-bool arms or an uncovered matrix change nothing (the fall-through
    // checker keeps its refusal).
    if (subject.len() >= 2 || opposite_boolean_pair)
        && arm_bool_tuples.iter().all(|tuple| {
            tuple
                .as_ref()
                .is_some_and(|tuple| tuple.len() == subject.len())
        })
        && subject.len() <= 8
    {
        let matrices: Vec<&Vec<Option<bool>>> = arm_bool_tuples
            .iter()
            .map(|t| t.as_ref().unwrap())
            .collect();
        let covered = (0u32..(1u32 << subject.len())).all(|combo| {
            matrices.iter().any(|arm| {
                arm.iter().enumerate().all(|(bit, pattern)| match pattern {
                    None => true,
                    Some(value) => *value == ((combo >> bit) & 1 == 1),
                })
            })
        });
        if covered
            && let Some(last) = arm_statements.last()
            && let StatementNode::Transition(transition) =
                syntax_trees.statements.statement(*last).clone()
        {
            syntax_trees.statements.replace_statement(
                *last,
                StatementNode::Transition(TableTransition {
                    guard: syntax_trees::statement::TransitionGuardNode::Always,
                    ..transition
                }),
            );
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((statements.finish(), input))
}

/// A boolean literal, or an equality of two boolean literals: the subject shapes
/// whose value is fixed without evaluation, so no capture binds anything.
fn expression_is_closed_boolean(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> bool {
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Boolean(_) => true,
        ExpressionNode::Binary(binary) => {
            binary.operator == syntax_trees::expression::BinaryOperator::Equal
                && matches!(
                    syntax_trees.expressions.expression(binary.left),
                    ExpressionNode::Boolean(_)
                )
                && matches!(
                    syntax_trees.expressions.expression(binary.right),
                    ExpressionNode::Boolean(_)
                )
        }
        _ => false,
    }
}

fn parse_expression_list_until_punctuation<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    delimiter: PunctuationKind,
) -> Result<(Vec<ExpressionHandle>, Input<'tokens, 'source>), ParseError> {
    let (expression_input, rest) =
        input.split_at_top_level_punctuation(delimiter, "expected transition block delimiter")?;
    let (expressions, rest_after_expression) =
        parse_transition_expression_list(syntax_trees, expression_input)?;

    if !rest_after_expression.tokens.is_empty() {
        return Err(rest_after_expression.error_here("expected transition subject expression"));
    }

    Ok((expressions, rest))
}

#[cfg(test)]
mod tests {}
