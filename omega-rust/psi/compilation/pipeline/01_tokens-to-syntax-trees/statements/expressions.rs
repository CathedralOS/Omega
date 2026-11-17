use crate::errors::parse_error::ParseError;
use crate::syntax_storage::statement_paths::copy_expression_handles_to_statement_table;
use crate::syntax_storage::tables::{exprs, exprs_of};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableIndexedExpression,
    TableMemberExpression,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::TableCall;

pub(crate) fn copy_expression_as_place(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let node = exprs_of(syntax_trees).expression(expression).clone();
    let copied = match node {
        ExpressionNode::Member(m) => {
            let recv = copy_expression_as_place(syntax_trees, m.receiver)?;
            ExpressionNode::Member(TableMemberExpression {
                receiver: recv,
                access_source_span: m.access_source_span,
                member: m.member,
                case_variant: m.case_variant.clone(),
            })
        }
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => {
            let new_path = syntax_trees
                .expressions
                .copy_identifier_path_prefix(path.to_owned(), path.to_owned().len());
            ExpressionNode::Name {
                members: new_path,
                application: None,
            }
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
    Some(exprs(syntax_trees).insert(copied))
}

pub(crate) fn root_binding_declaration(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Result<Option<syntax_trees::statement::RootBinding>, ParseError> {
    let ExpressionNode::Call(call) = exprs_of(syntax_trees).expression(expression) else {
        return Ok(None);
    };
    if call.target.as_str(syntax_trees) == "bind"
        && !call.target_is_static
        && call.receiver.is_valid()
        && matches!(exprs_of(syntax_trees).expression(call.receiver),
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
    let ExpressionNode::Name {
        members: owner,
        application: owner_application,
    } = exprs_of(syntax_trees).expression(call.receiver)
    else {
        return Ok(None);
    };
    if owner_application.is_some() {
        return Ok(None);
    }
    let owner = exprs_of(syntax_trees).identifier_path_members(owner.to_owned());
    if !matches!(owner, [name] if name.as_str(syntax_trees) == "Build") {
        return Ok(None);
    }
    if !call.machine_arguments.is_empty() {
        return Ok(None);
    }
    let invalid = || {
        ParseError::new(
            "root-slot binding requires an explicit Build argument, one slot path and one implementation path or description expression",
        )
    };
    let [builder, slot, implementation] = exprs_of(syntax_trees).expression_handles(call.arguments)
    else {
        return Err(invalid());
    };
    if !call.machine_arguments.is_empty() || !call.evidence_arguments.is_empty() {
        return Err(invalid());
    }
    let operand = |handle| {
        let ExpressionNode::Name {
            members: path,
            application: path_application,
        } = exprs_of(syntax_trees).expression(handle)
        else {
            return Err(invalid());
        };
        if path_application.is_some() {
            return Err(invalid());
        }
        Ok(syntax_trees
            .expressions
            .identifier_path_members(path.to_owned())
            .to_vec()
            .into_boxed_slice())
    };
    let described_operand = |handle: ExpressionHandle| {
        let ExpressionNode::Name {
            members: path,
            application: path_application,
        } = exprs_of(syntax_trees).expression(handle)
        else {
            return handle;
        };
        let members = exprs_of(syntax_trees).identifier_path_members(path.to_owned());
        if path_application.is_none()
            && members.len() == 1
            && !members[0].is_self_receiver(syntax_trees)
        {
            handle
        } else {
            ExpressionHandle::invalid()
        }
    };
    Ok(Some(syntax_trees::statement::RootBinding {
        receiver: *builder,
        slot: operand(*slot)?,
        implementation: if matches!(
            exprs_of(syntax_trees).expression(*implementation),
            ExpressionNode::Name { .. }
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
    let ExpressionNode::Call(call) = exprs_of(syntax_trees).expression(expression).clone() else {
        return None;
    };

    let (receiver, target) = split_expression_call_handle(syntax_trees, &call)?;
    Some(TableCall {
        callee: call.callee,
        target_is_static: call.target_is_static,
        receiver,
        target,
        machine_arguments: call.machine_arguments,
        arguments: copy_expression_handles_to_statement_table(syntax_trees, call.arguments),
        evidence_arguments: call.evidence_arguments,
        operational_acknowledgement: call.operational_acknowledgement,
        discards_result: false,
    })
}

fn split_expression_call_handle(
    syntax_trees: &mut SyntaxTrees,
    call: &TableCallExpression,
) -> Option<(HandleSpan<Identifier>, Identifier)> {
    let receiver = if call.receiver.is_valid() {
        crate::syntax_storage::statement_paths::copy_expression_to_statement_identifier_path(
            syntax_trees,
            call.receiver,
        )?
    } else {
        HandleSpan::empty()
    };

    Some((receiver, call.target.clone()))
}
