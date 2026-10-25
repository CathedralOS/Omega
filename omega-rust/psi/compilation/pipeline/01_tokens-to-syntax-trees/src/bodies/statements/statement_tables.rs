//! Copying expression handles and identifier paths into the statement
//! table.

use crate::diagnostics::parse_error::ParseError;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableIndexedExpression,
    TableMemberExpression,
};
use syntax_trees::statement::TableCall;

/// Deep-copy an expression that is a valid place (member / name / indexed /
/// self), returning a fresh handle with the same structure.  Returns `None`
/// for non-place expression shapes (binary, call, etc.) since those cannot
/// appear on the left-hand side of an assignment.
pub(crate) fn copy_expression_as_place(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let node = syntax_trees.expressions.expression(expression).clone();
    let copied = match node {
        ExpressionNode::Member(m) => {
            let recv = copy_expression_as_place(syntax_trees, m.receiver)?;
            ExpressionNode::Member(TableMemberExpression {
                receiver: recv,
                member: m.member,
                case_variant: m.case_variant.clone(),
            })
        }
        ExpressionNode::Name(path) => {
            let new_path = syntax_trees
                .expressions
                .copy_identifier_path_prefix(path, path.len());
            ExpressionNode::Name(new_path)
        }
        ExpressionNode::Indexed(idx) => {
            let coll = copy_expression_as_place(syntax_trees, idx.collection)?;
            ExpressionNode::Indexed(TableIndexedExpression {
                collection: coll,
                index: idx.index,
            })
        }
        ExpressionNode::Borrow(inner) => {
            let inner_copy = copy_expression_as_place(syntax_trees, inner.target)?;
            ExpressionNode::Borrow(syntax_trees::expression::TableBorrowExpression {
                target: inner_copy,
                access: inner.access,
            })
        }
        _ => return None,
    };
    Some(syntax_trees.expressions.insert(copied))
}

pub(crate) fn root_binding_declaration(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Result<Option<syntax_trees::statement::RootBinding>, ParseError> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression) else {
        return Ok(None);
    };
    if call.target.as_str(syntax_trees) == "bind"
        && !call.target_is_static
        && call.receiver.is_valid()
        && matches!(syntax_trees.expressions.expression(call.receiver),
            ExpressionNode::Member(member) if member.member.as_str(syntax_trees) == "roots")
    {
        return Err(ParseError::new(
            "receiver-style root binding is retired; use Build::bind(builder, slot, entry)",
        ));
    }
    if call.target.as_str(syntax_trees) != "bind"
        || !call.target_is_static
        || !call.receiver.is_valid()
    {
        return Ok(None);
    }
    let ExpressionNode::Name(owner) = syntax_trees.expressions.expression(call.receiver) else {
        return Ok(None);
    };
    let owner = syntax_trees.expressions.identifier_path_members(*owner);
    if !matches!(owner, [name] if name.as_str(syntax_trees) == "Build") {
        return Ok(None);
    }
    let invalid = || {
        ParseError::new(
            "root-slot binding requires an explicit Build argument, one slot path and one implementation path or description expression",
        )
    };
    let [builder, slot, implementation] =
        syntax_trees.expressions.expression_handles(call.arguments)
    else {
        return Err(invalid());
    };
    if !call.machine_arguments.is_empty() || !call.evidence_arguments.is_empty() {
        return Err(invalid());
    }
    let operand = |handle| {
        let ExpressionNode::Name(path) = syntax_trees.expressions.expression(handle) else {
            return Err(invalid());
        };
        Ok(syntax_trees
            .expressions
            .identifier_path_members(*path)
            .to_vec()
            .into_boxed_slice())
    };
    // A bare operand name may be a delegated `ProductEntryRef` place rather
    // than a product declaration path; keep its expression so name resolution
    // can decide. Non-name operands are ordinary description expressions;
    // they have no product declaration path to fall back to. Multi-member
    // declaration paths stay purely lexical.
    let described_operand = |handle: ExpressionHandle| {
        let ExpressionNode::Name(path) = syntax_trees.expressions.expression(handle) else {
            return handle;
        };
        let members = syntax_trees.expressions.identifier_path_members(*path);
        if members.len() == 1 && !members[0].is_self_receiver(syntax_trees) {
            handle
        } else {
            ExpressionHandle::invalid()
        }
    };
    Ok(Some(syntax_trees::statement::RootBinding {
        receiver: *builder,
        slot: operand(*slot)?,
        implementation: if matches!(
            syntax_trees.expressions.expression(*implementation),
            ExpressionNode::Name(_)
        ) {
            operand(*implementation)?
        } else {
            Box::default()
        },
        implementation_operand: described_operand(*implementation),
        source_span: call.target.source_span(),
    }))
}

pub(crate) fn expression_handle_to_statement_call(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<TableCall> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression).clone() else {
        return None;
    };

    let (receiver, target) = split_expression_call_handle(syntax_trees, &call)?;
    Some(TableCall {
        target_is_static: call.target_is_static,
        receiver,
        target,
        machine_arguments: call.machine_arguments,
        arguments: crate::bodies::copy_expression_handles_to_statement_table(
            syntax_trees,
            call.arguments,
        ),
        evidence_arguments: call.evidence_arguments,
        operational_acknowledgement: call.operational_acknowledgement,
        discards_result: false,
    })
}

fn split_expression_call_handle(
    syntax_trees: &mut SyntaxTrees,
    call: &TableCallExpression,
) -> Option<(
    HandleSpan<syntax_trees::identifier::Identifier>,
    syntax_trees::identifier::Identifier,
)> {
    let receiver = if call.receiver.is_valid() {
        crate::bodies::copy_expression_to_statement_identifier_path(syntax_trees, call.receiver)?
    } else {
        HandleSpan::empty()
    };

    Some((receiver, call.target.clone()))
}
