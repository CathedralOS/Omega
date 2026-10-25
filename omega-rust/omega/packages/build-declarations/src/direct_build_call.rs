//! The one direct `Build::<operation>(builder, ...)` call shape every
//! declaration consumer projects from: a static `Build` owner whose explicit
//! first operand is the entry's `builder` parameter, with no generic,
//! evidence, operational, or discard modifiers.

use crate::build_declaration::{BUILD_TYPE_NAME, BUILDER_PARAMETER_NAME};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::statement::TableCall;

/// How one direct `Build::<operation>(...)` statement departed from the
/// canonical call shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectBuildCallError {
    /// The owner was not a static `Build` selection, or the explicit first
    /// operand was not the root build entry's `builder` parameter.
    WrongReceiver,
    /// The call carried generic, evidence, operational, or discard modifiers.
    WrongArguments,
}

/// Validate one direct `Build::<operation>(builder, ...)` call statement and
/// return its operand expressions after the explicit builder operand.
pub(crate) fn direct_build_call_operands<'syntax>(
    syntax_trees: &'syntax SyntaxTrees,
    call: &TableCall,
) -> Result<&'syntax [ExpressionHandle], DirectBuildCallError> {
    if !call.target_is_static
        || !matches!(syntax_trees.statements.identifier_path_members(call.receiver), [owner] if owner.as_str(syntax_trees) == BUILD_TYPE_NAME)
    {
        return Err(DirectBuildCallError::WrongReceiver);
    }
    if !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.operational_acknowledgement != Default::default()
        || call.discards_result
    {
        return Err(DirectBuildCallError::WrongArguments);
    }
    let operands = syntax_trees.statements.expression_handles(call.arguments);
    let Some((builder, operands)) = operands.split_first() else {
        return Err(DirectBuildCallError::WrongArguments);
    };
    if !matches!(syntax_trees.expressions.expression(*builder), ExpressionNode::Name(path)
        if matches!(syntax_trees.expressions.identifier_path_members(*path), [name] if name.as_str(syntax_trees) == BUILDER_PARAMETER_NAME))
    {
        return Err(DirectBuildCallError::WrongReceiver);
    }
    Ok(operands)
}
