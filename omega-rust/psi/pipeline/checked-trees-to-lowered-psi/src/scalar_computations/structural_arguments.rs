//! Bind source-checked structural operands to the enclosing machine's places.

use super::*;

pub(super) fn lower(
    checked: &CheckedTrees,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    arguments: &[checked_trees::CheckedScalarComputationStructuralArgument],
    bindings: &storage::ScalarBindings,
    arrays: &[arrays::Slot],
) -> Result<Vec<StructuralArgument>, LoweringError> {
    if arguments.is_empty() {
        return Ok(Vec::new());
    }
    let target = crate::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
        checked,
        target_machine,
    )?;
    let (_, state) = crate::scalar_source_custody::authored_state(checked, target_state)?;
    if target.entry_state()? != target_state
        || target.structural_parameters().len() != arguments.len()
        || !target.entry_claims().is_empty()
    {
        return unsupported("computed borrow disagrees with its selected callee signature");
    }
    arguments
        .iter()
        .zip(target.structural_parameters())
        .map(|(argument, parameter)| {
            let argument = match argument {
                checked_trees::CheckedScalarComputationStructuralArgument::Case(_) => {
                    return unsupported("computed case call arguments are not admitted");
                }
                checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) => {
                    argument
                }
                checked_trees::CheckedScalarComputationStructuralArgument::Array {
                    expression,
                    type_reference,
                    elements,
                } => {
                    let slot = arrays
                        .iter()
                        .find(|slot| slot.expression == *expression)
                        .ok_or(LoweringError::Unsupported(
                            "computed array has no reserved structural result",
                        ))?;
                    let source_parameter = checked
                        .state_parameters(state)
                        .get(parameter.position as usize)
                        .ok_or(LoweringError::Unsupported(
                            "computed array formal position is absent",
                        ))?;
                    let leaves = checked
                        .facts
                        .values
                        .scalar_computations
                        .operands
                        .span(*elements)
                        .ok_or(LoweringError::Unsupported(
                            "computed array elements have a stale span",
                        ))?;
                    if parameter.access != checked_trees::CheckedStructuralAccess::Owned
                        || parameter.multiplicity != Multiplicity::Unrestricted
                        || !parameter.qualifications.is_empty()
                        || parameter.fused_service_erasure.is_some()
                        || source_parameter.is_mutable
                        || checked.normalized_type_identity(*type_reference).as_str()
                            != parameter.type_identity
                        || checked
                            .normalized_type_identity(source_parameter.type_reference)
                            .as_str()
                            != parameter.type_identity
                        || u64::try_from(leaves.len()).ok() != Some(slot.leaf_count)
                    {
                        return unsupported("computed array differs from its exact owned formal");
                    }
                    for leaf in leaves {
                        let nodes = &checked.facts.values.scalar_computations.nodes;
                        if !nodes.is_valid(*leaf)
                            || terminal_scalar_type(nodes.get(*leaf).primitive_type)?
                                != slot.leaf_type
                        {
                            return unsupported(
                                "computed array leaf carrier differs from its structural type",
                            );
                        }
                    }
                    return Ok(StructuralArgument {
                        place: slot.place,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    });
                }
            };
            if argument.type_identity != parameter.type_identity
                || argument.access != parameter.access
                || !matches!(
                    (parameter.access, parameter.multiplicity),
                    (_, Multiplicity::Unrestricted)
                        | (
                            checked_trees::CheckedStructuralAccess::Owned,
                            Multiplicity::Affine
                        )
                )
                || !parameter.qualifications.is_empty()
                || parameter.fused_service_erasure.is_some()
            {
                return unsupported("computed borrow disagrees with its callee referent custody");
            }
            let source_parameter = checked
                .state_parameters(state)
                .get(parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "computed borrow callee position is absent",
                ))?;
            if parameter.access == checked_trees::CheckedStructuralAccess::Owned {
                let primitive_array = validation::is_closed_primitive_array_type(
                    checked,
                    source_parameter.type_reference,
                );
                if source_parameter.is_mutable
                    || (!primitive_array
                        && !matches!(
                            checked
                                .type_reference_table
                                .type_reference(source_parameter.type_reference),
                            checked_trees::types::TypeReferenceNode::Named { .. }
                        ))
                    || (primitive_array && parameter.multiplicity != Multiplicity::Unrestricted)
                    || checked.type_multiplicity(source_parameter.type_reference)
                        != parameter.multiplicity
                    || checked
                        .normalized_type_identity(source_parameter.type_reference)
                        .into_string()
                        != parameter.type_identity
                {
                    return unsupported("computed owned operand differs from its callee signature");
                }
                return bindings.owned_argument(argument);
            }
            let checked_trees::types::TypeReferenceNode::Reference { referee, .. } = checked
                .type_reference_table
                .type_reference(source_parameter.type_reference)
            else {
                return unsupported("computed structural operand is not a primitive reference");
            };
            if !matches!(
                checked.type_reference_table.type_reference(*referee),
                checked_trees::types::TypeReferenceNode::Named { .. }
            ) {
                return unsupported("computed primitive borrow requires an unqualified referent");
            }
            let primitive =
                checked
                    .primitive_type_reference(*referee)
                    .ok_or(LoweringError::Unsupported(
                        "computed borrow referent is not primitive",
                    ))?;
            bindings.primitive_borrow(argument, terminal_scalar_type(primitive)?)
        })
        .collect()
}
