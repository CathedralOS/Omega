//! One machine's structural foundation: `validate_machine` checks its
//! attachment and parameters, then each block's parameters and places,
//! entry claims, operation results and the affine cleanups its terminators
//! declare.

use super::{
    has_structural_type, is_plain_affine_operation_result, require_known_services,
    structural_place_type, validate_operation_foundation, validate_provider_attachment_foundation,
    validate_structural_parameters, validate_structural_path,
};
use crate::codec_error::{CodecError, malformed};
use crate::sections::semantic_module::structural_result_wire;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::Block;
use terminal_psi::{StructuralMultiplicity, TerminalMachine, TerminalModule, Terminator};

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
    validate_loan_completions(module, machine)?;
    for operation in machine.operations() {
        validate_operation_foundation(module, machine, operation)?;
    }
    validate_operation_result_places(machine)?;
    for block in &machine.blocks {
        super::call_foundations::validate_external_completion(module, machine, block)?;
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
                .block(&block)
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
        let Some(place) = machine.structural_place(parameter.place) else {
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
        let Some(parameter) = machine.signature_structural_parameter(claim.input) else {
            return malformed("entry claim is not bound to a structural parameter");
        };
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
            return malformed("entry claim cannot bind an unrestricted parameter");
        }
        validate_structural_path(module, parameter.structural_type, &claim.path)?;
    }
    Ok(())
}

/// Completion rows have an actual call, owned home and typed payload paths.
/// This validates representation shape; source checking still owns exclusive
/// authority and the callee's obligation to return established storage.
fn validate_loan_completions(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    for completion in &machine.loan_completions {
        let operation = machine.operation(completion.operation).ok_or(
            CodecError::MalformedStructuralFoundation("loan completion has no call operation"),
        )?;
        if !matches!(
            &operation.kind,
            terminal_psi::call_structural_argument_variants!(_arguments)
        ) {
            return malformed("loan completion is not a structural call");
        }
        let root = structural_place_type(machine, completion.place).ok_or(
            CodecError::MalformedStructuralFoundation("loan completion has no owned home"),
        )?;
        if machine
            .structural_parameter(completion.place)
            .is_some_and(|parameter| parameter.access != terminal_psi::StructuralAccess::Owned)
        {
            return malformed("loan completion cannot own borrowed ingress");
        }
        validate_structural_path(module, root, &completion.path)?;
        for claim in &completion.claims {
            if !claim.path.starts_with(&completion.path) {
                return malformed("loan completion claim lies outside its returned region");
            }
            validate_structural_path(module, root, &claim.path)?;
        }
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
            .operations()
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
/// The residual affine discards of a selected edge or partial return name
/// distinct claim-free affine paths whose leaf type matches the path.
fn validate_partial_affine_discards(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: &Block,
) -> Result<(), CodecError> {
    let rosters = match &block.terminator {
        Terminator::Jump {
            cleanup_actions, ..
        } => vec![cleanup_actions],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => vec![&when_true.cleanup_actions, &when_false.cleanup_actions],
        Terminator::StructuralCase { cases, .. } => {
            cases.iter().map(|edge| &edge.cleanup_actions).collect()
        }
        _ => return Ok(()),
    };
    for actions in rosters {
        for discard in actions.iter().filter_map(|action| match action {
            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => Some(discard),
            _ => None,
        }) {
            let parameter = machine.signature_structural_parameter(discard.place);
            let block_parameter = || {
                let declaration = machine.structural_place(discard.place)?;
                let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind
                else {
                    return None;
                };
                machine
                    .block(&block)?
                    .structural_parameters
                    .get(position as usize)
                    .filter(|parameter| {
                        parameter.place == discard.place && parameter.position == position
                    })
            };
            let supported_root = parameter.map_or_else(
                || {
                    is_plain_affine_operation_result(machine, discard.place)
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
                || actions.iter().any(|action| matches!(action, terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) if *place == discard.place))
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
