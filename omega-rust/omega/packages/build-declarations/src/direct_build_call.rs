// Validates the canonical direct `Build::<operation>(builder, ...)` call shape
// shared by role and dependency syntax projectors: a static `Build` owner whose
// explicit first operand is the root entry's `builder` parameter, with no
// generic, evidence, operational, or result-discard modifiers.

use crate::build_declaration::{BUILD_TYPE_NAME, BUILDER_PARAMETER_NAME};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::statement::TableCall;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectBuildCallError {
    WrongReceiver,  // Owner was not static `Build`, or first operand was not `builder`.
    WrongArguments, // Call carried generic, evidence, operational, or discard modifiers, or omitted `builder`.
}

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
        if path.application.is_none() && matches!(syntax_trees.expressions.identifier_path_members(path.members), [name] if name.as_str(syntax_trees) == BUILDER_PARAMETER_NAME))
    {
        return Err(DirectBuildCallError::WrongReceiver);
    }
    Ok(operands)
}
