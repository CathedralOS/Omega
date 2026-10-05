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
    let (_, state) = crate::declarations::state_declaration(checked, target_state)?;
    let (caller_machine, caller_authored) =
        crate::declarations::state_declaration(checked, caller_state)?;
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
                    if checked
                        .state_parameters(state)
                        .get(parameter.position as usize)
                        .is_none()
                    {
                        return unsupported("computed case formal position is absent");
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
                    elements,
                    ..
                } => {
                    let slot = arrays
                        .iter()
                        .find(|slot| slot.expression == *expression)
                        .ok_or(LoweringError::Unsupported(
                            "computed array has no reserved structural result",
                        ))?;
                    if checked
                        .state_parameters(state)
                        .get(parameter.position as usize)
                        .is_none()
                    {
                        return unsupported("computed array formal position is absent");
                    }
                    if checked
                        .facts
                        .values
                        .scalar_computations
                        .operands
                        .span(*elements)
                        .is_none()
                    {
                        return unsupported("computed array elements have a stale span");
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
            if parameter.access == checked_trees::CheckedStructuralAccess::Owned {
                return bindings
                    .owned_argument(argument)
                    .and_then(PreparedStructuralArgument::from_static);
            }
            // The callee's plan row names the primitive a borrowed formal
            // lends; any other referent binds a structural place.
            let Some(primitive) = parameter.primitive_referent else {
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
            };
            bindings
                .primitive_borrow(argument, terminal_scalar_type(primitive)?, structural_types)
                .and_then(PreparedStructuralArgument::from_static)
        })
        .collect()
}
