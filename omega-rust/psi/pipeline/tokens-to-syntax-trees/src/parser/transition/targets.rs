use crate::parse_error::ParseError;
use crate::parser::expression::{
    parse_argument_list_after_open_paren_handle, parse_expression_handle,
};
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::transition::guards::{
    DestructureBindings, rewrite_destructure_guard_expression,
};
use crate::parser::transition::targets::copy::{
    append_statement_identifier_path_member, copy_expression_handles_to_statement_table,
    copy_expression_identifier_path_to_statement_table,
};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableMemberExpression,
};
use syntax_trees::statement::{TransitionTargetHandle, TransitionTargetNode};
use tokens::{KeywordKind, PunctuationKind};

mod copy;

pub(in crate::parser) fn parse_transition_block_target_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TransitionTargetHandle> {
    parse_transition_block_target_with_bindings(syntax_trees, input, &[])
}

/// Parse a transition-block target, rewriting every destructure-pattern
/// binding into the target expression first: `Command::Say { text } ->
/// done(text)` passes `subject.text`, while tuple arms can project fields from
/// several subjects into the same target call.
pub(super) fn parse_transition_block_target_with_bindings<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    bindings: &[DestructureBindings],
) -> ParseResult<'tokens, 'source, TransitionTargetHandle> {
    // A target-shaped arm body (a bare path or `self`, optionally called) can
    // name a transition target; anything else -- including an arm body the
    // author EXPLICITLY parenthesized -- is a value expression. `-> burn(n)`
    // is a transition target; `-> (burn(n))` is the value of the call (the
    // parens are the author's value-expression spelling, same as
    // `-> (burn(n) + 0)`). A parenthesized call that names a sibling state is
    // re-classified back into a state transition once states are known
    // (symbol assignment), so existing target spellings keep their meaning.
    let target_shaped = !input.at_keyword(KeywordKind::True)
        && !input.at_keyword(KeywordKind::False)
        && (input.at_keyword(KeywordKind::SelfValue) || input.at_name_like());
    let (expression, rest) = if target_shaped {
        let (expr, rest) = parse_transition_target_expression_handle(syntax_trees, input)?;
        // A struct/case literal arm VALUE (`-> Vec2 { dx: 1, dy: 2 }`) is name-like, so
        // the target-expression parser reads only the leading path and leaves the `{`.
        // A bare path immediately followed by `{` is a value, not a transition target
        // -- re-parse the original input as a full expression (which DOES handle struct
        // literals; the scrutinee position is the one that disallows them, not an arm).
        if rest.at_punctuation(PunctuationKind::LeftBrace)
            && matches!(
                syntax_trees.expressions.expression(expr),
                ExpressionNode::Name(_)
            )
        {
            parse_expression_handle(syntax_trees, input)?
        } else {
            (expr, rest)
        }
    } else {
        parse_expression_handle(syntax_trees, input)?
    };

    let expression = bindings.iter().fold(expression, |expression, bindings| {
        if bindings.fields.is_empty() {
            expression
        } else {
            rewrite_destructure_guard_expression(
                syntax_trees,
                expression,
                bindings.subject,
                &bindings.fields,
            )
        }
    });

    Ok((
        classify_transition_target_handle(syntax_trees, expression, target_shaped)?,
        rest,
    ))
}

fn classify_transition_target_handle(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    target_shaped: bool,
) -> Result<TransitionTargetHandle, ParseError> {
    let node = syntax_trees.expressions.expression(expression).clone();
    let target = match node {
        ExpressionNode::Call(call) if target_shaped => {
            classify_call_target_handle(syntax_trees, call)?
        }
        ExpressionNode::SelfValue => TransitionTargetNode::SelfTarget,
        ExpressionNode::Name(_) => TransitionTargetNode::Value(expression),
        _ => TransitionTargetNode::Value(expression),
    };

    Ok(syntax_trees.statements.insert_transition_target(target))
}

fn parse_transition_target_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (mut expression, mut input) = if input.at_keyword(KeywordKind::SelfValue) {
        (
            syntax_trees.expressions.insert(ExpressionNode::SelfValue),
            input.take_keyword(KeywordKind::SelfValue, "self")?,
        )
    } else {
        let (path, rest) = parse_path_handle_span(input, |member| {
            syntax_trees
                .expressions
                .append_identifier_path_member(member)
        })?;
        (
            syntax_trees.expressions.insert(ExpressionNode::Name(path)),
            rest,
        )
    };

    while input.at_punctuation(PunctuationKind::Dot) {
        input = input.take_punctuation(PunctuationKind::Dot, ".")?;
        let (member, rest) = input.take_identifier()?;
        input = rest;
        expression =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: expression,
                    member,
                    case_variant: None,
                }));
    }

    if input.at_punctuation(PunctuationKind::LeftParen) {
        input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let ((arguments, evidence_arguments), rest) =
            parse_argument_list_after_open_paren_handle(syntax_trees, input)?;
        input = rest;
        expression = match syntax_trees.expressions.expression(expression).clone() {
            ExpressionNode::Name(path) => {
                let members = syntax_trees
                    .tables
                    .expressions
                    .identifier_path_members(path)
                    .to_vec();
                let target = members
                    .last()
                    .cloned()
                    .expect("call path should have at least one member");
                let receiver = if members.len() <= 1 {
                    ExpressionHandle::invalid()
                } else {
                    let receiver_path = syntax_trees
                        .tables
                        .expressions
                        .copy_identifier_path_prefix(path, members.len() - 1);
                    syntax_trees
                        .tables
                        .expressions
                        .insert(ExpressionNode::Name(receiver_path))
                };

                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(TableCallExpression {
                        receiver,
                        target,
                        machine_arguments: Box::default(),
                        arguments,
                        evidence_arguments,
                        operational_acknowledgement: Default::default(),
                    }))
            }
            ExpressionNode::Member(member) => {
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(TableCallExpression {
                        receiver: member.receiver,
                        target: member.member,
                        machine_arguments: Box::default(),
                        arguments,
                        evidence_arguments,
                        operational_acknowledgement: Default::default(),
                    }))
            }
            _ => unreachable!("transition target call base should be a path or member access"),
        };
    }

    Ok((expression, input))
}

fn classify_call_target_handle(
    syntax_trees: &mut SyntaxTrees,
    call: TableCallExpression,
) -> Result<TransitionTargetNode, ParseError> {
    let receiver_depth = if call.receiver.is_valid() {
        expression_handle_identifier_depth(syntax_trees, call.receiver)
    } else {
        Some(0)
    };
    let Some(receiver_depth) = receiver_depth else {
        return Ok(TransitionTargetNode::Value(
            syntax_trees.expressions.insert(ExpressionNode::Call(call)),
        ));
    };
    if receiver_depth > 1 {
        return Ok(TransitionTargetNode::Value(
            syntax_trees.expressions.insert(ExpressionNode::Call(call)),
        ));
    }
    let (path, path_starts_at_self) = if call.receiver.is_valid() {
        let receiver =
            copy_expression_identifier_path_to_statement_table(syntax_trees, call.receiver)
                .ok_or_else(|| ParseError::new("call target must be a path or member access"))?;
        (
            append_statement_identifier_path_member(
                syntax_trees,
                receiver.members,
                call.target.clone(),
            ),
            receiver.starts_at_self,
        )
    } else {
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(call.target.clone());
        (HandleSpan::from_parts(handle, 1), false)
    };

    if path_starts_at_self && path.count() == 1 {
        Ok(TransitionTargetNode::SelfTarget)
    } else {
        let arguments = copy_expression_handles_to_statement_table(syntax_trees, call.arguments);
        Ok(TransitionTargetNode::Named {
            path,
            path_starts_at_self,
            arguments,
            evidence_arguments: call.evidence_arguments,
            source_span: call.target.source_span(),
        })
    }
}

fn expression_handle_identifier_depth(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<usize> {
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Name(path) => Some(
            syntax_trees
                .expressions
                .identifier_path_members(*path)
                .len(),
        ),
        ExpressionNode::Member(member) => {
            let depth = expression_handle_identifier_depth(syntax_trees, member.receiver)?;
            Some(depth + 1)
        }
        ExpressionNode::SelfValue => Some(1),
        _ => None,
    }
}
