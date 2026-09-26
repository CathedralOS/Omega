use typed_trees_to_checked_trees::checked_trees::CheckedTrees;
use typed_trees_to_checked_trees::checked_trees::statement::StatementNode;
use typed_trees_to_checked_trees::checked_trees::{
    CheckedArrayConstructionSource, CheckedUnitCallCoordinate,
};

/// Reconstruct the constructor's expression and declared type from its owner.
/// Formal positions, rather than filtered array positions, distinguish equal
/// typed actuals. This join remains mandatory when an array has no leaves.
pub(crate) fn construction_expression(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    source: CheckedArrayConstructionSource,
) -> Option<(
    typed_trees_to_checked_trees::checked_trees::expression::ExpressionHandle,
    typed_trees_to_checked_trees::checked_trees::types::TypeReferenceHandle,
)> {
    let (owner, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state).ok()?;
    if owner.symbol != machine {
        return None;
    }
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    match source {
        CheckedArrayConstructionSource::Statement => {
            match statements.get(statement_index as usize)? {
                StatementNode::LocalData(local) if !local.is_mutable => {
                    Some((local.initial_value, local.type_reference))
                }
                StatementNode::Expression(expression)
                    if statement_index as usize + 1 == statements.len() =>
                {
                    Some((*expression, state.return_type))
                }
                // A whole array-literal replacement: the literal lands at its
                // target place's declared type.
                StatementNode::Assignment(assignment) => {
                    typed_trees_to_checked_trees::validation::declared_place_type_raw(
                        &checked.typed,
                        owner,
                        Some(state),
                        assignment.target,
                    )
                    .and_then(|declared| {
                        typed_trees_to_checked_trees::validation::closed_array_store_type(
                            &checked.typed,
                            declared,
                        )
                    })
                    .map(|reference| (assignment.value, reference))
                }
                _ => None,
            }
        }
        CheckedArrayConstructionSource::CallArgument {
            call_ordinal,
            parameter_position,
        } => {
            let call = crate::emission::call_source_custody::authored::locate_source(
                checked,
                state.symbol,
                CheckedUnitCallCoordinate {
                    statement_index,
                    call_ordinal,
                },
            )
            .ok()?;
            let target = crate::emission::call_source_custody::authored::target_signature(
                checked,
                machine,
                call.source_target,
            )
            .ok()?;
            if target.boundary {
                return None;
            }
            let parameter = target.parameters.get(parameter_position as usize)?;
            if parameter.is_self
                || parameter.is_mutable
                || parameter.is_const
                || !typed_trees_to_checked_trees::validation::is_closed_primitive_array_type(
                    &checked.typed,
                    parameter.type_reference,
                )
            {
                return None;
            }
            let mut actuals = call
                .structural_arguments
                .iter()
                .filter(|(position, _)| *position == parameter_position);
            let (_, expression) = actuals.next()?;
            if actuals.next().is_some() {
                return None;
            }
            typed_trees_to_checked_trees::validation::scalar_array_elements(
                &checked.typed,
                machine,
                *expression,
                parameter.type_reference,
            )?;
            Some((*expression, parameter.type_reference))
        }
    }
}
