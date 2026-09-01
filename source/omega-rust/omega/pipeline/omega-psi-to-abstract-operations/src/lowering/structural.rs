use super::LoweringError;
use crate::shared::*;

/// Lower only the first complete structural ABI requirement: one verified
/// whole-root linear parameter is returned unchanged with its one live claim.
/// Wider verified terminal programs remain fenced until their target-neutral
/// carrier and Omega realization land together.
pub(super) fn lower_structural_machine(
    machine: &TerminalMachine,
    result: &StructuralResultDeclaration,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
) -> Result<AbstractFunction, LoweringError> {
    let unsupported = || LoweringError::UnsupportedStructuralResult(machine.id);
    let Some(parameter) = machine.structural_parameters.first() else {
        return Err(unsupported());
    };
    let discarded = machine.structural_parameters.get(1..).unwrap_or_default();
    if machine.structural_parameters.is_empty() {
        return Err(unsupported());
    }
    let [entry_claim] = machine.entry_claims.as_slice() else {
        return Err(unsupported());
    };
    let [block] = machine.blocks.as_slice() else {
        return Err(unsupported());
    };
    if let [operation] = block.operations.as_slice()
        && let OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        } = &operation.kind
        && let Some(operation_result) = operation.result.structural()
        && let Terminator::ReturnStructural {
            edge,
            source,
            returned_claims,
            trivial_affine_discards,
        } = &block.terminator
    {
        let [argument] = structural_arguments.as_slice() else {
            return Err(unsupported());
        };
        let [claim_transfer] = claim_transfers.as_slice() else {
            return Err(unsupported());
        };
        let [returned_transfer] = returned_claim_transfers.as_slice() else {
            return Err(unsupported());
        };
        let [result_claim] = operation_result.claims.as_slice() else {
            return Err(unsupported());
        };
        let operation_place = machine
            .structural_places
            .iter()
            .find(|place| place.id == operation_result.place)
            .ok_or_else(unsupported)?;
        if machine.structural_parameters.len() != 1
            || !discarded.is_empty()
            || !machine.parameters.is_empty()
            || parameter.position != 0
            || parameter.is_self
            || parameter.multiplicity != StructuralMultiplicity::Linear
            || result.multiplicity != StructuralMultiplicity::Linear
            || parameter.structural_type != result.structural_type
            || parameter.qualifications != result.qualifications
            || operation_result.structural_type != result.structural_type
            || operation_result.multiplicity != result.multiplicity
            || operation_result.qualifications != result.qualifications
            || operation_result.projected_qualifications != result.projected_qualifications
            || argument.place != parameter.place
            || argument.access != psi_terminal::StructuralAccess::Owned
            || !argument.path.is_empty()
            || claim_transfer.argument_index != 0
            || claim_transfer.claim != entry_claim.claim
            || returned_transfer.caller_claim != entry_claim.claim
            || result_claim.claim != entry_claim.claim
            || !result_claim.path.is_empty()
            || *source != operation_result.place
            || returned_claims.as_slice() != [entry_claim.claim]
            || !trivial_affine_discards.is_empty()
            || !requirement_obligations.is_empty()
            || !crash_continuations.is_empty()
            || block.id != machine.entry
            || !block.parameters.is_empty()
            || !machine.published_service_ceiling.is_empty()
            || !machine.contract.crash_routes.is_empty()
            || !machine.contract.requires.is_empty()
            || !machine.contract.ensures.is_empty()
            || machine.structural_places.len() != 3
            || !matches!(
                operation_place.kind,
                StructuralPlaceKind::OperationResult { producer, structural_type }
                    if producer == operation.id && structural_type == result.structural_type
            )
        {
            return Err(unsupported());
        }
        return Ok(AbstractFunction {
            machine: machine.id,
            attachment: machine.attachment,
            entry: machine.entry,
            parameters: Vec::new(),
            structural_parameters: machine.structural_parameters.clone(),
            result: AbstractFunctionResult::Structural(result.clone()),
            entry_claims: vec![entry_claim.clone()],
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: block.id,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::CallStructural {
                    psi_operation: operation.id,
                    result: operation_result.clone(),
                    callee: *callee,
                    structural_arguments: structural_arguments.clone(),
                    claim_transfers: claim_transfers.clone(),
                    returned_claim_transfers: returned_claim_transfers.clone(),
                    requirement_obligations: requirement_obligations.clone(),
                    crash_continuations: crash_continuations.clone(),
                    selected_evidence: selected_evidence.clone(),
                },
                AbstractOperation::ReturnStructural {
                    psi_edge: *edge,
                    source: *source,
                    returned_claims: returned_claims.clone(),
                    trivial_affine_locals: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            ],
        });
    }
    let parameter_place = machine
        .structural_places
        .iter()
        .find(|place| place.id == parameter.place)
        .ok_or_else(unsupported)?;
    let result_place = machine
        .structural_places
        .iter()
        .find(|place| place.id == result.place)
        .ok_or_else(unsupported)?;
    let discarded_places = discarded
        .iter()
        .map(|discarded| {
            machine
                .structural_places
                .iter()
                .find(|place| place.id == discarded.place)
                .ok_or_else(unsupported)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let trivial_affine_locals = block
        .operations
        .iter()
        .map(|operation| {
            let OperationKind::EstablishTrivialAffineLocal { destination } = operation.kind else {
                return Err(unsupported());
            };
            if operation.result != OperationResult::Unit {
                return Err(unsupported());
            }
            let declaration = machine
                .structural_places
                .iter()
                .find(|place| place.id == destination)
                .cloned()
                .ok_or_else(unsupported)?;
            let psi_core::StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } = declaration.kind
            else {
                return Err(unsupported());
            };
            let local_type = structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .cloned()
                .ok_or_else(unsupported)?;
            if !matches!(
                local_type.shape,
                psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty()
            ) {
                return Err(unsupported());
            }
            Ok((operation.id, declaration, local_type))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let Terminator::ReturnStructural {
        edge,
        source,
        returned_claims,
        trivial_affine_discards,
    } = &block.terminator
    else {
        return Err(unsupported());
    };

    if !machine.parameters.is_empty()
        || parameter.position != 0
        || parameter.is_self
        || discarded.iter().enumerate().any(|(index, discarded)| {
            usize::try_from(discarded.position) != Ok(index + 1)
                || discarded.is_self
                || discarded.multiplicity != StructuralMultiplicity::Affine
                || !discarded.qualifications.is_empty()
        })
        || machine
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != machine.structural_parameters.len()
        || parameter.multiplicity != StructuralMultiplicity::Linear
        || result.multiplicity != StructuralMultiplicity::Linear
        || parameter.structural_type != result.structural_type
        || parameter.qualifications != result.qualifications
        || parameter.place != *source
        || entry_claim.input != parameter.place
        || !entry_claim.path.is_empty()
        || returned_claims.as_slice() != [entry_claim.claim]
        || trivial_affine_discards
            != &trivial_affine_locals
                .iter()
                .rev()
                .map(|(_, local, _)| local.id)
                .chain(discarded.iter().rev().map(|discarded| discarded.place))
                .collect::<Vec<_>>()
        || block.id != machine.entry
        || !block.parameters.is_empty()
        || !machine.published_service_ceiling.is_empty()
        || !machine.contract.crash_routes.is_empty()
        || !machine.contract.requires.is_empty()
        || !machine.contract.ensures.is_empty()
        || parameter_place.id != parameter.place
        || !matches!(
            parameter_place.kind,
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false
            }
        )
        || result_place.id != result.place
        || result_place.kind != StructuralPlaceKind::Result
        || discarded_places.iter().enumerate().any(|(index, place)| {
            !matches!(
                place.kind,
                StructuralPlaceKind::Parameter {
                    position,
                    is_self: false
                } if usize::try_from(position) == Ok(index + 1)
            )
        })
        || trivial_affine_locals
            .iter()
            .enumerate()
            .any(|(index, (_, local, local_type))| {
                !matches!(
                    local.kind,
                    StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal,
                        structural_type,
                        construction: None,
                    } if usize::try_from(declaration_ordinal) == Ok(index)
                        && structural_type == local_type.id
                )
            })
        || trivial_affine_locals
            .iter()
            .map(|(_, local, _)| local.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != trivial_affine_locals.len()
        || machine.structural_places.len()
            != machine.structural_parameters.len() + trivial_affine_locals.len() + 1
    {
        return Err(unsupported());
    }

    Ok(AbstractFunction {
        machine: machine.id,
        attachment: machine.attachment,
        entry: machine.entry,
        parameters: Vec::new(),
        structural_parameters: machine.structural_parameters.clone(),
        result: AbstractFunctionResult::Structural(result.clone()),
        entry_claims: vec![entry_claim.clone()],
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            block: block.id,
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations: vec![AbstractOperation::ReturnStructural {
            psi_edge: *edge,
            source: *source,
            returned_claims: returned_claims.clone(),
            trivial_affine_locals,
            trivial_affine_discards: trivial_affine_discards.clone(),
        }],
    })
}
