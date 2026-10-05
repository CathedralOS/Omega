//! Rejoin mutable entry storage to the exact current-state parameter frontier.

use super::{CheckedTrees, LoweringError, supported_mutable_parameter, unsupported};
mod owned;
mod owned_types;

/// Reconstruct a borrowed signature's exact qualification set on both sides of
/// its reference shell. Peeling the carrier never drops its domain obligations:
/// admission compares this set before the existing type/access checks, and
/// signature lowering publishes those same domains on the invocation place.
pub(crate) fn borrowed_parameter_custody(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> Result<
    (
        checked_trees::types::TypeReferenceHandle,
        language_core::ReferenceAccess,
        Vec<language_semantics::SemanticDomainId>,
    ),
    LoweringError,
> {
    let mut visited = Vec::new();
    let mut access = None;
    let mut qualifications = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return unsupported("Unit graph borrowed parameter has an invalid type chain");
        }
        visited.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            checked_trees::types::TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let retained = checked.type_reference_table.constraints(*constraints);
                if retained.len() != constraints.len() {
                    return unsupported("Unit graph borrowed parameter has stale qualifications");
                }
                for constraint in retained {
                    let domain = match constraint {
                        // Arithmetic policies govern checked operations, not
                        // semantic-domain membership of stored values. The
                        // producer's signature partition makes the same
                        // distinction; accompanying domains still travel below.
                        checked_trees::types::TypeConstraintNode::ArithmeticDomain(_) => continue,
                        checked_trees::types::TypeConstraintNode::Domain(domain) => domain,
                        _ => {
                            return unsupported(
                                "Unit graph borrowed parameter has an unsupported constraint",
                            );
                        }
                    };
                    if !domain.semantic_id.is_valid() {
                        return unsupported("Unit graph borrowed parameter has an absent domain");
                    }
                    qualifications.push(domain.semantic_id);
                }
                reference = *base_type;
            }
            checked_trees::types::TypeReferenceNode::Reference {
                referee,
                access: mode,
                ..
            } if access.is_none() => {
                access = Some(*mode);
                reference = *referee;
            }
            _ => break,
        }
    }
    let access = access.ok_or(LoweringError::Unsupported(
        "Unit graph structural parameter is not a borrowed view",
    ))?;
    qualifications.sort_by_key(|domain| domain.0);
    qualifications.dedup();
    Ok((reference, access, qualifications))
}

pub(crate) fn parameter_storage<'checked>(
    checked: &'checked CheckedTrees,
    machine: symbols::SymbolHandle,
    graph: &checked_trees::CheckedScalarStateGraph,
) -> Result<&'checked [checked_trees::CheckedScalarParameterStorage], LoweringError> {
    let (owner, state) = crate::declarations::state_declaration(checked, graph.state)?;
    if owner.symbol != machine {
        return unsupported("scalar parameter storage belongs to another machine");
    }
    owned::validate(checked, machine, state, &graph.structural_parameters)?;
    let parameters = checked.state_parameters(state);
    // Erased bindings own neither scalar nor structural entry storage.
    let retained_parameters = parameters
        .iter()
        .filter(|parameter| !parameter.relevance.is_erased())
        .count();
    if retained_parameters != graph.parameter_types.len() + graph.structural_parameters.len()
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
            || parameter.is_const
        {
            return unsupported("scalar parameter storage disagrees with its authored signature");
        }
        if parameter.relevance.is_erased() {
            if parameter.is_mutable {
                return unsupported("erased scalar parameter cannot own mutable entry storage");
            }
            continue;
        }
        let Some(primitive) = checked.primitive_type_reference(parameter.type_reference) else {
            let retained = structural.next().ok_or(LoweringError::Unsupported(
                "scalar graph lost a structural parameter",
            ))?;
            validate_structural_parameter(checked, parameter, retained)?;
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
    retained: &checked_trees::CheckedUnitStructuralParameterPlan,
) -> Result<(), LoweringError> {
    use checked_trees::types::TypeReferenceNode;
    let primitive_array =
        validation::is_closed_primitive_array_type(checked, parameter.type_reference);
    if primitive_array
        || matches!(
            checked
                .type_reference_table
                .type_reference(parameter.type_reference),
            TypeReferenceNode::Named { .. }
        )
    {
        return Ok(());
    }
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = checked
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return unsupported("scalar graph structural parameter requires an admitted reference");
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
    let referee_node = checked.type_reference_table.type_reference(*referee);
    // A slice view's element type, when it is not a byte view; a fixed byte
    // view has no separate element declaration.
    let view = match referee_node {
        TypeReferenceNode::Slice { element_type } => {
            let byte_element = checked.primitive_type_reference(*element_type)
                == Some(checked_trees::types::PrimitiveType::U8);
            Some((!byte_element).then_some(*element_type))
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: checked_trees::types::FixedArrayLength::Literal(_),
        } if checked.primitive_type_reference(*element_type)
            == Some(checked_trees::types::PrimitiveType::U8)
            && matches!(
                access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
            ) =>
        {
            if !retained.projected_qualifications.is_empty() {
                return unsupported("scalar graph fixed view has unsupported source custody");
            }
            Some(None)
        }
        _ => None,
    };
    if let Some(element) = view {
        let shape = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter()
            .find(|shape| shape.identity == retained.type_identity)
            .ok_or(LoweringError::Unsupported(
                "scalar graph view shape is absent",
            ))?;
        // Views retain the same element declaration as owned storage. Rebuild
        // its complete shape from source, including record fields and their
        // numeric restrictions.
        if let Some(element) = element {
            if !matches!(
                shape.shape,
                checked_trees::CheckedUnitStructuralTypeShape::BorrowedSliceView { .. }
            ) {
                return unsupported("scalar graph element view lost its element identity");
            }
            owned_types::validate_referent_type(checked, element)?;
        }
        return Ok(());
    }
    if checked.primitive_type_reference(*referee).is_none() {
        if !matches!(
            access,
            checked_trees::CheckedStructuralAccess::SharedBorrow
                | checked_trees::CheckedStructuralAccess::MutableBorrow
        ) || !retained.projected_qualifications.is_empty()
        {
            return unsupported("scalar graph record reference has unsupported source custody");
        }
        // Borrowing changes access, not the declared record or fixed array.
        // Reconstruct fields, element types and lengths by the same source-type
        // replay used for slice elements before admitting its borrowed carrier.
        owned_types::validate_referent_type(checked, *referee)?;
        return Ok(());
    }
    if !checked
        .facts
        .flow
        .terminal_scalar_graphs
        .structural_types
        .iter()
        .any(|shape| shape.identity == retained.type_identity)
    {
        return unsupported("scalar graph primitive referent shape is absent");
    }
    Ok(())
}
