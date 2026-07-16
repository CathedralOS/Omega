use crate::parse_error::ParseError;
use crate::parser::input::{Input, ParseResult};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::ExpressionHandle;
use omega_syntax_trees::statement::{
    StatementHandle, StatementNode, TableTransition, TransitionTargetHandle, TransitionTargetNode,
};
use omega_tokens::PunctuationKind;

mod guards;
mod targets;

use guards::{parse_transition_expression_list, parse_transition_guard_node};
use targets::parse_transition_block_target_with_bindings;
pub(super) use targets::parse_transition_block_target_handle;

pub(super) fn parse_transition_block_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let (subject, mut input) = if input.at_punctuation(PunctuationKind::LeftBrace) {
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

    let mut start = Handle::invalid();
    let mut count = 0u32;
    let mut arm_statements: Vec<StatementHandle> = Vec::new();
    let mut arm_bool_tuples: Vec<Option<Vec<Option<bool>>>> = Vec::new();
    // Parsed arms are COLLECTED first and their Transition statements
    // appended after the loop, so exhaustiveness MARKER lets (below) can
    // precede every arm statement in the item list -- the returned span
    // must be one contiguous run.
    let mut parsed_arms: Vec<(
        omega_syntax_trees::statement::TransitionGuardNode,
        TransitionTargetHandle,
    )> = Vec::new();
    // (marker name, subject) per `..`-free destructure arm on a PLACE
    // subject: the exhaustiveness law's carrier, deduplicated by name
    // (identical patterns share one marker; duplicate local names would
    // collide in symbol resolution).
    let mut pattern_markers: Vec<(String, ExpressionHandle)> = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (guard, bindings, bool_tuple, rest) =
            parse_transition_guard_node(syntax_trees, input, &subject)?;
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
            parse_transition_block_target_with_bindings(syntax_trees, input, bindings.as_ref())?
        };
        input = rest;

        // Exhaustiveness law (owner spec 2026-07-18): a `..`-free destructure
        // arm must spell every field of its record/case. The spelled set is
        // encoded into a marker let (`__arm_destructure#V=<variant>#<f1>...`;
        // `#`/`=` cannot appear in identifiers, so the split is unambiguous)
        // whose initializer is the subject place -- the typed-stage
        // validation resolves it against the data definition. Non-place
        // subjects skip the marker (no declared type to resolve; the law is
        // enforced where the type is knowable).
        if let Some(bindings) = bindings.as_ref()
            && let Some(spelling) = bindings.spelling.as_ref()
            && !spelling.has_rest
            && expression_is_place(syntax_trees, bindings.subject)
        {
            let mut marker_name = String::from("__arm_destructure#V=");
            if let Some(variant) = spelling.variant.as_ref() {
                marker_name.push_str(variant.as_str());
            }
            for member in &spelling.members {
                marker_name.push('#');
                marker_name.push_str(member.as_str());
            }
            if !pattern_markers.iter().any(|(name, _)| *name == marker_name) {
                pattern_markers.push((marker_name, bindings.subject));
            }
        }

        parsed_arms.push((guard, target));
        arm_bool_tuples.push(bool_tuple);
    }

    for (marker_name, subject_place) in pattern_markers {
        let marker = syntax_trees.statements.insert(StatementNode::LocalData(
            omega_syntax_trees::statement::TableLocalData {
                name: omega_syntax_trees::identifier::Identifier::generated(marker_name),
                type_reference: omega_syntax_trees::types::TypeReferenceHandle::invalid(),
                initial_value: subject_place,
                is_mutable: false,
            },
        ));
        let handle = syntax_trees.items.append_statement_handle(marker);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("transition block statement span count overflow");
    }

    for (guard, target) in parsed_arms {
        let statement =
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation: TransitionTargetHandle::invalid(),
                    guard,
                }));
        let handle = syntax_trees.items.append_statement_handle(statement);
        arm_statements.push(statement);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("transition block statement span count overflow");
    }

    // BOOL-TUPLE exhaustiveness (ch4): when every arm is a bool-literal /
    // wildcard tuple and the matrix covers {true,false}^N, the LAST arm's
    // guard rewrites to the fall-through -- dispatch tests arms in order, so
    // an input reaching the last arm of a covering matrix must match it.
    // Non-bool arms or an uncovered matrix change nothing (the fall-through
    // checker keeps its refusal).
    if subject.len() >= 2
        && arm_bool_tuples.iter().all(|tuple| {
            tuple
                .as_ref()
                .is_some_and(|tuple| tuple.len() == subject.len())
        })
        && subject.len() <= 8
    {
        let matrices: Vec<&Vec<Option<bool>>> =
            arm_bool_tuples.iter().map(|t| t.as_ref().unwrap()).collect();
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
                    guard: omega_syntax_trees::statement::TransitionGuardNode::Always,
                    ..transition
                }),
            );
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let statements = if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    };
    Ok((statements, input))
}

/// A pure re-readable place (Name / self / member chain) -- the same gate as
/// the LET destructure's value: the marker's initializer re-reads the subject,
/// which must not double-evaluate a call.
fn expression_is_place(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> bool {
    match syntax_trees.expressions.expression(expression) {
        omega_syntax_trees::expression::ExpressionNode::Name(_)
        | omega_syntax_trees::expression::ExpressionNode::SelfValue => true,
        omega_syntax_trees::expression::ExpressionNode::Member(member) => {
            expression_is_place(syntax_trees, member.receiver)
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
