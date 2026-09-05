use super::LoweringError;
use crate::shared::*;

/// Lower the exact admitted structural-return families: the established
/// whole-root linear/claim-bearing return and one claim-free affine return with
/// a fixed-integer scalar prefix. Wider verified Terminal programs remain
/// fenced until their target-neutral carrier and Omega realization land
/// together.
pub(super) fn lower_structural_machine(
    machine: &TerminalMachine,
    result: &StructuralResultDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<AbstractFunction, LoweringError> {
    let unsupported = || LoweringError::UnsupportedStructuralResult(machine.id);
    if let Some(lowered) = lower_claim_free_affine_mixed_machine(machine, result, structural_types)?
    {
        return Ok(lowered);
    }
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
            || argument.access != terminal_psi::StructuralAccess::Owned
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
                    arguments: Vec::new(),
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
            let semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                structural_type,
                ..
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
                terminal_psi::StructuralTypeShape::Record { ref fields } if fields.is_empty()
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

fn lower_claim_free_affine_mixed_machine(
    machine: &TerminalMachine,
    result: &StructuralResultDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Option<AbstractFunction>, LoweringError> {
    let ([scalar_parameter], [structural_parameter], [block]) = (
        machine.parameters.as_slice(),
        machine.structural_parameters.as_slice(),
        machine.blocks.as_slice(),
    ) else {
        return Ok(None);
    };
    let ScalarType::Integer(integer) = scalar_parameter.scalar_type else {
        return Ok(None);
    };
    let Terminator::ReturnStructural {
        edge,
        source,
        returned_claims,
        trivial_affine_discards,
    } = &block.terminator
    else {
        return Ok(None);
    };
    let exact_record = structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .is_some_and(|declaration| {
            matches!(
                &declaration.shape,
                terminal_psi::StructuralTypeShape::Record { fields }
                    if matches!(
                        fields.as_slice(),
                        [field]
                            if matches!(
                                field.field_type,
                                terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(field_integer))
                                    if field_integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                                        && field_integer.bits() == 64
                            )
                    )
            )
        });
    let parameter_place = machine
        .structural_places
        .iter()
        .find(|place| place.id == structural_parameter.place);
    let result_place = machine
        .structural_places
        .iter()
        .find(|place| place.id == result.place);
    if integer.is_address()
        || !matches!(integer.bits(), 8 | 16 | 32 | 64)
        || !exact_record
        || !machine.entry_claims.is_empty()
        || !machine.content_entry_claims.is_empty()
        || !machine.published_service_ceiling.is_empty()
        || !machine.contract.crash_routes.is_empty()
        || !machine.contract.requires.is_empty()
        || !machine.contract.ensures.is_empty()
        || block.id != machine.entry
        || !block.parameters.is_empty()
        || !block.operations.is_empty()
        || structural_parameter.position != 0
        || structural_parameter.is_self
        || structural_parameter.multiplicity != StructuralMultiplicity::Affine
        || structural_parameter.access != terminal_psi::StructuralAccess::Owned
        || !structural_parameter.qualifications.is_empty()
        || !structural_parameter.projected_qualifications.is_empty()
        || structural_parameter.structural_type != result.structural_type
        || *source != structural_parameter.place
        || result.place == structural_parameter.place
        || result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !returned_claims.is_empty()
        || !trivial_affine_discards.is_empty()
        || machine.structural_places.len() != 2
        || !parameter_place.is_some_and(|place| {
            matches!(
                place.kind,
                StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                }
            )
        })
        || result_place.is_none_or(|place| place.kind != StructuralPlaceKind::Result)
    {
        return Ok(None);
    }
    let parameter = AbstractParameter {
        value: scalar_parameter.id,
        scalar_type: scalar_parameter.scalar_type,
    };
    Ok(Some(AbstractFunction {
        machine: machine.id,
        attachment: machine.attachment,
        entry: machine.entry,
        parameters: vec![parameter],
        structural_parameters: vec![structural_parameter.clone()],
        result: AbstractFunctionResult::Structural(result.clone()),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            block: block.id,
            parameters: vec![parameter],
            operation_offset: 0,
        }],
        operations: vec![AbstractOperation::ReturnStructural {
            psi_edge: *edge,
            source: *source,
            returned_claims: Vec::new(),
            trivial_affine_locals: Vec::new(),
            trivial_affine_discards: Vec::new(),
        }],
    }))
}
