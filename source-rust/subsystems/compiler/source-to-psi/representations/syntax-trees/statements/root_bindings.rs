//! Read the authored operands of a potential Build root binding. The caller
//! selects the owner before validating a call as a Build operation.

use crate::SyntaxTrees;
use crate::expression::{ExpressionHandle, ExpressionNode};
use source_text::SourceSpan;

pub const BLOCK_VALUE_STATEMENT_ERROR: &str = "a block value holds `let` bindings, assignments, calls, `transition` and `crash` statements before its final expression; an `asm`, root or proof-output binding statement cannot appear in it";

fn at(message: &'static str, span: SourceSpan) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic::error(message).with_source_span(span)
}

pub fn is_root_binding_candidate(
    syntax_trees: &SyntaxTrees,
    call: &crate::expression::TableCallExpression,
) -> bool {
    if call.target.as_str(syntax_trees) != "bind"
        || !call.target_is_static
        || !call.receiver.is_valid()
    {
        return false;
    }
    let ExpressionNode::Name {
        members: owner,
        application: owner_application,
    } = syntax_trees.expressions.expression(call.receiver)
    else {
        return false;
    };
    if owner_application.is_some() {
        return false;
    }
    let owner = syntax_trees
        .expressions
        .identifier_path_members(owner.to_owned());
    if !matches!(owner, [name] if name.as_str(syntax_trees) == "Build") {
        return false;
    }
    call.machine_arguments.is_empty()
}

pub fn root_binding_declaration(
    syntax_trees: &SyntaxTrees,
    call: &crate::expression::TableCallExpression,
) -> Result<crate::statement::RootBinding, diagnostics::Diagnostic> {
    // A wrong argument count faults the `bind` being called; a slot or
    // implementation of the wrong shape faults that operand.
    let invalid = |source_span| {
        at(
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
    Ok(crate::statement::RootBinding {
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
    })
}
