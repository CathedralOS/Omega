use crate::errors::parse_error::ParseError;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::statement::TableCall;

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
        return Err(ParseError::at_source_span(
            "receiver-style root binding is retired; use Build::bind(builder, slot, entry)",
            call.target.source_span(),
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
    } = syntax_trees.expressions.expression(call.receiver)
    else {
        return Ok(None);
    };
    if owner_application.is_some() {
        return Ok(None);
    }
    let owner = syntax_trees
        .expressions
        .identifier_path_members(owner.to_owned());
    if !matches!(owner, [name] if name.as_str(syntax_trees) == "Build") {
        return Ok(None);
    }
    if !call.machine_arguments.is_empty() {
        return Ok(None);
    }
    // A wrong argument count faults the `bind` being called; a slot or
    // implementation of the wrong shape faults that operand.
    let invalid = |source_span| {
        ParseError::at_source_span(
            "root-slot binding requires an explicit Build argument, one slot path and one implementation path or description expression",
            source_span,
        )
    };
    let binding = call.target.source_span();
    let [builder, slot, implementation] =
        syntax_trees.expressions.expression_handles(call.arguments)
    else {
        return Err(invalid(binding));
    };
    if !call.machine_arguments.is_empty() || !call.evidence_arguments.is_empty() {
        return Err(invalid(binding));
    }
    let operand = |handle| {
        // Not every expression kind records where it was written; an
        // operand without a span falls back to the `bind` it was passed to.
        let recorded = syntax_trees.expressions.source_span(handle);
        let operand_span = if recorded.span.end > recorded.span.start {
            recorded
        } else {
            binding
        };
        let ExpressionNode::Name {
            members: path,
            application: path_application,
        } = syntax_trees.expressions.expression(handle)
        else {
            return Err(invalid(operand_span));
        };
        if path_application.is_some() {
            return Err(invalid(operand_span));
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
        } = syntax_trees.expressions.expression(handle)
        else {
            return handle;
        };
        let members = syntax_trees
            .expressions
            .identifier_path_members(path.to_owned());
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
            syntax_trees.expressions.expression(*implementation),
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
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<TableCall> {
    matches!(
        syntax_trees.expressions.expression(expression),
        ExpressionNode::Call(_)
    )
    .then_some(TableCall {
        expression,
        discards_result: false,
    })
}
