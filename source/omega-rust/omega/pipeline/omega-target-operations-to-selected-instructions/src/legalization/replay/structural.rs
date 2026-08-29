use super::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_structural_unit_function(
    function: usize,
    target: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    proposed: &LegalizedStructuralUnitFunction,
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<usize, LegalizationError> {
    let TargetOperation::UnitBody(body) = &target.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [abstract_entry] = abstracted.block_entries.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [optimized_block] = optimized.blocks.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let (
        target_call,
        target_return,
        abstract_call,
        abstract_return,
        optimized_call,
        optimized_return,
        settlement_rows,
    ) = match (
        body.operations.as_slice(),
        abstracted.operations.as_slice(),
        optimized_block.nodes.as_slice(),
    ) {
        (
            [target_return @ TargetUnitOperation::Return { .. }],
            [abstract_return @ AbstractOperation::ReturnUnit { .. }],
            [optimized_return],
        ) => (
            None,
            target_return,
            None,
            abstract_return,
            None,
            optimized_return,
            None,
        ),
        (
            [
                target_call @ TargetUnitOperation::Call { .. },
                target_return @ TargetUnitOperation::Return { .. },
            ],
            [
                abstract_call @ AbstractOperation::CallUnit { .. },
                abstract_return @ AbstractOperation::ReturnUnit { .. },
            ],
            [optimized_call, optimized_return],
        ) => (
            Some(target_call),
            target_return,
            Some(abstract_call),
            abstract_return,
            Some(optimized_call),
            optimized_return,
            None,
        ),
        (
            [
                target_call @ TargetUnitOperation::InstalledProviderCall { .. },
                target_return @ TargetUnitOperation::Return { .. },
            ],
            [
                abstract_call @ AbstractOperation::BoundaryCall { .. },
                abstract_return @ AbstractOperation::ReturnUnit { .. },
            ],
            [optimized_call, optimized_return],
        ) => (
            Some(target_call),
            target_return,
            Some(abstract_call),
            abstract_return,
            Some(optimized_call),
            optimized_return,
            None,
        ),
        (
            [
                target_settlements @ ..,
                target_return @ TargetUnitOperation::Return { .. },
            ],
            [
                abstract_settlements @ ..,
                abstract_return @ AbstractOperation::ReturnUnit { .. },
            ],
            [optimized_settlements @ .., optimized_return],
        ) if !target_settlements.is_empty()
            && target_settlements.len() == abstract_settlements.len()
            && target_settlements.len() == optimized_settlements.len()
            && target_settlements.iter().all(|operation| {
                matches!(
                    operation,
                    TargetUnitOperation::BoundarySettlement {
                        realization:
                            omega_target_operations::BoundaryRealization::ClaimCompletionOnly(_),
                        ..
                    }
                )
            })
            && abstract_settlements
                .iter()
                .all(|operation| matches!(operation, AbstractOperation::BoundaryCall { .. })) =>
        {
            (
                None,
                target_return,
                None,
                abstract_return,
                None,
                optimized_return,
                Some((
                    target_settlements,
                    abstract_settlements,
                    optimized_settlements,
                )),
            )
        }
        _ => return Err(Error::UnsupportedSourceShape { function }),
    };
    let TargetUnitOperation::Return {
        psi_edge,
        cleanup_actions,
    } = target_return
    else {
        unreachable!()
    };
    let expected_provenance = TerminalPsiProvenance {
        operations: if let Some((_, abstract_settlements, _)) = settlement_rows {
            abstract_settlements
                .iter()
                .filter_map(|operation| match operation {
                    AbstractOperation::BoundaryCall { psi_operation, .. } => Some(*psi_operation),
                    _ => None,
                })
                .collect()
        } else {
            abstract_call
                .and_then(|operation| match operation {
                    AbstractOperation::CallUnit { psi_operation, .. } => Some(*psi_operation),
                    AbstractOperation::BoundaryCall { psi_operation, .. } => Some(*psi_operation),
                    _ => None,
                })
                .into_iter()
                .collect()
        },
        edges: vec![*psi_edge],
    };
    let expected_return_effect_input = settlement_rows.map_or_else(
        || u64::from(abstract_call.is_some()),
        |(rows, _, _)| rows.len() as u64,
    );
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target_plan.target),
        &CallSignature {
            parameters: body
                .parameters
                .iter()
                .map(|parameter| parameter.shape)
                .collect(),
            result: None,
        },
    )
    .map_err(|_| Error::UnsupportedSourceShape { function })?;
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || target.attachment != optimized.attachment
        || target.provenance != expected_provenance
        || abstracted.result != omega_abstract_operations::AbstractFunctionResult::Unit
        || optimized.result != abstracted.result
        || !abstracted.parameters.is_empty()
        || !optimized.parameters.is_empty()
        || body.structural_types != abstract_plan.structural_types
        || body.structural_types != unit.structural_types
        || body.call_plan != expected_call_plan
        || abstracted.structural_parameters != optimized.structural_parameters
        || abstracted.entry_claims != optimized.entry_claim_declarations
        || abstracted.published_service_ceiling != optimized.published_service_ceiling
        || abstracted.entry != abstract_entry.block
        || optimized.entry != abstract_entry.block
        || optimized_block.id != abstract_entry.block
        || abstract_entry.operation_offset != 0
        || !abstract_entry.parameters.is_empty()
        || !optimized_block.parameters.is_empty()
        || !cleanup_actions.is_empty()
        || abstract_return != &optimized_return.operation
        || !matches!(abstract_return, AbstractOperation::ReturnUnit { psi_edge: edge, cleanup_actions } if edge == psi_edge && cleanup_actions.is_empty())
        || optimized_return.provenance != [PsiProvenance::Edge(*psi_edge)]
        || optimized_return.effect.input != expected_return_effect_input
        || optimized_return.effect.output != expected_return_effect_input + 1
        || !optimized_return.definitions.is_empty()
        || !optimized_return.uses.is_empty()
        || !optimized_return.successors.is_empty()
        || optimized_return.ownership != [OwnershipEvent::Cleanup(Vec::new())]
        || body.parameters.len() != abstracted.structural_parameters.len()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let expected_places = abstracted
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_claim_ids = abstracted
        .entry_claims
        .iter()
        .map(|claim| claim.claim)
        .collect::<std::collections::BTreeSet<_>>();
    if optimized.declared_places != expected_places
        || optimized.entry_claims != expected_claim_ids
        || abstracted
            .entry_claims
            .iter()
            .any(|claim| !claim.path.is_empty() || !expected_places.contains(&claim.input))
        || proposed.machine != target.machine
        || proposed.attachment != target.attachment
        || proposed.provenance != target.provenance
        || proposed.structural_types != body.structural_types
        || proposed.call_plan != body.call_plan
        || proposed.entry_claims != abstracted.entry_claims
        || proposed.published_service_ceiling != abstracted.published_service_ceiling
        || proposed.entry_block != optimized_block.id
        || proposed.boundary_settlements.len()
            != settlement_rows.map_or(0, |(rows, _, _)| rows.len())
        || proposed.return_edge != *psi_edge
        || proposed.return_fuel != optimized_return.fuel
        || proposed.return_effect != optimized_return.effect
        || proposed.return_ownership != optimized_return.ownership
        || proposed.parameters.len() != body.parameters.len()
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    for ((proposed_parameter, semantic), target_parameter) in proposed
        .parameters
        .iter()
        .zip(&abstracted.structural_parameters)
        .zip(&body.parameters)
    {
        if proposed_parameter.semantic != *semantic
            || proposed_parameter.target != *target_parameter
            || semantic.place != target_parameter.place
            || semantic.structural_type != target_parameter.structural_type
            || semantic.multiplicity != target_parameter.multiplicity
            || semantic.access != target_parameter.access
        {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
    }
    let expected_structural_places = abstracted
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .collect::<Vec<_>>();
    if proposed.structural_places != expected_structural_places {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    if let Some((target_rows, abstract_rows, optimized_rows)) = settlement_rows {
        for (index, (((target_row, abstract_row), optimized_row), proposed_row)) in target_rows
            .iter()
            .zip(abstract_rows)
            .zip(optimized_rows)
            .zip(&proposed.boundary_settlements)
            .enumerate()
        {
            replay_boundary_settlement(
                function,
                index,
                target_row,
                abstract_row,
                optimized_row,
                proposed_row,
                &proposed.parameters,
                &abstracted.entry_claims,
                abstract_plan,
            )?;
        }
    }
    match (target_call, abstract_call, optimized_call, &proposed.call) {
        (None, None, None, None) => {}
        (Some(target_call), Some(abstract_call), Some(optimized_call), Some(proposed_call)) => {
            replay_structural_call(
                function,
                target_call,
                abstract_call,
                optimized_call,
                proposed_call,
                &proposed.parameters,
                &abstracted.entry_claims,
                target_plan,
                abstract_plan,
                unit,
            )?;
        }
        _ => return Err(Error::NonCanonicalLegalizedPlan),
    }
    Ok(0)
}

#[allow(clippy::too_many_arguments)]
fn replay_boundary_settlement(
    function: usize,
    index: usize,
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    optimized: &omega_optimization_unit::OptimizationNode,
    proposed: &omega_legalized_operations::LegalizedBoundarySettlement,
    caller_parameters: &[omega_legalized_operations::LegalizedCallUnitParameter],
    caller_claims: &[psi_terminal::EntryClaim],
    abstract_plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    let TargetUnitOperation::BoundarySettlement {
        psi_operation: target_operation,
        boundary: target_boundary,
        provider_execution,
        realization: omega_target_operations::BoundaryRealization::ClaimCompletionOnly(realization),
        scalar_arguments,
        arguments: target_arguments,
        byte_sequence_arguments,
        completion_claim_sources: target_sources,
        completion_receipts: target_receipts,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let AbstractOperation::BoundaryCall {
        psi_operation,
        result: None,
        boundary,
        arguments,
        structural_arguments,
        completion_claim_sources,
        completion_receipts,
    } = abstracted
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let boundary_declarations = abstract_plan
        .boundary_machines
        .iter()
        .filter(|declaration| declaration.id == *boundary)
        .collect::<Vec<_>>();
    let [declaration] = boundary_declarations.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let expected_evidence = structural_arguments
        .iter()
        .enumerate()
        .map(|(argument_index, argument)| {
            let matching_claims = caller_claims
                .iter()
                .filter(|claim| claim.input == argument.place && claim.path.is_empty())
                .collect::<Vec<_>>();
            let [claim] = matching_claims.as_slice() else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            Ok((
                omega_abstract_operations::CompletionClaimSource {
                    claim: claim.claim,
                    entry: Some((*claim).clone()),
                    content: None,
                },
                psi_terminal::CompletionReceipt {
                    claim: claim.claim,
                    argument_index: argument_index as u32,
                },
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_receipts = expected_evidence
        .iter()
        .map(|(_, receipt)| *receipt)
        .collect::<Vec<_>>();
    let expected_sources = caller_claims
        .iter()
        .cloned()
        .map(|entry| omega_abstract_operations::CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    let completed_claims = expected_receipts
        .iter()
        .map(|receipt| receipt.claim)
        .collect::<Vec<_>>();
    if target_operation != psi_operation
        || target_boundary != boundary
        || !scalar_arguments.is_empty()
        || !arguments.is_empty()
        || !byte_sequence_arguments.is_empty()
        || target_arguments != structural_arguments
        || target_sources != completion_claim_sources
        || target_receipts != completion_receipts
        || structural_arguments.is_empty()
        || !declaration.scalar_parameters.is_empty()
        || declaration.result.is_some()
        || !declaration.program_local_root_introductions.is_empty()
        || !declaration.content_guarantees.is_empty()
        || !declaration.published_service_ceiling.is_empty()
        || declaration.structural_parameters.len() != structural_arguments.len()
        || declaration.requires.iter().any(|requirement| {
            requirement.argument_index as usize >= declaration.structural_parameters.len()
        })
        || completion_receipts != &expected_receipts
        || completion_claim_sources != &expected_sources
        || optimized.operation != *abstracted
        || optimized.provenance != [PsiProvenance::Operation(*psi_operation)]
        || optimized.effect.input != index as u64
        || optimized.effect.output != index as u64 + 1
        || !optimized.definitions.is_empty()
        || !optimized.uses.is_empty()
        || !optimized.successors.is_empty()
        || optimized.ownership != [OwnershipEvent::ClaimCompletion(completed_claims)]
        || structural_arguments
            .iter()
            .enumerate()
            .any(|(index, argument)| {
                let caller_matches = caller_parameters
                    .iter()
                    .filter(|parameter| parameter.semantic.place == argument.place)
                    .collect::<Vec<_>>();
                let [caller] = caller_matches.as_slice() else {
                    return true;
                };
                let boundary_parameter = &declaration.structural_parameters[index];
                let mut expected_qualifications = boundary_parameter.qualifications.clone();
                expected_qualifications.extend(
                    declaration
                        .requires
                        .iter()
                        .filter(|requirement| requirement.argument_index as usize == index)
                        .map(|requirement| requirement.domain),
                );
                expected_qualifications.sort_unstable();
                expected_qualifications.dedup();
                !argument.path.is_empty()
                    || argument.access != psi_terminal::StructuralAccess::Owned
                    || caller.semantic.multiplicity != psi_terminal::StructuralMultiplicity::Linear
                    || caller.semantic.access != psi_terminal::StructuralAccess::Owned
                    || boundary_parameter.position != index as u32
                    || boundary_parameter.structural_type != caller.semantic.structural_type
                    || boundary_parameter.multiplicity != caller.semantic.multiplicity
                    || boundary_parameter.access != caller.semantic.access
                    || expected_qualifications != caller.semantic.qualifications
            })
        || proposed.operation != *psi_operation
        || proposed.boundary != *boundary
        || proposed.provider_execution != *provider_execution
        || proposed.realization != *realization
        || proposed.arguments != *structural_arguments
        || proposed.completion_claim_sources != *completion_claim_sources
        || proposed.completion_receipts != *completion_receipts
        || proposed.fuel != optimized.fuel
        || proposed.effect != optimized.effect
        || proposed.ownership != optimized.ownership
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_structural_call(
    function: usize,
    target_call: &TargetUnitOperation,
    abstract_call: &AbstractOperation,
    optimized_call: &omega_optimization_unit::OptimizationNode,
    proposed: &omega_legalized_operations::LegalizedCallUnit,
    caller_parameters: &[omega_legalized_operations::LegalizedCallUnitParameter],
    caller_claims: &[psi_terminal::EntryClaim],
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let (
        psi_operation,
        callee,
        structural_arguments,
        target_arguments,
        claim_transfers,
        expected_source,
        expected_ownership,
    ) = match (target_call, abstract_call) {
        (
            TargetUnitOperation::Call {
                psi_operation: target_operation,
                callee: target_callee,
                arguments: target_arguments,
                claim_transfers: target_transfers,
            },
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                claim_transfers,
            },
        ) if target_operation == psi_operation
            && target_callee == callee
            && target_transfers == claim_transfers =>
        {
            (
                *psi_operation,
                *callee,
                structural_arguments,
                target_arguments,
                claim_transfers,
                omega_legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit,
                OwnershipEvent::ClaimTransfer(
                    claim_transfers
                        .iter()
                        .map(|transfer| transfer.claim)
                        .collect(),
                ),
            )
        }
        (
            TargetUnitOperation::InstalledProviderCall {
                psi_operation: target_operation,
                boundary: target_boundary,
                provider,
                source_arguments,
                arguments: target_arguments,
                claim_transfers: target_transfers,
                completion_claim_sources: target_sources,
                completion_receipts: target_receipts,
            },
            AbstractOperation::BoundaryCall {
                psi_operation,
                result: None,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            },
        ) if target_operation == psi_operation
            && target_boundary == boundary
            && arguments.is_empty()
            && source_arguments == structural_arguments
            && target_sources == completion_claim_sources
            && target_receipts == completion_receipts
            && target_transfers
                == &completion_receipts
                    .iter()
                    .map(|receipt| psi_terminal::ClaimTransfer {
                        claim: receipt.claim,
                        argument_index: receipt.argument_index,
                    })
                    .collect::<Vec<_>>()
            && provider.boundary == *boundary
            && abstract_plan
                .provider_candidates
                .iter()
                .any(|candidate| candidate == provider) =>
        {
            (
                *psi_operation,
                provider.candidate,
                structural_arguments,
                target_arguments,
                target_transfers,
                omega_legalized_operations::LegalizedCallUnitSource::InstalledProvider {
                    boundary: *boundary,
                    provider: provider.clone(),
                    completion_claim_sources: completion_claim_sources.clone(),
                    completion_receipts: completion_receipts.clone(),
                },
                OwnershipEvent::ClaimCompletion(
                    completion_receipts
                        .iter()
                        .map(|receipt| receipt.claim)
                        .collect(),
                ),
            )
        }
        _ => return Err(Error::UnsupportedSourceShape { function }),
    };
    if optimized_call.operation != *abstract_call
        || optimized_call.provenance != [PsiProvenance::Operation(psi_operation)]
        || optimized_call.ownership != [expected_ownership]
        || optimized_call.effect.input != 0
        || optimized_call.effect.output != 1
        || !optimized_call.definitions.is_empty()
        || !optimized_call.uses.is_empty()
        || !optimized_call.successors.is_empty()
        || proposed.source != expected_source
        || proposed.operation != psi_operation
        || proposed.callee != callee
        || proposed.claim_transfers != *claim_transfers
        || proposed.fuel != optimized_call.fuel
        || proposed.effect != optimized_call.effect
        || proposed.ownership != optimized_call.ownership
        || proposed.arguments.len() != structural_arguments.len()
        || proposed.arguments.len() != target_arguments.len()
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    proposed
        .validate_source()
        .map_err(|_| Error::NonCanonicalLegalizedPlan)?;
    for (((proposed_argument, semantic), target_argument), source) in proposed
        .arguments
        .iter()
        .zip(structural_arguments)
        .zip(target_arguments)
        .map(|triple| {
            let source = caller_parameters
                .iter()
                .find(|parameter| parameter.semantic.place == triple.0.1.place);
            (triple, source)
        })
    {
        let Some(source) = source else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        if proposed_argument.semantic != *semantic
            || proposed_argument.target != *target_argument
            || semantic.place != target_argument.place
            || semantic.access != target_argument.access
            || !semantic.path.is_empty()
            || !target_argument.path.is_empty()
            || target_argument.root_structural_type != source.semantic.structural_type
            || target_argument.structural_type != source.semantic.structural_type
            || target_argument.shape != source.target.shape
            || target_argument.source_byte_offset != 0
            || target_argument.fixed_array_length.is_some()
            || target_argument.element_stride.is_some()
            || target_argument.source != source.target.placement
        {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
    }

    let target_callees = target_plan
        .functions
        .iter()
        .filter(|candidate| candidate.machine == callee)
        .collect::<Vec<_>>();
    let abstract_callees = abstract_plan
        .functions
        .iter()
        .filter(|candidate| candidate.machine == callee)
        .collect::<Vec<_>>();
    let optimized_callees = unit
        .functions
        .iter()
        .filter(|candidate| candidate.machine == callee)
        .collect::<Vec<_>>();
    let ([target_callee], [abstract_callee], [optimized_callee]) = (
        target_callees.as_slice(),
        abstract_callees.as_slice(),
        optimized_callees.as_slice(),
    ) else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let TargetOperation::UnitBody(callee_body) = &target_callee.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let expected_callee_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target_plan.target),
        &CallSignature {
            parameters: callee_body
                .parameters
                .iter()
                .map(|parameter| parameter.shape)
                .collect(),
            result: None,
        },
    )
    .map_err(|_| Error::UnsupportedSourceShape { function })?;
    if abstract_callee.result != omega_abstract_operations::AbstractFunctionResult::Unit
        || optimized_callee.result != abstract_callee.result
        || !abstract_callee.parameters.is_empty()
        || !optimized_callee.parameters.is_empty()
        || abstract_callee.structural_parameters != optimized_callee.structural_parameters
        || abstract_callee.entry_claims != optimized_callee.entry_claim_declarations
        || callee_body.call_plan != expected_callee_plan
        || callee_body.parameters.len() != abstract_callee.structural_parameters.len()
        || proposed.arguments.len() != abstract_callee.structural_parameters.len()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    for ((argument, semantic_parameter), target_parameter) in proposed
        .arguments
        .iter()
        .zip(&abstract_callee.structural_parameters)
        .zip(&callee_body.parameters)
    {
        if argument.semantic.access != semantic_parameter.access
            || argument.target.structural_type != semantic_parameter.structural_type
            || argument.target.shape != target_parameter.shape
            || argument.target.destination != target_parameter.placement
            || semantic_parameter.place != target_parameter.place
            || semantic_parameter.structural_type != target_parameter.structural_type
            || semantic_parameter.multiplicity != target_parameter.multiplicity
            || semantic_parameter.access != target_parameter.access
        {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
    }
    if proposed.claim_transfers.len() != abstract_callee.entry_claims.len() {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    for (transfer, callee_claim) in proposed
        .claim_transfers
        .iter()
        .zip(&abstract_callee.entry_claims)
    {
        let argument_index = usize::try_from(transfer.argument_index)
            .map_err(|_| Error::NonCanonicalLegalizedPlan)?;
        let Some(argument) = proposed.arguments.get(argument_index) else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        let Some(callee_parameter) = abstract_callee.structural_parameters.get(argument_index)
        else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        let caller_claims = caller_claims
            .iter()
            .filter(|claim| claim.claim == transfer.claim)
            .collect::<Vec<_>>();
        let [caller_claim] = caller_claims.as_slice() else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        if caller_claim.input != argument.semantic.place
            || !caller_claim.path.is_empty()
            || callee_claim.input != callee_parameter.place
            || !callee_claim.path.is_empty()
        {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
    }
    Ok(())
}
