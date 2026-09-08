//! Rejoin mutable entry storage to the exact current-state parameter frontier.

use super::*;

mod owned;
mod owned_types;

pub(crate) fn parameter_storage<'checked>(
    checked: &'checked CheckedTrees,
    machine: symbols::SymbolHandle,
    graph: &checked_trees::CheckedScalarStateGraph,
) -> Result<&'checked [checked_trees::CheckedScalarParameterStorage], LoweringError> {
    let (owner, state) = authored_state(checked, graph.state)?;
    if owner.symbol != machine {
        return unsupported("scalar parameter storage belongs to another machine");
    }
    owned::validate(checked, machine, state, &graph.structural_parameters)?;
    let parameters = checked.state_parameters(state);
    if parameters.len() != graph.parameter_types.len() + graph.structural_parameters.len()
        || graph.scalar_parameters.len() != graph.parameter_types.len()
    {
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
    let mut scalar_position = 0usize;
    let mut structural = graph.structural_parameters.iter();
    for (position, parameter) in parameters.iter().enumerate() {
        if !parameter.symbol.is_valid()
            || parameters[..position]
                .iter()
                .any(|prior| prior.symbol == parameter.symbol)
            || parameter.is_self
            || parameter.is_const
        {
            return unsupported("scalar parameter storage disagrees with its authored signature");
        }
        let Some(primitive) = checked.primitive_type_reference(parameter.type_reference) else {
            let retained = structural.next().ok_or(LoweringError::Unsupported(
                "scalar graph lost a structural parameter",
            ))?;
            validate_structural_parameter(checked, parameter, position, retained)?;
            continue;
        };
        let retained =
            graph
                .scalar_parameters
                .get(scalar_position)
                .ok_or(LoweringError::Unsupported(
                    "scalar graph lost a scalar parameter",
                ))?;
        if retained.source_position as usize != position
            || retained.primitive_type != primitive
            || graph.parameter_types.get(scalar_position) != Some(&primitive)
        {
            return unsupported("scalar graph scalar positions differ from its authored signature");
        }
        scalar_position += 1;
        if !parameter.is_mutable {
            continue;
        }
        let row = storage.next().ok_or(LoweringError::Unsupported(
            "mutable scalar parameter has no retained entry storage",
        ))?;
        if !supported_mutable_parameter(primitive)
            || usize::try_from(row.parameter_ordinal).ok() != Some(position)
            || row.symbol != parameter.symbol
            || row.primitive_type != primitive
        {
            return unsupported(
                "scalar parameter storage disagrees with its authored mutable binding",
            );
        }
    }
    if storage.next().is_some()
        || structural.next().is_some()
        || scalar_position != graph.parameter_types.len()
    {
        return unsupported("scalar parameter storage contains an unauthored entry binding");
    }
    owned_types::validate(checked, state, &graph.structural_parameters)?;
    Ok(rows)
}

fn validate_structural_parameter(
    checked: &CheckedTrees,
    parameter: &checked_trees::signature::StateParameter,
    position: usize,
    retained: &checked_trees::CheckedUnitStructuralParameterPlan,
) -> Result<(), LoweringError> {
    use checked_trees::types::TypeReferenceNode;
    if matches!(
        checked
            .type_reference_table
            .type_reference(parameter.type_reference),
        TypeReferenceNode::Named { .. }
    ) {
        if parameter.is_mutable
            || !validation::has_plain_owned_contents_with_numeric_constraints(
                checked,
                parameter.type_reference,
            )
            || retained.is_self
            || retained.position as usize != position
            || retained.access != checked_trees::CheckedStructuralAccess::Owned
            || !matches!(
                retained.multiplicity,
                Multiplicity::Unrestricted | Multiplicity::Affine
            )
            || checked.type_multiplicity(parameter.type_reference) != retained.multiplicity
            || !retained.qualifications.is_empty()
            || retained.fused_service_erasure.is_some()
            || retained.type_identity
                != checked
                    .normalized_type_identity(parameter.type_reference)
                    .into_string()
        {
            return unsupported("scalar graph owned parameter differs from its authored signature");
        }
        return Ok(());
    }
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = checked
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return unsupported("scalar graph structural parameter requires a primitive reference");
    };
    let access = match access {
        language_semantics::ReferenceAccess::Shared => {
            checked_trees::CheckedStructuralAccess::SharedBorrow
        }
        language_semantics::ReferenceAccess::Mutable => {
            checked_trees::CheckedStructuralAccess::MutableBorrow
        }
        language_semantics::ReferenceAccess::WriteOnly => {
            checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
        }
    };
    if retained.is_self
        || retained.position as usize != position
        || retained.access != access
        || retained.multiplicity != Multiplicity::Unrestricted
        || !retained.qualifications.is_empty()
        || retained.fused_service_erasure.is_some()
        || !matches!(
            checked.type_reference_table.type_reference(*referee),
            TypeReferenceNode::Named { .. }
        )
        || checked.primitive_type_reference(*referee).is_none()
        || retained.type_identity
            != checked
                .normalized_type_identity_with_binders(*referee, &[])
                .into_string()
    {
        return unsupported("scalar graph primitive reference differs from its authored signature");
    }
    let mut shapes = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .structural_types
        .iter()
        .filter(|shape| shape.identity == retained.type_identity);
    let shape = shapes.next().ok_or(LoweringError::Unsupported(
        "scalar graph primitive referent shape is absent",
    ))?;
    if shapes.next().is_some()
        || shape.shape
            != checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(
                checked
                    .primitive_type_reference(*referee)
                    .ok_or(LoweringError::Unsupported(
                        "scalar graph primitive referent is absent",
                    ))?,
            )
    {
        return unsupported("scalar graph primitive referent shape differs from its source");
    }
    Ok(())
}
