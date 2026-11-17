use crate::errors::parse_error::ParseError;
use crate::expressions::context::ExpressionContext;
use crate::expressions::operators::has_unary_expression_prefix;
use crate::expressions::parse::parse_expression_handle;
use crate::expressions::postfix::parse_postfix_expression_handle;
use crate::syntax_storage::statement_paths::{
    append_identifier_path_member_to_statement_table, copy_expression_handles_to_statement_table,
    copy_expression_to_statement_identifier_path,
};
use crate::syntax_storage::tables::{exprs, exprs_of, stmts};
use crate::token_input::cursor::{Input, ParseResult};
use crate::transitions::guards::{DestructureBindings, rewrite_destructure_guard_expression};
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
        )?,
        rest,
    ))
}

fn classify_transition_target_handle(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    target_shaped: bool,
) -> Result<TransitionTargetHandle, ParseError> {
    let node = exprs_of(syntax_trees).expression(expression).clone();
    let target = match node {
        ExpressionNode::Call(call)
            if target_shaped && call.target_is_static && call.machine_arguments.is_empty() =>
        {
            classify_call_target_handle(syntax_trees, call)?
        }
        _ => TransitionTargetNode::Value(expression),
    };

    Ok(stmts(syntax_trees).insert_transition_target(target))
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
            exprs(syntax_trees).insert(ExpressionNode::Call(call)),
        ));
    };
    if receiver_depth > 1 {
        return Ok(TransitionTargetNode::Value(
            exprs(syntax_trees).insert(ExpressionNode::Call(call)),
        ));
    }
    let path = if call.receiver.is_valid() {
        let receiver = copy_expression_to_statement_identifier_path(syntax_trees, call.receiver)
            .ok_or_else(|| ParseError::new("call target must be a path or member access"))?;
        append_identifier_path_member_to_statement_table(
            syntax_trees,
            receiver,
            call.target.clone(),
        )
    } else {
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(call.target.clone());
        HandleSpan::from_parts(handle, 1)
    };

    let arguments = copy_expression_handles_to_statement_table(syntax_trees, call.arguments);
    Ok(TransitionTargetNode::Named {
        operational_acknowledgement: call.operational_acknowledgement,
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
    match exprs_of(syntax_trees).expression(expression) {
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => Some(
            exprs_of(syntax_trees)
                .identifier_path_members(path.to_owned())
                .len(),
        ),
        ExpressionNode::Member(member) => {
            let depth = expression_handle_identifier_depth(syntax_trees, member.receiver)?;
            Some(depth + 1)
        }
        _ => None,
    }
}
