use crate::errors::parse_error::ParseError;
use crate::token_input::cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::statement::{
    DispatchArm, StatementHandle, StatementNode, TableDispatch, TransitionTargetNode,
};
use tokens::{KeywordKind, PunctuationKind};

use crate::transitions::guards::{parse_transition_expression_list, parse_transition_guard_node};
use crate::transitions::targets::parse_transition_block_target_with_bindings;

pub(crate) fn parse_transition_block_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let start = input;
    let input = input.take_keyword(KeywordKind::Transition, "transition")?;
    let (subjects, after_subjects) = if input.at_punctuation(PunctuationKind::LeftBrace) {
        (Vec::new(), input)
    } else {
        let (expressions, rest) = parse_expression_list_until_punctuation(
            syntax_trees,
            input,
            PunctuationKind::LeftBrace,
        )?;
        (expressions, rest)
    };

    let subjects_span = (!subjects.is_empty()).then(|| input.source_span_until(after_subjects));
    let mut input = after_subjects.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut arms = Vec::new();
    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (condition, rest) = parse_transition_guard_node(syntax_trees, input, &subjects)?;
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
            parse_transition_block_target_with_bindings(syntax_trees, input, &condition.bindings)?
        };
        input = rest;

        arms.push(DispatchArm {
            patterns: condition.patterns,
            guard: condition.guard,
            target,
            source_span,
        });
    }
    if arms.is_empty() {
        return Err(input.error_here(
            "a transition block must carry at least one arm; an empty dispatch can never proceed",
        ));
    }
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let subjects = syntax_trees.statements.insert_expression_handles(subjects);
    let dispatch = syntax_trees.statements.insert_dispatch(TableDispatch {
        subjects,
        subjects_span,
        arms: arms.into_boxed_slice(),
        source_span: start.source_span_until(input),
    });
    let statement = syntax_trees
        .statements
        .insert(StatementNode::Dispatch(dispatch));
    Ok((
        HandleSpan::from_parts(syntax_trees.items.append_statement_handle(statement), 1),
        input,
    ))
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
