//! One machine's structural foundation: `validate_machine` checks its
//! attachment and parameters, then each block's parameters and places,
//! entry claims, operation results and the affine cleanups its terminators
//! declare.

use super::{
    has_structural_type, is_plain_affine_call_result, require_known_services,
    structural_place_type, validate_operation_foundation, validate_provider_attachment_foundation,
    validate_structural_parameters, validate_structural_path,
};
use crate::codec_error::{CodecError, malformed};
use crate::sections::semantic_module::structural_result_wire;
use semantic_vocabulary::StructuralPlaceKind;
use std::collections::BTreeSet;
use terminal_psi::Block;
use terminal_psi::{
    StructuralMultiplicity, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
};

/// One machine's structural foundation: its attachment, parameters and
/// reference sources, then each block's parameters, the places those
/// parameters and operation results occupy, its entry claims, every
/// operation's foundation, and the affine cleanups its terminators declare.
pub(super) fn validate_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    if machine
        .attachment
        .is_some_and(|attachment| !has_structural_type(module, attachment))
    {
        return malformed("machine has an unknown attachment type");
    }
    validate_structural_parameters(module, &machine.structural_parameters)?;
    structural_result_wire::validate_reference_sources(module, machine)?;
    for block in &machine.blocks {
        validate_block_parameters(module, machine, block)?;
    }
    validate_block_parameter_places(machine)?;
    validate_provider_attachment_foundation(module, machine)?;
    require_known_services(module, &machine.published_service_ceiling)?;
    validate_parameter_places(machine)?;
    validate_entry_claims(module, machine)?;
    for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
        validate_operation_foundation(module, machine, operation)?;
    }
    validate_operation_result_places(machine)?;
    for block in &machine.blocks {
        validate_nominal_affine_cleanup(machine, block)?;
        validate_partial_affine_discards(module, machine, block)?;
    }
    Ok(())
}

/// One block's structural parameters: known types, no parameters on the
/// entry block, dense positions without self, and a matching block-parameter
/// place for each. Owned aggregates share this wire shape recursively.
fn validate_block_parameters(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: &Block,
) -> Result<(), CodecError> {
    validate_structural_parameters(module, &block.structural_parameters)?;
    if block.id == machine.entry && !block.structural_parameters.is_empty() {
        return malformed("entry block cannot declare structural parameters");
    }
    for (position, parameter) in block.structural_parameters.iter().enumerate() {
        if parameter.position as usize != position {
            return malformed("structural block parameters require dense positions");
        }
        if !machine.structural_places.iter().any(|place| {
            place.id == parameter.place
                && place.kind
                    == StructuralPlaceKind::BlockParameter {
                        block: block.id,
                        position: parameter.position,
                    }
        }) {
            return malformed("structural block parameter place disagrees with its declaration");
        }
    }
    Ok(())
}

/// Every block-parameter place is declared by the block and position it
/// names.
fn validate_block_parameter_places(machine: &TerminalMachine) -> Result<(), CodecError> {
    for place in &machine.structural_places {
        if let StructuralPlaceKind::BlockParameter { block, position } = place.kind {
            let parameter = machine
                .blocks
                .iter()
                .find(|candidate| candidate.id == block)
                .and_then(|candidate| candidate.structural_parameters.get(position as usize));
            if parameter.is_none_or(|parameter| parameter.place != place.id) {
                return malformed("structural block place has no matching parameter");
            }
        }
    }
    Ok(())
}

/// Every structural parameter occupies a declared parameter place whose
/// kind matches its position and self-ness.
fn validate_parameter_places(machine: &TerminalMachine) -> Result<(), CodecError> {
    for parameter in &machine.structural_parameters {
        let Some(place) = machine
            .structural_places
            .iter()
            .find(|place| place.id == parameter.place)
        else {
            return malformed("structural parameter has no declared structural place");
        };
        if place.kind
            != (StructuralPlaceKind::Parameter {
                position: parameter.position,
            })
        {
            return malformed("structural parameter place kind disagrees with its signature");
        }
    }
    Ok(())
}

/// Every entry claim binds a restricted structural parameter along a valid
/// path.
fn validate_entry_claims(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    for claim in &machine.entry_claims {
        let Some(parameter) = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input)
        else {
            return malformed("entry claim is not bound to a structural parameter");
        };
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
            return malformed("entry claim cannot bind an unrestricted parameter");
        }
        validate_structural_path(module, parameter.structural_type, &claim.path)?;
    }
    Ok(())
}

/// Every operation-result place has exactly one producer whose structural
/// result names that place and type.
fn validate_operation_result_places(machine: &TerminalMachine) -> Result<(), CodecError> {
    for place in &machine.structural_places {
        let StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } = place.kind
        else {
            continue;
        };
        let mut producers = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.id == producer);
        let Some(operation) = producers.next() else {
            return malformed("structural operation-result place has no producer");
        };
        if producers.next().is_some() {
            return malformed("structural operation-result place has duplicate producers");
        }
        let Some(result) = operation.result.structural() else {
            return malformed(
                "structural operation-result place producer has no structural result",
            );
        };
        if result.place != place.id || result.structural_type != structural_type {
            return malformed("structural operation-result place disagrees with its producer");
        }
    }
    Ok(())
}

/// A nominal cleanup terminator covers every structural parameter in reverse
/// order, retaining each exact whole owner and its consumed claim roster.
/// The verifier separately authenticates owner designation and live custody.
fn validate_nominal_affine_cleanup(
    machine: &TerminalMachine,
    block: &Block,
) -> Result<(), CodecError> {
    if let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator {
        if !matches!(machine.result, TerminalMachineResult::Unit) {
            return malformed("nominal affine cleanup requires a Unit result");
        }
        if cleanups.is_empty() {
            return malformed("nominal affine cleanup list is empty");
        }
        if cleanups.len() != machine.structural_parameters.len() {
            return malformed(
                "nominal affine cleanup list does not cover every structural parameter",
            );
        }
        if cleanups.iter().any(|cleanup| {
            !machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == cleanup.place)
        }) {
            return malformed("nominal affine cleanup root is not a structural parameter");
        }
        let mut places = BTreeSet::new();
        for (cleanup, parameter) in cleanups
            .iter()
            .zip(machine.structural_parameters.iter().rev())
        {
            if cleanup.place != parameter.place {
                return malformed("nominal affine cleanup list is not in reverse parameter order");
            }
            let claims = machine
                .entry_claims
                .iter()
                .filter(|claim| claim.input == cleanup.place)
                .collect::<Vec<_>>();
            let complete_custody = match parameter.multiplicity {
                StructuralMultiplicity::Affine => {
                    claims.is_empty() && cleanup.consumed_claims.is_empty()
                }
                StructuralMultiplicity::Linear => matches!(claims.as_slice(), [claim]
                    if claim.path.is_empty() && cleanup.consumed_claims.as_slice() == [claim.claim]),
                StructuralMultiplicity::Unrestricted => false,
            };
            if !places.insert(cleanup.place)
                || !complete_custody
                || parameter.access != terminal_psi::StructuralAccess::Owned
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == cleanup.place)
            {
                return malformed(
                    "nominal cleanup is duplicated or differs from its complete owned custody",
                );
            }
            if parameter.structural_type != cleanup.structural_type {
                return malformed(
                    "nominal affine cleanup type does not match its structural parameter",
                );
            }
        }
    }
    Ok(())
}

/// The residual affine discards of a selected edge or partial return name
/// distinct claim-free affine paths whose leaf type matches the path.
fn validate_partial_affine_discards(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: &Block,
) -> Result<(), CodecError> {
    let rosters = match &block.terminator {
        Terminator::ReturnUnitPartialAffine {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => {
            if !matches!(machine.result, TerminalMachineResult::Unit)
                || residual_affine_discards.is_empty()
            {
                return malformed(
                    "partial affine cleanup requires a Unit result and a residual action",
                );
            }
            vec![(trivial_affine_discards, residual_affine_discards)]
        }
        Terminator::Jump {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => vec![(trivial_affine_discards, residual_affine_discards)],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|edge| {
                (
                    &edge.trivial_affine_discards,
                    &edge.residual_affine_discards,
                )
            })
            .collect(),
        Terminator::StructuralCase { cases, .. } => cases
            .iter()
            .map(|edge| {
                (
                    &edge.trivial_affine_discards,
                    &edge.residual_affine_discards,
                )
            })
            .collect(),
        _ => return Ok(()),
    };
    for (trivial_affine_discards, residual_affine_discards) in rosters {
        for discard in residual_affine_discards {
            let parameter = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == discard.place);
            let block_parameter = || {
                let declaration = machine
                    .structural_places
                    .iter()
                    .find(|declaration| declaration.id == discard.place)?;
                let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind
                else {
                    return None;
                };
                machine
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == block)?
                    .structural_parameters
                    .get(position as usize)
                    .filter(|parameter| {
                        parameter.place == discard.place && parameter.position == position
                    })
            };
            let supported_root = parameter.map_or_else(
                || {
                    is_plain_affine_call_result(machine, discard.place)
                        || block_parameter().is_some_and(|parameter| {
                            parameter.multiplicity == StructuralMultiplicity::Affine
                                && parameter.access == terminal_psi::StructuralAccess::Owned
                                && parameter.qualifications.is_empty()
                                && parameter.projected_qualifications.is_empty()
                        })
                },
                |parameter| parameter.multiplicity == StructuralMultiplicity::Affine,
            );
            if !supported_root
                || discard.path.is_empty()
                || trivial_affine_discards.contains(&discard.place)
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == discard.place)
            {
                return malformed(
                    "partial affine cleanup is not a distinct claim-free affine path",
                );
            }
            let Some(root_type) = structural_place_type(machine, discard.place) else {
                return malformed("partial affine cleanup has no exact structural root type");
            };
            if validate_structural_path(module, root_type, &discard.path)?
                != discard.structural_type
            {
                return malformed("partial affine cleanup leaf type does not match its path");
            }
        }
    }
    Ok(())
}
