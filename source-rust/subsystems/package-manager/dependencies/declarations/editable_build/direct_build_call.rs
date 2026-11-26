// Validates the canonical direct `Build::<operation>(builder, ...)` call shape
// shared by role and dependency syntax projectors: a static `Build` owner whose
// explicit first operand names the root entry's Build parameter, with no
// generic, evidence, or operational modifiers.

use build_declarations::BUILD_TYPE_NAME;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::TableCallExpression;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectBuildCallError {    WrongReceiver, // Owner was not static `Build`, or first operand was not the entry receiver.
    WrongArguments, // Call carried generic, evidence, or operational modifiers, or omitted the receiver.
}

pub(crate) fn direct_build_call_operands<'syntax>(
    syntax_trees: &'syntax SyntaxTrees,
    call: &TableCallExpression,
    receiver_name: &str,
) -> Result<&'syntax [ExpressionHandle], DirectBuildCallError> {
    if !call.target_is_static
        || !matches!(syntax_trees.expression_identifier_path(call.receiver).as_deref(), Some([owner]) if owner.as_str(syntax_trees) == BUILD_TYPE_NAME)
    {
        return Err(DirectBuildCallError::WrongReceiver);
    }
    if !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.operational_acknowledgement != Default::default()
    {
        return Err(DirectBuildCallError::WrongArguments);
    }
    let operands = syntax_trees.expressions.expression_handles(call.arguments);
    let Some((builder, operands)) = operands.split_first() else {
        return Err(DirectBuildCallError::WrongArguments);
    };
    if !matches!(syntax_trees.expressions.expression(*builder), ExpressionNode::Name { members: path, application: path_application }
        if path_application.is_none() && matches!(syntax_trees.expressions.identifier_path_members(path.to_owned()), [name] if name.as_str(syntax_trees) == receiver_name))
    {
        return Err(DirectBuildCallError::WrongReceiver);
    }
    Ok(operands)
}
