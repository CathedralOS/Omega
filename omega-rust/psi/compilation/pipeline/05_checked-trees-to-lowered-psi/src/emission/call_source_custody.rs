//! Source custody shared by ordinary and boundary calls, independent of result.

use super::{CheckedTrees, LoweringError, unsupported};
pub(crate) mod authored;
mod boundary_buffers;
pub(crate) mod literal_arguments;
pub(crate) mod occurrences;
pub(crate) mod projected_receivers;

/// A projected Fused binding selects an erased capability, not an implicit
/// runtime borrow. Its boundary signature has no self operand, so argument
/// access facts cannot stand in for the exact parameter and field custody.
/// Reports whether the call's receiver is such a parameter-rooted binding, so
/// attachment validation does not mistake an ordinary context for lexical self.
pub(crate) fn validate_parameter_binding_receiver(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    caller_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    call: &crate::call_sites::AuthoredCall,
) -> Result<bool, LoweringError> {
    use checked_trees::CheckedUnitStructuralTypeShape;
    use checked_trees::expression::ExpressionNode;
    use checked_trees::statement::StatementNode;
    use checked_trees::{CheckedUnitStructuralFieldType, CheckedUnitStructuralPathSegment};

    let Some(owner) = checked.traits().iter().find(|definition| {
        definition.is_boundary
            && checked
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == call.source_target)
    }) else {
        // Boundary machines and operators have no Binding requirement receiver.
        return Ok(false);
    };
    let (machine, state) = crate::declarations::state_declaration(checked, caller_state)?;
    if machine.symbol != caller_machine {
        return unsupported("binding receiver belongs to another authored machine");
    }
    let (root, path, carrier) = match call.source_site {
        Some(checked_trees::NominalMachineUseSite::Expression(expression)) => {
            let ExpressionNode::Call(source) = checked.expression_table.expression(expression)
            else {
                return unsupported("binding receiver lost its authored call");
            };
            if !source.receiver.is_valid()
                || !matches!(
                    checked.expression_table.expression(source.receiver),
                    ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
                )
            {
                return Ok(false);
            }
            let projected = projected_receivers::source(
                checked,
                caller_machine,
                caller_state,
                coordinate.statement_index as usize,
                source.receiver,
            )?;
            let carrier = validation::declared_place_type_raw(
                &checked.typed,
                machine,
                Some(state),
                source.receiver,
            )
            .ok_or(LoweringError::Unsupported(
                "binding receiver has no exact declared type",
            ))?;
            (projected.root, projected.path, carrier)
        }
        Some(checked_trees::NominalMachineUseSite::Statement(_)) => {
            let Some(statement @ StatementNode::Call(source)) = checked
                .statement_table
                .statements(state.statement_nodes)
                .get(coordinate.statement_index as usize)
            else {
                return unsupported("binding receiver lost its authored statement");
            };
            let members = checked.statement_table.name_path_members(source.receiver);
            if !checked.state_parameters(state).iter().any(|parameter| {
                parameter.symbol == source.receiver_root_symbol
                    || members.first() == Some(&parameter.name)
            }) || members.len() < 2
            {
                return Ok(false);
            }
            let (root, path) =
                crate::unit::attached_unit::statement_receiver_path(checked, state, statement)?;
            // The shared path validator has checked the endpoint against its
            // exact declared owner. Recover that field's type, not its spelling.
            let mut fields = checked
                .data_definitions()
                .iter()
                .flat_map(|definition| checked.data_members(definition))
                .filter_map(|member| match member {
                    checked_trees::data::DataMember::Field(field)
                        if field.symbol == source.receiver_symbol =>
                    {
                        Some(field)
                    }
                    _ => None,
                });
            let field = fields.next().ok_or(LoweringError::Unsupported(
                "binding receiver endpoint has no declared field",
            ))?;
            if fields.next().is_some() {
                return unsupported("binding receiver endpoint has multiple declarations");
            }
            (root, path, field.type_reference)
        }
        _ => return Ok(false),
    };
    let Some((position, source_parameter)) = checked
        .state_parameters(state)
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == root)
    else {
        // Attached self and local receiver routes retain their existing owners.
        return Ok(false);
    };
    if path.is_empty() {
        return Ok(false);
    }
    let mut parameters = caller_parameters
        .iter()
        .filter(|parameter| parameter.position as usize == position);
    let parameter = parameters.next().ok_or(LoweringError::Unsupported(
        "binding receiver lost its retained structural parameter",
    ))?;
    if parameters.next().is_some()
        || source_parameter.relevance.is_erased()
        || parameter.fused_service_erasure.is_some()
    {
        return unsupported("binding receiver has inconsistent ordinary parameter custody");
    }
    checked
        .typed
        .unwrapped_type_reference(source_parameter.type_reference)
        .ok_or(LoweringError::Unsupported(
            "binding receiver parameter has no declared carrier",
        ))?;
    let requirement =
        typed_trees::service::exact_bound_service_requirement(&checked.typed, carrier).ok_or(
            LoweringError::Unsupported("parameter receiver is not an exact Binding carrier"),
        )?;
    if owner.symbol != requirement || checked.fused_service_erasure(requirement).is_none() {
        return unsupported("binding receiver lacks its exact Fused requirement authorization");
    }
    let mut identity = parameter.type_identity.as_str();
    let mut endpoint = None;
    for (position, segment) in path.iter().enumerate() {
        let shape = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .find(|shape| shape.identity == identity)
            .ok_or(LoweringError::Unsupported(
                "binding receiver lost its retained carrier shape",
            ))?;
        match (segment, &shape.shape) {
            (
                CheckedUnitStructuralPathSegment::Field(name),
                CheckedUnitStructuralTypeShape::Record { fields },
            ) => {
                let mut fields = fields.iter().filter(|field| field.identity == *name);
                let field = fields.next().ok_or(LoweringError::Unsupported(
                    "binding receiver lost its retained field",
                ))?;
                if fields.next().is_some() || field.relevance.is_erased() {
                    return unsupported("binding receiver has inconsistent retained field custody");
                }
                endpoint = Some(&field.field_type);
                if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                    &field.field_type
                {
                    identity = type_identity;
                } else if position + 1 != path.len() {
                    return unsupported("binding receiver projects through a nonstructural field");
                }
            }
            (
                CheckedUnitStructuralPathSegment::FixedIndex(index),
                CheckedUnitStructuralTypeShape::FixedArray {
                    element_type_identity,
                    length,
                },
            ) if *index < *length => {
                identity = element_type_identity;
                endpoint = None;
            }
            _ => {
                return unsupported(
                    "binding receiver projection disagrees with its retained carrier",
                );
            }
        }
    }
    if !matches!(endpoint, Some(CheckedUnitStructuralFieldType::FusedServiceBacked {
        provider_type_identity, erasure,
    }) if erasure.requirement == requirement
        && provider_type_identity == checked.normalized_type_identity(carrier).as_str())
    {
        return unsupported("binding receiver lost its exact Fused field erasure receipt");
    }
    Ok(true)
}
