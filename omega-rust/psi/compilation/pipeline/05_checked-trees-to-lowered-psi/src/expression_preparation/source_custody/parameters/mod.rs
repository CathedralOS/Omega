//! Rejoin mutable entry storage to the exact current-state parameter frontier.

use super::super::Multiplicity;
use super::{
    CheckedTrees, LoweringError, authored_state, supported_mutable_parameter, unsupported,
};
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
                    let checked_trees::types::TypeConstraintNode::Domain(domain) = constraint
                    else {
                        return unsupported(
                            "Unit graph borrowed parameter has a non-domain constraint",
                        );
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

#[cfg(test)]
mod tests;

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
    // An `[erased]` binding occurrence owns neither a scalar nor a structural
    // entry; the retained arity is reconstructed from the typed relevance. A
    // borrowed `self` stays ambient on the attachment carrier, but when the
    // entry roster retained it as the machine's receiver it counts here too.
    let ambient_self = graph
        .structural_parameters
        .iter()
        .any(|parameter| parameter.is_self);
    let retained_parameters = parameters
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased() && (ambient_self || !parameter.is_self)
        })
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
        if parameter.is_self {
            // The receiver rejoins only as a borrowed reference. When the
            // entry roster retained it as the machine's ambient operand, it
            // owns the matching structural entry exactly here.
            let checked_trees::types::TypeReferenceNode::Reference { access, .. } = checked
                .type_reference_table
                .type_reference(parameter.type_reference)
            else {
                return unsupported(
                    "scalar parameter storage disagrees with its authored signature",
                );
            };
            if ambient_self {
                let access = match access {
                    language_semantics::ReferenceAccess::Shared => {
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                    }
                    language_semantics::ReferenceAccess::Mutable => {
                        checked_trees::CheckedStructuralAccess::MutableBorrow
                    }
                    language_semantics::ReferenceAccess::WriteOnly => {
                        return unsupported("scalar graph retained receiver must be readable");
                    }
                };
                let retained = structural.next().ok_or(LoweringError::Unsupported(
                    "scalar graph lost its ambient receiver",
                ))?;
                if !retained.is_self
                    || retained.position as usize != position
                    || retained.access != access
                    || retained.multiplicity != Multiplicity::Unrestricted
                {
                    return unsupported(
                        "scalar parameter storage disagrees with its authored signature",
                    );
                }
            }
            continue;
        }
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
        if parameter.is_mutable
            || (!primitive_array
                && !validation::has_plain_owned_contents_with_numeric_constraints(
                    checked,
                    parameter.type_reference,
                ))
            || (primitive_array && retained.multiplicity != Multiplicity::Unrestricted)
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
    // Reconstruct the formal's view presentation from source, independently
    // of the retained catalog. Slices peel their reference identity; a fixed
    // byte view retains it and records the declared extent in its carrier.
    let view = match referee_node {
        TypeReferenceNode::Slice { element_type } => {
            let byte_element = checked.primitive_type_reference(*element_type)
                == Some(checked_trees::types::PrimitiveType::U8);
            let shape = if byte_element {
                checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                    checked_trees::CheckedByteSequenceCarrier::BorrowedView { access: None },
                )
            } else {
                checked_trees::CheckedUnitStructuralTypeShape::BorrowedSliceView {
                    element_type_identity: checked
                        .normalized_type_identity(*element_type)
                        .into_string(),
                }
            };
            Some((*referee, shape, (!byte_element).then_some(*element_type)))
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: checked_trees::types::FixedArrayLength::Literal(length),
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
            Some((
                parameter.type_reference,
                checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                    checked_trees::CheckedByteSequenceCarrier::BoundedView {
                        access: None,
                        capacity: u64::try_from(*length).map_err(|_| {
                            LoweringError::Unsupported("scalar graph fixed view extent exceeds u64")
                        })?,
                    },
                ),
                None,
            ))
        }
        _ => None,
    };
    if let Some((presentation, expected_shape, element)) = view {
        if retained.is_self
            || retained.position as usize != position
            || retained.access != access
            || retained.multiplicity != Multiplicity::Unrestricted
            || !retained.qualifications.is_empty()
            || retained.fused_service_erasure.is_some()
            || retained.type_identity
                != checked.normalized_type_identity(presentation).into_string()
        {
            return unsupported("scalar graph view parameter differs from its authored signature");
        }
        let mut shapes = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter()
            .filter(|shape| shape.identity == retained.type_identity);
        let shape = shapes.next().ok_or(LoweringError::Unsupported(
            "scalar graph view shape is absent",
        ))?;
        // The catalog keeps an element view's carrier identity, which peels an
        // arithmetic policy the slice's own identity still spells
        // (`&[i32 in Wrapping]` retains `i32`), and a byte view's carrier
        // fixes its element whatever access the catalog records. Every other
        // presentation must match the source exactly.
        let retained_element = match (&shape.shape, &expected_shape) {
            (
                checked_trees::CheckedUnitStructuralTypeShape::BorrowedSliceView {
                    element_type_identity,
                },
                checked_trees::CheckedUnitStructuralTypeShape::BorrowedSliceView { .. },
            ) => Some(element_type_identity.as_str()),
            (
                checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                    checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. },
                ),
                checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                    checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. },
                ),
            ) => None,
            (retained, expected) if retained == expected => None,
            _ => return unsupported("scalar graph view shape differs from its source"),
        };
        if shapes.next().is_some() {
            return unsupported("scalar graph view shape differs from its source");
        }
        // Views retain the same element declaration as owned storage. Rebuild
        // its complete shape from source, including record fields and their
        // numeric restrictions; checking only primitive leaves excludes record
        // views, while trusting the retained identity alone permits substitution.
        if let Some(element) = element {
            owned_types::validate_referent_type(
                checked,
                element,
                retained_element.ok_or(LoweringError::Unsupported(
                    "scalar graph element view lost its element identity",
                ))?,
            )?;
        }
        return Ok(());
    }
    if retained.is_self
        || retained.position as usize != position
        || retained.access != access
        || retained.multiplicity != Multiplicity::Unrestricted
        || !retained.qualifications.is_empty()
        || retained.fused_service_erasure.is_some()
        || !matches!(
            referee_node,
            TypeReferenceNode::Named { .. } | TypeReferenceNode::FixedArray { .. }
        )
        || retained.type_identity != checked.normalized_type_identity(*referee).into_string()
    {
        return unsupported("scalar graph reference differs from its authored signature");
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
        owned_types::validate_referent_type(checked, *referee, &retained.type_identity)?;
        if !checked
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter()
            .any(|shape| {
                shape.identity == retained.type_identity
                    && matches!(
                        shape.shape,
                        checked_trees::CheckedUnitStructuralTypeShape::Record { .. }
                            | checked_trees::CheckedUnitStructuralTypeShape::FixedArray { .. }
                    )
            })
        {
            return unsupported("scalar graph borrowed aggregate differs from its source shape");
        }
        return Ok(());
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
