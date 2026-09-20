//! Saved constructor projections with checked, point-specific index values.
//!
//! Index arithmetic uses the ordinary scalar plan and evaluator, preserving
//! selected operations, carrier width and arithmetic policy. Facts belong to
//! the index's statement, not a later consumer: local aliases may lead back to
//! an earlier snapshot. Writes within that statement conservatively refuse
//! substitution because a sibling operand may have changed the observed value.

use checked_trees::{CheckedOperatorFacts, CrashPredicateExpression, FlowFacts, FlowStateFact};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::StatementNode;

pub(in crate::facts) fn entry_value(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    semantic: &facts::FactPlan,
    flow: &FlowFacts,
    state_flow: &FlowStateFact,
    before_statement: usize,
    expression: ExpressionHandle,
) -> Option<CrashPredicateExpression> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state_flow.machine_symbol)?;
    super::literal_projection::entry_value_with_selector(
        program,
        machine.symbol,
        state_flow.state_symbol,
        before_statement,
        expression,
        0,
        &mut |state, statement_index, selector| {
            if !validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                selector,
            ) {
                return None;
            }
            let primitive = program.primitive_type_reference(
                validation::expression_result_type_reference(program, machine, state, selector)?,
            )?;
            let lowered = crate::values::lower_unit_scalar_argument(
                program,
                operators,
                state,
                statement_index,
                selector,
                primitive,
            )?;
            let statements = program.statement_table.statements(state.statement_nodes);
            let statement = statements.get(statement_index)?;
            let parameters = program.state_parameters(state);
            let symbols = parameters
                .iter()
                .filter(|parameter| {
                    program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                })
                .map(|parameter| parameter.symbol)
                .chain(
                    statements.get(..statement_index)?.iter().filter_map(
                        |statement| match statement {
                            StatementNode::LocalData(local)
                                if !local.is_mutable
                                    && program
                                        .primitive_type_reference(local.type_reference)
                                        .is_some() =>
                            {
                                Some(local.symbol)
                            }
                            _ => None,
                        },
                    ),
                )
                .collect::<Vec<_>>();
            let context_span = flow
                .state_statement(state_flow, statement_index)?
                .entry_semantic_contexts;
            let contexts = flow
                .contexts
                .semantic_context_refs
                .span_or_empty(context_span);
            let value = crate::values::evaluate_checked_scalar(
                &lowered,
                &mut crate::values::PlaceScalarValues {
                    program,
                    parameters,
                    symbols: &symbols,
                    value_at_place: |place: &crate::flow::CanonicalPlace| {
                        if super::statement_may_overwrite_place(
                            program,
                            machine.symbol,
                            statement,
                            place,
                        ) {
                            return None;
                        }
                        crate::values::scalar_value_at_place(
                            program,
                            semantic,
                            contexts
                                .iter()
                                .map(|reference| semantic.contexts.get(reference.context)),
                            place,
                        )
                    },
                },
            )?;
            let facts::ScalarValue::Integer(value) = value else {
                return None;
            };
            usize::try_from(value.to_u64()?).ok()
        },
    )
}
