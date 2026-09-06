//! Rejoin mutable entry storage to the exact current-state parameter frontier.

use super::*;

pub(crate) fn parameter_storage<'checked>(
    checked: &'checked CheckedTrees,
    machine: symbols::SymbolHandle,
    graph: &checked_trees::CheckedScalarStateGraph,
) -> Result<&'checked [checked_trees::CheckedScalarParameterStorage], LoweringError> {
    let (owner, state) = authored_state(checked, graph.state)?;
    if owner.symbol != machine {
        return unsupported("scalar parameter storage belongs to another machine");
    }
    let parameters = checked.state_parameters(state);
    if parameters.len() != graph.parameter_types.len() {
        return unsupported("scalar parameter storage disagrees with its entry arity");
    }
    let rows = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .parameter_storage
        .span(graph.parameter_storage)
        .ok_or(LoweringError::Unsupported(
            "scalar parameter storage has an invalid retained span",
        ))?;
    let mut storage = rows.iter();
    for (position, (parameter, primitive)) in
        parameters.iter().zip(&graph.parameter_types).enumerate()
    {
        if !parameter.symbol.is_valid()
            || parameters[..position]
                .iter()
                .any(|prior| prior.symbol == parameter.symbol)
            || parameter.is_self
            || parameter.is_const
            || checked.primitive_type_reference(parameter.type_reference) != Some(*primitive)
        {
            return unsupported("scalar parameter storage disagrees with its authored signature");
        }
        if !parameter.is_mutable {
            continue;
        }
        let row = storage.next().ok_or(LoweringError::Unsupported(
            "mutable scalar parameter has no retained entry storage",
        ))?;
        if !supported_mutable_parameter(*primitive)
            || usize::try_from(row.parameter_ordinal).ok() != Some(position)
            || row.symbol != parameter.symbol
            || row.primitive_type != *primitive
        {
            return unsupported(
                "scalar parameter storage disagrees with its authored mutable binding",
            );
        }
    }
    if storage.next().is_some() {
        return unsupported("scalar parameter storage contains an unauthored entry binding");
    }
    Ok(rows)
}
