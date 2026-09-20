//! Copying expression handles and identifier paths into the statement
//! table.

use crate::diagnostics::parse_error::ParseError;
use arena::{Handle, HandleSpan};
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
    expr: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let node = syntax_trees.expressions.expression(expr).clone();
    let copied = match node {
        ExpressionNode::SelfValue => ExpressionNode::SelfValue,
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
    if call.target.as_str() != "bind" || !call.receiver.is_valid() {
        return Ok(None);
    }
    let ExpressionNode::Member(member) = syntax_trees.expressions.expression(call.receiver) else {
        return Ok(None);
    };
    if member.member.as_str() != "roots" {
        return Ok(None);
    }
    let invalid = || {
        ParseError::new(
            "root-slot binding requires exactly one slot path and one implementation path or description expression",
        )
    };
    let [slot, implementation] = syntax_trees.expressions.expression_handles(call.arguments) else {
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
        if members.len() == 1 && members[0].as_str() != "self" {
            handle
        } else {
            ExpressionHandle::invalid()
        }
    };
    Ok(Some(syntax_trees::statement::RootBinding {
        receiver: member.receiver,
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
        receiver: receiver.members,
        receiver_starts_at_self: receiver.starts_at_self,
        target,
        machine_arguments: call.machine_arguments,
        arguments: copy_expression_handles_to_statement_table(syntax_trees, call.arguments),
        evidence_arguments: call.evidence_arguments,
        operational_acknowledgement: call.operational_acknowledgement,
        discards_result: false,
    })
}

struct StatementIdentifierPath {
    members: HandleSpan<syntax_trees::identifier::Identifier>,
    starts_at_self: bool,
}

fn split_expression_call_handle(
    syntax_trees: &mut SyntaxTrees,
    call: &TableCallExpression,
) -> Option<(
    StatementIdentifierPath,
    syntax_trees::identifier::Identifier,
)> {
    let receiver = if call.receiver.is_valid() {
        expression_handle_to_identifier_path_span(syntax_trees, call.receiver)?
    } else {
        StatementIdentifierPath {
            members: HandleSpan::empty(),
            starts_at_self: false,
        }
    };

    Some((receiver, call.target.clone()))
}

fn expression_handle_to_identifier_path_span(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<StatementIdentifierPath> {
    match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Name(path) => Some(StatementIdentifierPath {
            members: copy_expression_identifier_path_to_statement_table(syntax_trees, path),
            starts_at_self: false,
        }),
        ExpressionNode::SelfValue => {
            let self_member = syntax_trees.statements.append_identifier_path_member(
                syntax_trees::identifier::Identifier::generated("self"),
            );
            Some(StatementIdentifierPath {
                members: HandleSpan::from_parts(self_member, 1),
                starts_at_self: true,
            })
        }
        ExpressionNode::Member(member) => {
            let mut receiver =
                expression_handle_to_identifier_path_span(syntax_trees, member.receiver)?;
            receiver.members = append_statement_identifier_path_member(
                syntax_trees,
                receiver.members,
                member.member,
            );
            Some(receiver)
        }
        _ => None,
    }
}

fn copy_expression_identifier_path_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<syntax_trees::identifier::Identifier>,
) -> HandleSpan<syntax_trees::identifier::Identifier> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    let member_count = syntax_trees.expressions.identifier_path_members(path).len();

    for index in 0..member_count {
        let member = syntax_trees.expressions.identifier_path_members(path)[index].clone();
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(member);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("statement identifier path span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn append_statement_identifier_path_member(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<syntax_trees::identifier::Identifier>,
    member: syntax_trees::identifier::Identifier,
) -> HandleSpan<syntax_trees::identifier::Identifier> {
    let handle = syntax_trees
        .statements
        .append_identifier_path_member(member);

    if path.is_empty() {
        HandleSpan::from_parts(handle, 1)
    } else {
        HandleSpan::from_parts(
            path.start(),
            path.count()
                .checked_add(1)
                .expect("statement identifier path span count overflow"),
        )
    }
}

fn copy_expression_handles_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    arguments: HandleSpan<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    let arguments = syntax_trees
        .tables
        .expressions
        .expression_handles(arguments)
        .to_vec();

    for argument in arguments {
        let handle = syntax_trees
            .tables
            .statements
            .append_expression_handle(argument);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("statement call argument span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}
