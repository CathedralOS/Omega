use crate::errors::parse_error::ParseError;
use crate::syntax_storage::tables::{items, stmts, stmts_of};
use crate::token_input::cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{
    StatementHandle, StatementNode, TableLocalData, TableTransition, TransitionTargetHandle,
    TransitionTargetNode,
};
use tokens::PunctuationKind;

use crate::transitions::guards::{parse_transition_expression_list, parse_transition_guard_node};
use crate::transitions::targets::parse_transition_block_target_with_bindings;

pub(crate) fn parse_transition_block_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    /*
        Captured subjects are named by where the transition starts
        in its file and their place in its subject list. The
        parser splits files into pages by worker count, and each
        page numbers its own expressions, so a name taken from an
        expression's arena index changed with the host's thread
        count and so did every checked record holding it.
    */
    let transition_start = input.tokens.first().map_or(0, |token| token.span.start);
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

    let mut subject_captures: Vec<(Identifier, ExpressionHandle)> = Vec::new();
    for (place, subject_expression) in subject.iter_mut().enumerate() {
        if expression_is_closed_boolean(syntax_trees, *subject_expression) {
            continue;
        }
        let captured = *subject_expression;
        let name =
            Identifier::generated(format!("__transition_subject#{transition_start}.{place}"));
        let path_start = syntax_trees
            .expressions
            .append_identifier_path_member(name.clone());
        let path = HandleSpan::from_parts(path_start, 1);
        *subject_expression = syntax_trees.expressions.insert(ExpressionNode::Name {
            members: path,
            application: None,
        });
        subject_captures.push((name, captured));
    }

    let mut statements = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();
    let mut arm_statements: Vec<StatementHandle> = Vec::new();
    let mut arm_bool_tuples: Vec<Option<Vec<Option<bool>>>> = Vec::new();
    let mut parsed_arms: Vec<(
        syntax_trees::statement::TransitionGuardNode,
        TransitionTargetHandle,
        arena::HandleSpan<syntax_trees::statement::TableOutcomeProofSelector>,
        source::SourceSpan,
        HandleSpan<syntax_trees::statement::TransitionPattern>,
    )> = Vec::new();
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
                stmts(syntax_trees).insert_transition_target(TransitionTargetNode::Terminal),
                input,
            )
        } else {
            parse_transition_block_target_with_bindings(syntax_trees, input, &bindings)?
        };
        input = rest;

        let patterns = stmts(syntax_trees)
            .insert_transition_patterns(bindings.iter().filter_map(|binding| binding.pattern));
        let proof_selectors = bindings
            .first()
            .map(|bindings| {
                stmts(syntax_trees)
                    .insert_outcome_proof_selectors(bindings.proof_selectors.iter().cloned())
            })
            .unwrap_or_else(HandleSpan::empty);
        parsed_arms.push((guard, target, proof_selectors, source_span, patterns));
        arm_bool_tuples.push(bool_tuple);
    }

    if parsed_arms.is_empty() {
        return Err(input.error_here(
            "a transition block must carry at least one arm; an empty dispatch can never proceed",
        ));
    }

    for (name, initial_value) in subject_captures {
        let capture = stmts(syntax_trees).insert(StatementNode::LocalData(TableLocalData {
            role: language_core::LocalBindingRole::DispatchSubject,
            name,
            type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
            initial_value,
            is_mutable: false,
            relevance: language_core::BindingRelevance::Relevant,
            pattern_fields: Default::default(),
        }));
        let handle = items(syntax_trees).append_statement_handle(capture);
        statements.push(handle);
    }

    for (guard, target, proof_selectors, source_span, patterns) in parsed_arms {
        let statement = stmts(syntax_trees).insert(StatementNode::Transition(TableTransition {
            patterns,
            target,
            continuation: TransitionTargetHandle::invalid(),
            guard,
            proof_selectors,
            exit: Default::default(),
            ends_dispatch: false,
            source_span,
        }));
        let handle = items(syntax_trees).append_statement_handle(statement);
        arm_statements.push(statement);
        statements.push(handle);
    }

    let last = *arm_statements
        .last()
        .expect("empty dispatch rejected above");
    if let StatementNode::Transition(mut transition) =
        stmts_of(syntax_trees).statement(last).clone()
    {
        transition.ends_dispatch = true;
        stmts(syntax_trees).replace_statement(last, StatementNode::Transition(transition));
    }

    let opposite_boolean_pair = subject.len() == 1
        && matches!(arm_bool_tuples.as_slice(), [Some(first), Some(second)]
            if matches!((first.as_slice(), second.as_slice()),
                ([Some(left)], [Some(right)]) if left != right));

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
                stmts_of(syntax_trees).statement(*last).clone()
        {
            stmts(syntax_trees).replace_statement(
                *last,
                StatementNode::Transition(TableTransition {
                    guard: syntax_trees::statement::TransitionGuardNode::Always,
                    patterns: HandleSpan::empty(),
                    ..transition
                }),
            );
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((statements.finish(), input))
}

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
