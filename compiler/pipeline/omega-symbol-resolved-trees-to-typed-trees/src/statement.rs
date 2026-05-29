mod arguments;
mod calls;
mod transitions;

use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_handle_from_table;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

use self::arguments::lower_statement_expression;
use self::calls::lower_call_statement;
use self::transitions::lower_transition_statement;

pub(crate) fn lower_statement_node(
    lowerer: &mut Lowerer,
    statement: &resolved::statement::StatementNode,
) -> Result<typed::statement::StatementNode, Diagnostic> {
    match statement {
        resolved::statement::StatementNode::Assignment(assignment) => Ok(
            typed::statement::StatementNode::Assignment(typed::statement::TableAssignment {
                target: lower_statement_expression(lowerer, assignment.target)?,
                value: lower_statement_expression(lowerer, assignment.value)?,
            }),
        ),
        resolved::statement::StatementNode::Call(call) => Ok(
            typed::statement::StatementNode::Call(lower_call_statement(lowerer, call)?),
        ),
        resolved::statement::StatementNode::Expression(expression) => {
            Ok(typed::statement::StatementNode::Expression(
                lower_statement_expression(lowerer, *expression)?,
            ))
        }
        resolved::statement::StatementNode::LocalData(local_data) => Ok(
            typed::statement::StatementNode::LocalData(typed::statement::TableLocalData {
                symbol: local_data.symbol,
                name: crate::name::lower_name(&local_data.name),
                type_reference: lower_type_reference_handle_from_table(
                    lowerer,
                    local_data.type_reference,
                )?,
                initial_value: local_data
                    .initial_value
                    .is_valid()
                    .then(|| lower_statement_expression(lowerer, local_data.initial_value))
                    .transpose()?
                    .unwrap_or_else(typed::expression::ExpressionHandle::invalid),
            }),
        ),
        resolved::statement::StatementNode::Transition(transition) => {
            Ok(typed::statement::StatementNode::Transition(
                lower_transition_statement(lowerer, transition)?,
            ))
        }
    }
}
