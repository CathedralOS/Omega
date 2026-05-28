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
use targets::parse_transition_block_target_handle;

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

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (guard, rest) = parse_transition_guard_node(syntax_trees, input, &subject)?;
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
            parse_transition_block_target_handle(syntax_trees, input)?
        };
        input = rest;

        let statement =
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation: TransitionTargetHandle::invalid(),
                    guard,
                }));
        let handle = syntax_trees.items.append_statement_handle(statement);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("transition block statement span count overflow");
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let statements = if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    };
    Ok((statements, input))
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
