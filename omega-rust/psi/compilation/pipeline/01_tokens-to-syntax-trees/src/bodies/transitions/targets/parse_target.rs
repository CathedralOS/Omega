use crate::bodies::transitions::guards::{
    DestructureBindings, rewrite_destructure_guard_expression,
};
use crate::bodies::transitions::targets::copy::{
    append_statement_identifier_path_member, copy_expression_handles_to_statement_table,
    copy_expression_identifier_path_to_statement_table,
};
use crate::diagnostics::parse_error::ParseError;
use crate::expressions::context::ExpressionContext;
use crate::expressions::parse_expression::{has_unary_expression_prefix, parse_expression_handle};
use crate::expressions::parse_postfix::parse_postfix_expression_handle;
use crate::input::token_cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use syntax_trees::statement::{TransitionTargetHandle, TransitionTargetNode};
use tokens::KeywordKind;

pub(crate) fn parse_transition_block_target_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TransitionTargetHandle> {
    parse_transition_block_target_with_bindings(syntax_trees, input, &[])
}

/// Parse a transition-block target, rewriting every destructure-pattern
/// binding into the target expression first: `Command::Say { text } ->
/// done(text)` passes `subject.text`, while tuple arms can project fields from
/// several subjects into the same target call.
pub(in crate::bodies::transitions) fn parse_transition_block_target_with_bindings<
    'tokens,
    'source,
>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    bindings: &[DestructureBindings],
) -> ParseResult<'tokens, 'source, TransitionTargetHandle> {
    // A target-shaped arm body (a bare path, optionally called) can
    // name a transition target; anything else -- including an arm body the
    // author EXPLICITLY parenthesized -- is a value expression. `-> burn(n)`
    // is a transition target; `-> (burn(n))` is the value of the call (the
    // parens are the author's value-expression spelling, same as
    // `-> (burn(n) + 0)`). A parenthesized call that names a sibling state is
    // re-classified back into a state transition once states are known
    // (symbol assignment), so existing target spellings keep their meaning.
    // Prefixes belong to the ordinary expression parser, which retains call
    // acknowledgements and diagnoses their order. Parsing one as a target
    // name would strand the actual call and discard its operational envelope.
    let target_shaped = !has_unary_expression_prefix(input)?
        && !input.at_keyword(KeywordKind::True)
        && !input.at_keyword(KeywordKind::False)
        && !input.at_keyword(KeywordKind::SelfValue)
        && input.at_name_like();
    // Target-shaped arms use the ordinary postfix grammar, including generic
    // applications, operational acknowledgements and nested postfix operands.
    // Classification below may choose a plain named arrival only when that
    // representation retains the complete invocation payload.
    // A following negative pattern belongs to the next arm, not subtraction
    // from this target. Arithmetic arm values retain their grouped spelling.
    let (expression, rest) = if target_shaped {
        parse_postfix_expression_handle(syntax_trees, input, ExpressionContext::Default)?
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
        ExpressionNode::Call(call)
            if target_shaped && call.target_is_static && call.machine_arguments.is_empty() =>
        {
            classify_call_target_handle(syntax_trees, call)?
        }
        _ => TransitionTargetNode::Value(expression),
    };

    Ok(syntax_trees.statements.insert_transition_target(target))
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
    let path = if call.receiver.is_valid() {
        let receiver =
            copy_expression_identifier_path_to_statement_table(syntax_trees, call.receiver)
                .ok_or_else(|| ParseError::new("call target must be a path or member access"))?;
        append_statement_identifier_path_member(syntax_trees, receiver, call.target.clone())
    } else {
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(call.target.clone());
        HandleSpan::from_parts(handle, 1)
    };

    let arguments = copy_expression_handles_to_statement_table(syntax_trees, call.arguments);
    Ok(TransitionTargetNode::Named {
        path,
        arguments,
        evidence_arguments: call.evidence_arguments,
        source_span: call.target.source_span(),
    })
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
        _ => None,
    }
}
