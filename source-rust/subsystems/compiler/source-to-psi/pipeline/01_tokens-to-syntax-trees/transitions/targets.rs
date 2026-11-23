use crate::expressions::context::ExpressionContext;
use crate::expressions::operators::has_unary_expression_prefix;
use crate::expressions::parse::parse_expression_handle;
use crate::expressions::postfix::parse_postfix_expression_handle;
use crate::token_input::cursor::{Input, ParseResult};
use crate::transitions::guards::{DestructureBindings, rewrite_destructure_guard_expression};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::statement::{TransitionTargetHandle, TransitionTargetNode};
use tokens::KeywordKind;

pub(crate) fn parse_transition_block_target_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TransitionTargetHandle> {
    parse_transition_block_target_with_bindings(syntax_trees, input, &[])
}

pub(crate) fn parse_transition_block_target_with_bindings<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    bindings: &[DestructureBindings],
) -> ParseResult<'tokens, 'source, TransitionTargetHandle> {
    // An acknowledgement changes the call envelope, not
    // whether a static path is a terminal machine target.
    // Keep the original parse mode for prefixes and classify
    // only the complete call afterwards.
    let acknowledged = input.at_contextual("suspend") || input.at_contextual("block");
    let mut acknowledged_path = input;
    if acknowledged_path.at_contextual("suspend") {
        acknowledged_path = acknowledged_path.take_contextual("suspend")?;
    }
    if acknowledged_path.at_contextual("block") {
        acknowledged_path = acknowledged_path.take_contextual("block")?;
    }
    let acknowledged_target = acknowledged && acknowledged_path.at_name_like();
    let target_shaped = !has_unary_expression_prefix(input)?
        && !input.at_keyword(KeywordKind::True)
        && !input.at_keyword(KeywordKind::False)
        && !input.at_keyword(KeywordKind::SelfValue)
        && input.at_name_like();
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
        classify_transition_target_handle(
            syntax_trees,
            expression,
            target_shaped || acknowledged_target,
        ),
        rest,
    ))
}

fn classify_transition_target_handle(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    target_shaped: bool,
) -> TransitionTargetHandle {
    let target = match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Call(call)
            if target_shaped
                && call.target_is_static
                && call.machine_arguments.is_empty()
                && let Some(receiver) = syntax_trees.expression_identifier_path(call.receiver)
                && receiver.len() <= 1 =>
        {
            TransitionTargetNode::Named(expression)
        }
        _ => TransitionTargetNode::Value(expression),
    };
    syntax_trees.statements.insert_transition_target(target)
}
