//! Bind source-checked structural operands to the enclosing machine's places.

use super::super::{Multiplicity, StructuralAccess, terminal_scalar_type};
use super::{
    CheckedTrees, LoweringError, arrays, cases, literals, storage, subslices, unsupported,
};
use crate::emission::operation_emission::calls::PreparedStructuralArgument;

pub(super) fn lower(
    checked: &CheckedTrees,
    source_call: arena::Handle<checked_trees::FlowCallFact>,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    arguments: &[checked_trees::CheckedScalarComputationStructuralArgument],
    bindings: &storage::ScalarBindings,
    arrays: &[arrays::Slot],
    cases: &[cases::Slot],
    subslices: &[subslices::Slot],
    literals: &[literals::Slot],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<PreparedStructuralArgument>, LoweringError> {
    if arguments.is_empty() {
        return Ok(Vec::new());
    }
    let target =
        crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
            checked,
            target_machine,
        )?;
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, target_state)?;
    let (caller_machine, caller_authored) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    let caller_parameters = checked.state_parameters(caller_authored);
    if target.entry_state()? != target_state
        || target.structural_parameters().len() != arguments.len()
        || !target.entry_claims().is_empty()
    {
        return unsupported("computed borrow disagrees with its selected callee signature");
    }
    arguments
        .iter()
        .zip(target.structural_parameters())
        .enumerate()
        .map(|(argument_ordinal, (argument, parameter))| {
            let argument = match argument {
                checked_trees::CheckedScalarComputationStructuralArgument::Case(subject) => {
                    let slot = cases
                        .iter()
                        .find(|slot| slot.expression == subject.expression)
                        .ok_or(LoweringError::Unsupported(
                            "computed case has no reserved structural result",
                        ))?;
                    let source_parameter = checked
                        .state_parameters(state)
                        .get(parameter.position as usize)
                        .ok_or(LoweringError::Unsupported(
                            "computed case formal position is absent",
                        ))?;
                    if parameter.access != checked_trees::CheckedStructuralAccess::Owned
                        || !matches!(
                            parameter.multiplicity,
                            Multiplicity::Affine | Multiplicity::Unrestricted
                        )
                        || !parameter.qualifications.is_empty()
                        || parameter.fused_service_erasure.is_some()
                        || source_parameter.is_mutable
                        || checked.type_multiplicity(subject.type_reference)
                            != parameter.multiplicity
                        || checked
                            .normalized_type_identity(subject.type_reference)
                            .as_str()
                            != parameter.type_identity
                        || checked
                            .normalized_type_identity(source_parameter.type_reference)
                            .as_str()
                            != parameter.type_identity
                    {
                        return unsupported("computed case differs from its exact owned formal");
                    }
                    return Ok(PreparedStructuralArgument {
                        selector_positions: Vec::new(),
                        place: slot.place,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    });
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
                    return Ok(PreparedStructuralArgument {
                        selector_positions: Vec::new(),
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
                if !parameter.qualifications.is_empty()
                    || source_parameter.is_mutable
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
                return bindings
                    .owned_argument(argument)
                    .and_then(PreparedStructuralArgument::from_static);
            }
            let (referee, access, qualifications) =
                crate::expression_preparation::source_custody::borrowed_parameter_custody(
                    checked,
                    source_parameter.type_reference,
                )?;
            let access = match access {
                language_core::ReferenceAccess::Shared => checked_trees::CheckedStructuralAccess::SharedBorrow,
                language_core::ReferenceAccess::Mutable => checked_trees::CheckedStructuralAccess::MutableBorrow,
                language_core::ReferenceAccess::WriteOnly => checked_trees::CheckedStructuralAccess::WriteOnlyBorrow,
            };
            if parameter.access != access
                || parameter.qualifications != qualifications
            {
                return unsupported("computed borrow changed its authored access or qualifications");
            }
            if checked.primitive_type_reference(referee).is_none() {
                if let Some(range) = subslices::range(argument) {
                    let access = match argument.access {
                        checked_trees::CheckedStructuralAccess::SharedBorrow => {
                            StructuralAccess::SharedBorrow
                        }
                        checked_trees::CheckedStructuralAccess::MutableBorrow => {
                            StructuralAccess::MutableBorrow
                        }
                        _ => return unsupported("computed range requires readable borrowed access"),
                    };
                    let slot = subslices
                        .iter()
                        .find(|slot| slot.expression == range.expression)
                        .ok_or(LoweringError::Unsupported(
                            "computed range has no reserved structural result",
                        ))?;
                    return Ok(PreparedStructuralArgument {
                        selector_positions: Vec::new(),
                        place: slot.place,
                        path: Vec::new(),
                        access,
                    });
                }
                if parameter.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                    && let Some(bytes) = literals::source_bytes(argument)
                {
                    let slot = literals
                        .iter()
                        .find(|slot| {
                            slot.source_call == source_call
                                && slot.argument_ordinal == argument_ordinal
                                && slot.bytes.as_slice() == bytes
                        })
                        .ok_or(LoweringError::Unsupported(
                            "computed literal has no reserved place",
                        ))?;
                    if !argument.path.is_empty() {
                        return unsupported("computed literal cannot project its view");
                    }
                    return Ok(PreparedStructuralArgument {
                        selector_positions: Vec::new(),
                        place: slot.place,
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    });
                }
                let place = bindings.borrowed_structural_place(argument, caller_machine, caller_parameters)?;
                let root_type = bindings.structural_place_type(place).ok_or(
                    LoweringError::Unsupported("structural argument lost its exact published carrier"),
                )?;
                let path = crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                    &argument.path, root_type, structural_types,
                )?;
                return Ok(PreparedStructuralArgument {
                    selector_positions: Vec::new(),
                    place,
                    path,
                    access: match argument.access {
                        checked_trees::CheckedStructuralAccess::SharedBorrow => {
                            StructuralAccess::SharedBorrow
                        }
                        checked_trees::CheckedStructuralAccess::MutableBorrow => {
                            StructuralAccess::MutableBorrow
                        }
                        _ => {
                            return unsupported(
                                "computed argument requires readable borrowed access",
                            );
                        }
                    },
                });
            }
            if !matches!(
                checked.type_reference_table.type_reference(referee),
                checked_trees::types::TypeReferenceNode::Named { .. }
            ) {
                return unsupported("computed primitive borrow requires an unqualified referent");
            }
            let primitive =
                checked
                    .primitive_type_reference(referee)
                    .ok_or(LoweringError::Unsupported(
                        "computed borrow referent is not primitive",
                    ))?;
            bindings
                .primitive_borrow(argument, terminal_scalar_type(primitive)?, structural_types)
                .and_then(PreparedStructuralArgument::from_static)
        })
        .collect()
}
