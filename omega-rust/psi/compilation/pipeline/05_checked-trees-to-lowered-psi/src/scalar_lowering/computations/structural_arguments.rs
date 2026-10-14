//! Bind source-checked structural operands to the enclosing machine's places.

use super::super::{Multiplicity, StructuralAccess, terminal_scalar_type};
use super::{
    CheckedTrees, LoweringError, literals, produced_values, storage, subslices, unsupported,
};
use crate::emission::operation_emission::calls::PreparedStructuralArgument;

pub(super) fn lower(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
    let (_, caller_authored) = crate::declarations::state_declaration(checked, caller_state)?;
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
                            Multiplicity::Affine | Multiplicity::Linear
                        )
                )
            {
                return unsupported("computed borrow disagrees with its callee referent custody");
            }
            if matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }) {
                    let slot = produced_values.iter().find(|slot| {
                        slot.source_call == source_call && slot.argument_ordinal == Some(argument_ordinal)
                    }).ok_or(LoweringError::Unsupported(
                        "computed value has no reserved argument home",
                    ))?;
                    return Ok(PreparedStructuralArgument {
                        selector_positions: Vec::new(),
                        place: slot.place,
                        root_type: Some(slot.structural_type),
                        path: crate::expression_preparation::bindings::structural_paths::prepare_call_argument_path(identities, checked, argument, slot.structural_type, structural_types)?,
                        access: match argument.access {
                            checked_trees::CheckedStructuralAccess::Owned => StructuralAccess::Owned,
                            access => borrowed_access(access).ok_or(
                                LoweringError::Unsupported("computed owner requires transfer or lending"),
                            )?,
                        },
                    });
            }
            if let Some(bytes) = literals::source_bytes(argument) {
                let slot = literals.iter().find(|slot| {
                    slot.source_call == source_call
                        && slot.argument_ordinal == argument_ordinal
                        && slot.literal.bytes.as_slice() == bytes
                }).ok_or(LoweringError::Unsupported("computed literal has no reserved place"))?;
                if !argument.path.is_empty() {
                    return unsupported("computed literal cannot project its materialization");
                }
                return Ok(PreparedStructuralArgument {
                    selector_positions: Vec::new(),
                    place: slot.literal.place,
                    root_type: Some(slot.literal.structural_type),
                    path: Vec::new(),
                    access: match argument.access {
                        checked_trees::CheckedStructuralAccess::Owned => StructuralAccess::Owned,
                        checked_trees::CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
                        _ => return unsupported("computed literal admits ownership or shared lending"),
                    },
                });
            }
            if parameter.access == checked_trees::CheckedStructuralAccess::Owned {
                let source = bindings.owned_argument_home(argument)?;
                let selected_root = bindings.structural_argument_type(
            &source).ok_or(
                    LoweringError::Unsupported(
            "computed owned argument lost its published carrier"),
                )?;
                let mut prepared = PreparedStructuralArgument::from_static(
            source.clone(
            ), bindings.structural_place_type(
            source.place))?;
                prepared.path.extend(crate::expression_preparation::bindings::structural_paths::prepare_argument_path(identities, &argument.path, selected_root, structural_types)?);
                return Ok(prepared);
            }
            // The callee's plan row names the primitive a borrowed formal
            // lends; any other referent binds a structural place.
            let Some(primitive) = parameter.primitive_referent else {
                if let Some(range) = subslices::range(argument) {
                    let access = borrowed_access(argument.access).ok_or(
                        LoweringError::Unsupported(
                            "computed range requires borrowed access",
                        ),
                    )?;
                    let slot = subslices
                        .iter()
                        .find(|slot| slot.expression == range.expression)
                        .ok_or(LoweringError::Unsupported(
                            "computed range has no reserved structural result",
                        ))?;
                    return Ok(PreparedStructuralArgument {
                        selector_positions: Vec::new(),
                        place: slot.place,
                        root_type: Some(slot.structural_type),
                        path: Vec::new(),
                        access,
                    });
                }
                return bindings.prepared_borrowed_argument(
                    identities, checked, argument, caller_parameters, structural_types,
                );
            };
            bindings
                .prepared_primitive_borrow(identities, checked, argument, terminal_scalar_type(primitive)?, structural_types, caller_parameters)
        })
        .collect()
}

fn borrowed_access(access: checked_trees::CheckedStructuralAccess) -> Option<StructuralAccess> {
    match access {
        checked_trees::CheckedStructuralAccess::SharedBorrow => {
            Some(StructuralAccess::SharedBorrow)
        }
        checked_trees::CheckedStructuralAccess::MutableBorrow => {
            Some(StructuralAccess::MutableBorrow)
        }
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => {
            Some(StructuralAccess::WriteOnlyBorrow)
        }
        checked_trees::CheckedStructuralAccess::Owned => None,
    }
}
