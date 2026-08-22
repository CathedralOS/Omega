mod arguments;
mod calls;
mod hoist_temp_type;
mod transitions;

use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_handle_from_table;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

use self::arguments::lower_statement_expression;
use self::calls::lower_call_statement;
use self::hoist_temp_type::infer_hoist_temp_type;
use self::transitions::lower_transition_statement;

pub(crate) fn lower_statement_node(
    lowerer: &mut Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    statement: &resolved::statement::StatementNode,
) -> Result<typed::statement::StatementNode, Diagnostic> {
    match statement {
        resolved::statement::StatementNode::AssemblyFact(fact) => Ok(
            typed::statement::StatementNode::AssemblyFact(typed::statement::TableAssemblyFact {
                kind: match fact.kind {
                    resolved::statement::AssemblyFactKind::Requires => {
                        typed::statement::AssemblyFactKind::Requires
                    }
                    resolved::statement::AssemblyFactKind::Ensures => {
                        typed::statement::AssemblyFactKind::Ensures
                    }
                },
                expression: lower_statement_expression(lowerer, fact.expression)?,
            }),
        ),
        resolved::statement::StatementNode::Assignment(assignment) => Ok(
            typed::statement::StatementNode::Assignment(typed::statement::TableAssignment {
                target: lower_statement_expression(lowerer, assignment.target)?,
                value: lower_statement_expression(lowerer, assignment.value)?,
            }),
        ),
        resolved::statement::StatementNode::Call(call) => Ok(
            typed::statement::StatementNode::Call(lower_call_statement(lowerer, call)?),
        ),
        resolved::statement::StatementNode::ProofOutputBindingStatement(_) => {
            unreachable!("evidence packages are classified out before runtime statement lowering")
        }
        resolved::statement::StatementNode::Expression(expression) => {
            Ok(typed::statement::StatementNode::Expression(
                lower_statement_expression(lowerer, *expression)?,
            ))
        }
        resolved::statement::StatementNode::LocalData(local_data) => {
            // A hoist temp (`let __hoist_N = self.arr[i];`) is lowered with a
            // `Unit` type by the operand-hoisting normalization, the inference
            // sentinel. Fill in the element type (carrying the collection's
            // arithmetic domain) from the indexed-read initializer now that the
            // resolved data-field types are available -- the language has no
            // local type inference, so the temp needs a concrete declared type
            // for its place/domain/range/layout. Other locals lower as written.
            let declared_is_unit = matches!(
                lowerer
                    .source_trees
                    .tables
                    .types
                    .references
                    .type_reference(local_data.type_reference),
                resolved::types::TypeReferenceNode::Unit
            );
            let type_reference = if declared_is_unit && local_data.initial_value.is_valid() {
                infer_hoist_temp_type(lowerer, attached_data, state, local_data.initial_value)?
            } else {
                None
            };
            let type_reference = match type_reference {
                Some(type_reference) => type_reference,
                None => lower_type_reference_handle_from_table(lowerer, local_data.type_reference)?,
            };
            Ok(typed::statement::StatementNode::LocalData(
                typed::statement::TableLocalData {
                    symbol: local_data.symbol,
                    name: crate::name::lower_name(&local_data.name),
                    type_reference,
                    initial_value: local_data
                        .initial_value
                        .is_valid()
                        .then(|| lower_statement_expression(lowerer, local_data.initial_value))
                        .transpose()?
                        .unwrap_or_else(typed::expression::ExpressionHandle::invalid),
                    is_mutable: local_data.is_mutable,
                },
            ))
        }
        resolved::statement::StatementNode::Transition(transition) => {
            Ok(typed::statement::StatementNode::Transition(
                lower_transition_statement(lowerer, transition)?,
            ))
        }
    }
}
