//! Bind source-checked structural operands to the enclosing machine's places.

use super::super::{Multiplicity, StructuralAccess, terminal_scalar_type};
use super::{
    CheckedTrees, LoweringError, literals, produced_values, storage, subslices, unsupported,
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
    produced_values: &[produced_values::Slot],
    subslices: &[subslices::Slot],
    literals: &[literals::Slot],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<PreparedStructuralArgument>, LoweringError> {
    if arguments.is_empty() {
        return Ok(Vec::new());
    }
    let parameters =
        super::call_signature::structural_parameters(checked, target_machine, target_state)?;
    let (caller_machine, caller_authored) =
        crate::declarations::state_declaration(checked, caller_state)?;
    let caller_parameters = checked.state_parameters(caller_authored);
    if parameters.len() != arguments.len() {
        return unsupported("computed borrow disagrees with its selected callee signature");
    }
    arguments
        .iter()
        .zip(parameters)
        .enumerate()
        .map(|(argument_ordinal, (argument, parameter))| {
            let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) = argument else {
                return unsupported("computed constructor did not retain its structural value");
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
            if matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }) {
                    let slot = produced_values.iter().find(|slot| {
                        slot.source_call == source_call && slot.argument_ordinal == argument_ordinal
                    }).ok_or(LoweringError::Unsupported(
                        "computed value has no reserved argument home",
                    ))?;
                    return Ok(PreparedStructuralArgument {
                        selector_positions: Vec::new(),
                        place: slot.place,
                        path: crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                            &argument.path, slot.structural_type, structural_types,
                        )?,
                        access: match argument.access {
                            checked_trees::CheckedStructuralAccess::Owned => StructuralAccess::Owned,
                            checked_trees::CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
                            _ => return unsupported("computed owner only admits transfer or shared lending"),
                        },
                    });
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
                let source = bindings.borrowed_structural_home(argument, caller_machine, caller_parameters)?;
                let root_type = bindings.structural_argument_type(&source).ok_or(
                    LoweringError::Unsupported("structural argument lost its exact published carrier"),
                )?;
                let mut path = source.path.into_iter().map(
                    crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static,
                ).collect::<Vec<_>>();
                path.extend(crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                    &argument.path, root_type, structural_types,
                )?);
                return Ok(PreparedStructuralArgument {
                    selector_positions: Vec::new(),
                    place: source.place,
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
                .prepared_primitive_borrow(argument, terminal_scalar_type(primitive)?, structural_types)
        })
        .collect()
}
