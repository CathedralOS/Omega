use omega_abstract_operations::{AbstractOperation, AbstractOperationPlan};
use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use omega_legalized_operations::{
    LegalizationRecipe, LegalizationTheorem,
    LegalizedActiveResidentExactAddChain as SourceActiveResidentExactAddChain,
    LegalizedBoundarySettlement, LegalizedCallUnit, LegalizedCallUnitArgument,
    LegalizedCallUnitParameter, LegalizedExactAdd as SourceExactAdd,
    LegalizedFunction as SourceFunction, LegalizedImmediate as SourceImmediate,
    LegalizedLeaf as SourceLeaf, LegalizedLeafValue as SourceLeafValue,
    LegalizedStructuralUnitFunction as SourceStructuralUnitFunction, LegalizedTemporaryId,
    LegalizedUnitFunction as SourceUnitFunction,
};
use omega_optimization_unit::{
    AcceptedObligationFact, FuelSettlement, OptimizationFact, OwnershipEvent, PsiOptimizationUnit,
    PsiProvenance,
};
use omega_target_operations::{
    ScalarParameterLocation, TargetIntegerControl, TargetIntegerExpression, TargetOperation,
    TargetOperationPlan, TargetUnitOperation, TerminalPsiProvenance,
};
use psi_core::{EdgeId, IntegerSign, OperationId, ScalarType, StructuralPlaceKind};
use psi_terminal::StructuralPlaceDeclaration;

use crate::{LegalizationError, LegalizationError as Error};

pub(crate) fn derive_source_functions(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<SourceFunction>, LegalizationError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::SourceCustodyMismatch);
    }

    let functions = target
        .functions
        .iter()
        .zip(&abstract_plan.functions)
        .zip(&unit.functions)
        .enumerate()
        .filter(|(_, ((target, _), _))| !matches!(target.operation, TargetOperation::UnitBody(_)))
        .map(|(index, ((target, abstracted), optimized))| {
            derive_source_function(
                index,
                target,
                abstracted,
                optimized,
                &unit.accepted_obligation_facts,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if functions.iter().any(|function| {
        function.condition_register.architecture() != target.target.architecture
            || match (&function.when_true.value, &function.when_false.value) {
                (
                    SourceLeafValue::EntryParameter { register: left, .. },
                    SourceLeafValue::EntryParameter {
                        register: right, ..
                    },
                ) => {
                    left.architecture() != target.target.architecture
                        || right.architecture() != target.target.architecture
                }
                _ => false,
            }
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(functions)
}

pub(crate) fn derive_source_unit_functions(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<SourceUnitFunction>, LegalizationError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::SourceCustodyMismatch);
    }
    target
        .functions
        .iter()
        .zip(&abstract_plan.functions)
        .zip(&unit.functions)
        .enumerate()
        .filter(|(_, ((target, abstracted), optimized))| {
            is_plain_unit_function(target, abstracted, optimized)
        })
        .map(|(index, ((target, abstracted), optimized))| {
            derive_source_unit_function(index, target, abstracted, optimized)
        })
        .collect()
}

pub(crate) fn derive_source_structural_unit_functions(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<SourceStructuralUnitFunction>, LegalizationError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::SourceCustodyMismatch);
    }

    target
        .functions
        .iter()
        .enumerate()
        .filter_map(|(index, target_function)| {
            let abstract_matches = abstract_plan
                .functions
                .iter()
                .filter(|candidate| candidate.machine == target_function.machine)
                .collect::<Vec<_>>();
            let optimized_matches = unit
                .functions
                .iter()
                .filter(|candidate| candidate.machine == target_function.machine)
                .collect::<Vec<_>>();
            let ([abstracted], [optimized]) =
                (abstract_matches.as_slice(), optimized_matches.as_slice())
            else {
                return Some(Err(Error::SourceCustodyMismatch));
            };
            matches!(target_function.operation, TargetOperation::UnitBody(_))
                .then_some((index, target_function, *abstracted, *optimized))
                .filter(|(_, target_function, abstracted, optimized)| {
                    !is_plain_unit_function(target_function, abstracted, optimized)
                })
                .map(|(index, target_function, abstracted, optimized)| {
                    derive_source_structural_unit_function(
                        index,
                        target_function,
                        abstracted,
                        optimized,
                        target,
                        abstract_plan,
                        unit,
                    )
                })
        })
        .collect()
}

fn is_plain_unit_function(
    target: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
) -> bool {
    let TargetOperation::UnitBody(body) = &target.operation else {
        return false;
    };
    body.parameters.is_empty()
        && abstracted.structural_parameters.is_empty()
        && optimized.structural_parameters.is_empty()
        && abstracted.entry_claims.is_empty()
        && optimized.entry_claim_declarations.is_empty()
        && optimized.entry_claims.is_empty()
        && optimized.declared_places.is_empty()
        && abstracted.published_service_ceiling.is_empty()
        && optimized.published_service_ceiling.is_empty()
        && matches!(
            body.operations.as_slice(),
            [TargetUnitOperation::Return { .. }]
        )
}

#[allow(clippy::too_many_arguments)]
fn derive_source_structural_unit_function(
    function: usize,
    target: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<SourceStructuralUnitFunction, LegalizationError> {
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
                    AbstractOperation::CallUnit { psi_operation, .. }
                    | AbstractOperation::BoundaryCall { psi_operation, .. } => Some(*psi_operation),
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

    let parameters = abstracted
        .structural_parameters
        .iter()
        .zip(&body.parameters)
        .map(|(semantic, target)| {
            (semantic.place == target.place
                && semantic.structural_type == target.structural_type
                && semantic.multiplicity == target.multiplicity
                && semantic.access == target.access)
                .then(|| LegalizedCallUnitParameter {
                    semantic: semantic.clone(),
                    target: target.clone(),
                })
                .ok_or(Error::UnsupportedSourceShape { function })
        })
        .collect::<Result<Vec<_>, _>>()?;
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
    if body.call_plan != expected_call_plan {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let structural_places = synthesized_parameter_places(&abstracted.structural_parameters);
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
    {
        return Err(Error::UnsupportedSourceShape { function });
    }

    let call = match (target_call, abstract_call, optimized_call) {
        (None, None, None) => None,
        (Some(target_call), Some(abstract_call), Some(optimized_call)) => {
            Some(derive_structural_call(
                function,
                target_call,
                abstract_call,
                optimized_call,
                &parameters,
                &abstracted.entry_claims,
                target_plan,
                abstract_plan,
                unit,
            )?)
        }
        _ => return Err(Error::UnsupportedSourceShape { function }),
    };
    let boundary_settlements = settlement_rows
        .map(|(target_rows, abstract_rows, optimized_rows)| {
            target_rows
                .iter()
                .zip(abstract_rows)
                .zip(optimized_rows)
                .enumerate()
                .map(|(index, ((target, abstract_row), optimized))| {
                    derive_boundary_settlement(
                        function,
                        index,
                        target,
                        abstract_row,
                        optimized,
                        &parameters,
                        &abstracted.entry_claims,
                        abstract_plan,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(SourceStructuralUnitFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        structural_types: body.structural_types.clone(),
        call_plan: body.call_plan.clone(),
        parameters,
        structural_places,
        entry_claims: abstracted.entry_claims.clone(),
        published_service_ceiling: abstracted.published_service_ceiling.clone(),
        entry_block: optimized_block.id,
        boundary_settlements,
        call,
        return_edge: *psi_edge,
        return_fuel: optimized_return.fuel.clone(),
        return_effect: optimized_return.effect,
        return_ownership: optimized_return.ownership.clone(),
    })
}

fn derive_boundary_settlement(
    function: usize,
    index: usize,
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    optimized: &omega_optimization_unit::OptimizationNode,
    caller_parameters: &[LegalizedCallUnitParameter],
    caller_claims: &[psi_terminal::EntryClaim],
    abstract_plan: &AbstractOperationPlan,
) -> Result<LegalizedBoundarySettlement, LegalizationError> {
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
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok(LegalizedBoundarySettlement {
        operation: *psi_operation,
        boundary: *boundary,
        provider_execution: *provider_execution,
        realization: *realization,
        arguments: structural_arguments.clone(),
        completion_claim_sources: completion_claim_sources.clone(),
        completion_receipts: completion_receipts.clone(),
        fuel: optimized.fuel.clone(),
        effect: optimized.effect,
        ownership: optimized.ownership.clone(),
    })
}

fn synthesized_parameter_places(
    parameters: &[psi_terminal::StructuralParameterDeclaration],
) -> Vec<StructuralPlaceDeclaration> {
    parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn derive_structural_call(
    function: usize,
    target_call: &TargetUnitOperation,
    abstract_call: &AbstractOperation,
    optimized_call: &omega_optimization_unit::OptimizationNode,
    caller_parameters: &[LegalizedCallUnitParameter],
    caller_claims: &[psi_terminal::EntryClaim],
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedCallUnit, LegalizationError> {
    let (
        psi_operation,
        callee,
        structural_arguments,
        target_arguments,
        claim_transfers,
        source,
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
        || structural_arguments.len() != target_arguments.len()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let arguments = structural_arguments
        .iter()
        .zip(target_arguments)
        .map(|(semantic, target)| {
            let source = caller_parameters
                .iter()
                .find(|parameter| parameter.semantic.place == semantic.place)
                .ok_or(Error::UnsupportedSourceShape { function })?;
            (semantic.access == target.access
                && semantic.place == target.place
                && semantic.path.is_empty()
                && target.path.is_empty()
                && target.root_structural_type == source.semantic.structural_type
                && target.structural_type == source.semantic.structural_type
                && target.shape == source.target.shape
                && target.source_byte_offset == 0
                && target.fixed_array_length.is_none()
                && target.element_stride.is_none()
                && target.source == source.target.placement)
                .then(|| LegalizedCallUnitArgument {
                    semantic: semantic.clone(),
                    target: target.clone(),
                })
                .ok_or(Error::UnsupportedSourceShape { function })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_callee_alpha_match(
        function,
        callee,
        &arguments,
        claim_transfers,
        caller_claims,
        target_plan,
        abstract_plan,
        unit,
    )?;
    let call = LegalizedCallUnit {
        source,
        operation: psi_operation,
        callee,
        arguments,
        claim_transfers: claim_transfers.clone(),
        fuel: optimized_call.fuel.clone(),
        effect: optimized_call.effect,
        ownership: optimized_call.ownership.clone(),
    };
    call.validate_source()
        .map_err(|_| Error::UnsupportedSourceShape { function })?;
    Ok(call)
}

#[allow(clippy::too_many_arguments)]
fn validate_callee_alpha_match(
    function: usize,
    callee: psi_core::MachineId,
    arguments: &[LegalizedCallUnitArgument],
    transfers: &[psi_terminal::ClaimTransfer],
    caller_claims: &[psi_terminal::EntryClaim],
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let target_matches = target_plan
        .functions
        .iter()
        .filter(|candidate| candidate.machine == callee)
        .collect::<Vec<_>>();
    let abstract_matches = abstract_plan
        .functions
        .iter()
        .filter(|candidate| candidate.machine == callee)
        .collect::<Vec<_>>();
    let optimized_matches = unit
        .functions
        .iter()
        .filter(|candidate| candidate.machine == callee)
        .collect::<Vec<_>>();
    let ([target_callee], [abstract_callee], [optimized_callee]) = (
        target_matches.as_slice(),
        abstract_matches.as_slice(),
        optimized_matches.as_slice(),
    ) else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let TargetOperation::UnitBody(target_body) = &target_callee.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if abstract_callee.result != omega_abstract_operations::AbstractFunctionResult::Unit
        || optimized_callee.result != abstract_callee.result
        || !abstract_callee.parameters.is_empty()
        || !optimized_callee.parameters.is_empty()
        || abstract_callee.structural_parameters != optimized_callee.structural_parameters
        || abstract_callee.entry_claims != optimized_callee.entry_claim_declarations
        || target_body.parameters.len() != abstract_callee.structural_parameters.len()
        || arguments.len() != abstract_callee.structural_parameters.len()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let expected_callee_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target_plan.target),
        &CallSignature {
            parameters: target_body
                .parameters
                .iter()
                .map(|parameter| parameter.shape)
                .collect(),
            result: None,
        },
    )
    .map_err(|_| Error::UnsupportedSourceShape { function })?;
    if target_body.call_plan != expected_callee_plan {
        return Err(Error::UnsupportedSourceShape { function });
    }
    for ((argument, semantic_parameter), target_parameter) in arguments
        .iter()
        .zip(&abstract_callee.structural_parameters)
        .zip(&target_body.parameters)
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
            return Err(Error::UnsupportedSourceShape { function });
        }
    }
    if transfers.len() != abstract_callee.entry_claims.len() {
        return Err(Error::UnsupportedSourceShape { function });
    }
    for (transfer, callee_claim) in transfers.iter().zip(&abstract_callee.entry_claims) {
        let argument_index = usize::try_from(transfer.argument_index)
            .map_err(|_| Error::UnsupportedSourceShape { function })?;
        let Some(argument) = arguments.get(argument_index) else {
            return Err(Error::UnsupportedSourceShape { function });
        };
        let Some(callee_parameter) = abstract_callee.structural_parameters.get(argument_index)
        else {
            return Err(Error::UnsupportedSourceShape { function });
        };
        let caller_claim_matches = caller_claims
            .iter()
            .filter(|claim| claim.claim == transfer.claim)
            .collect::<Vec<_>>();
        let [caller_claim] = caller_claim_matches.as_slice() else {
            return Err(Error::UnsupportedSourceShape { function });
        };
        if caller_claim.input != argument.semantic.place
            || !caller_claim.path.is_empty()
            || callee_claim.input != callee_parameter.place
            || !callee_claim.path.is_empty()
        {
            return Err(Error::UnsupportedSourceShape { function });
        }
    }
    Ok(())
}

fn derive_source_unit_function(
    function: usize,
    target: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
) -> Result<SourceUnitFunction, LegalizationError> {
    let TargetOperation::UnitBody(body) = &target.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [target_return] = body.operations.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let omega_target_operations::TargetUnitOperation::Return {
        psi_edge,
        cleanup_actions,
    } = target_return
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [abstract_entry] = abstracted.block_entries.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [abstract_return] = abstracted.operations.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [optimized_block] = optimized.blocks.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [optimized_return] = optimized_block.nodes.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || !matches!(
            abstracted.result,
            omega_abstract_operations::AbstractFunctionResult::Unit
        )
        || !abstracted.parameters.is_empty()
        || !optimized.parameters.is_empty()
        // The current Unit vocabulary carries no structural ABI or ownership
        // rows. Reject them here instead of silently projecting them away; a
        // later ProgramStorage wrapper form must retain these fields exactly.
        || !body.parameters.is_empty()
        || !abstracted.structural_parameters.is_empty()
        || !optimized.structural_parameters.is_empty()
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.declared_places.is_empty()
        || !abstracted.published_service_ceiling.is_empty()
        || !optimized.published_service_ceiling.is_empty()
        || abstracted.entry != abstract_entry.block
        || optimized.entry != abstract_entry.block
        || optimized_block.id != abstract_entry.block
        || abstract_entry.operation_offset != 0
        || !abstract_entry.parameters.is_empty()
        || !optimized_block.parameters.is_empty()
        || !cleanup_actions.is_empty()
        || abstract_return != &optimized_return.operation
        || !matches!(abstract_return, AbstractOperation::ReturnUnit { psi_edge: edge, cleanup_actions } if edge == psi_edge && cleanup_actions.is_empty())
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok(SourceUnitFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        entry_block: optimized_block.id,
        return_edge: *psi_edge,
        return_fuel: optimized_return.fuel.clone(),
    })
}

fn derive_source_function(
    function: usize,
    target: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
) -> Result<SourceFunction, LegalizationError> {
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || abstracted.block_entries.len() != 3
        || optimized.blocks.len() != 3
        || optimized.entry != abstracted.entry
        || optimized.blocks[0].id != abstracted.block_entries[0].block
        || optimized.blocks[1].id != abstracted.block_entries[1].block
        || optimized.blocks[2].id != abstracted.block_entries[2].block
        || optimized.blocks[0].nodes.len() != 1
        || abstracted
            .block_entries
            .iter()
            .any(|entry| !entry.parameters.is_empty())
        || optimized
            .blocks
            .iter()
            .any(|block| !block.parameters.is_empty())
    {
        return Err(Error::UnsupportedSourceShape { function });
    }

    let TargetOperation::ReturnIntegerConditionalControl {
        condition_source,
        condition_parameter_index,
        condition_location: ScalarParameterLocation::Register(condition_register),
        scalar_type,
        when_true,
        when_false,
    } = &target.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if scalar_type.is_address()
        || scalar_type.sign() != IntegerSign::Unsigned
        || scalar_type.bits() != 64
    {
        return Err(Error::UnsupportedIntegerShape { function });
    }
    let constant_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::Immediate { .. },
                ..
            },
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::Immediate { .. },
                ..
            }
        )
    );
    let parameter_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::Parameter { .. },
                ..
            },
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::Parameter { .. },
                ..
            }
        )
    );
    let exact_add_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::ExactAdd {
                    left,
                    right,
                    ..
                },
                ..
            },
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::ExactAdd {
                    left: false_left,
                    right: false_right,
                    ..
                },
                ..
            }
        ) if matches!(
            (left.as_ref(), right.as_ref(), false_left.as_ref(), false_right.as_ref()),
            (
                TargetIntegerExpression::Immediate { .. },
                TargetIntegerExpression::Immediate { .. },
                TargetIntegerExpression::Immediate { .. },
                TargetIntegerExpression::Immediate { .. },
            )
        )
    );
    let exact_subtract_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::ExactSubtract {
                    left,
                    right,
                    ..
                },
                ..
            },
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::ExactSubtract {
                    left: false_left,
                    right: false_right,
                    ..
                },
                ..
            }
        ) if matches!(
            (left.as_ref(), right.as_ref(), false_left.as_ref(), false_right.as_ref()),
            (
                TargetIntegerExpression::Immediate { .. },
                TargetIntegerExpression::Immediate { .. },
                TargetIntegerExpression::Immediate { .. },
                TargetIntegerExpression::Immediate { .. },
            )
        )
    );
    let u8_integer_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let widened_u8_exact_add_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::IntegerWiden {
                    source_type,
                    operand,
                    ..
                },
                ..
            },
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::IntegerWiden {
                    source_type: false_source_type,
                    operand: false_operand,
                    ..
                },
                ..
            }
        ) if *source_type == u8_integer_type
            && *false_source_type == u8_integer_type
            && matches!(
                (operand.as_ref(), false_operand.as_ref()),
                (
                    TargetIntegerExpression::ExactAdd { left, right, .. },
                    TargetIntegerExpression::ExactAdd {
                        left: false_left,
                        right: false_right,
                        ..
                    }
                ) if matches!(
                    (left.as_ref(), right.as_ref(), false_left.as_ref(), false_right.as_ref()),
                    (
                        TargetIntegerExpression::Immediate { .. },
                        TargetIntegerExpression::Immediate { .. },
                        TargetIntegerExpression::Immediate { .. },
                        TargetIntegerExpression::Immediate { .. },
                    )
                )
            )
    );
    let widened_u8_exact_subtract_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::IntegerWiden {
                    source_type,
                    operand,
                    ..
                },
                ..
            },
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::IntegerWiden {
                    source_type: false_source_type,
                    operand: false_operand,
                    ..
                },
                ..
            }
        ) if *source_type == u8_integer_type
            && *false_source_type == u8_integer_type
            && matches!(
                (operand.as_ref(), false_operand.as_ref()),
                (
                    TargetIntegerExpression::ExactSubtract { left, right, .. },
                    TargetIntegerExpression::ExactSubtract {
                        left: false_left,
                        right: false_right,
                        ..
                    }
                ) if matches!(
                    (left.as_ref(), right.as_ref(), false_left.as_ref(), false_right.as_ref()),
                    (
                        TargetIntegerExpression::Immediate { .. },
                        TargetIntegerExpression::Immediate { .. },
                        TargetIntegerExpression::Immediate { .. },
                        TargetIntegerExpression::Immediate { .. },
                    )
                )
            )
    );
    let active_resident_chain = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TargetIntegerControl::Return { expression, .. },
            TargetIntegerControl::Return {
                expression: TargetIntegerExpression::Immediate { .. },
                ..
            }
        ) if is_active_resident_exact_add_chain(expression)
    );
    let expected_offsets = if constant_leaves {
        [0, 1, 3]
    } else if parameter_leaves {
        [0, 1, 2]
    } else if exact_add_leaves || exact_subtract_leaves {
        [0, 1, 5]
    } else if widened_u8_exact_add_leaves || widened_u8_exact_subtract_leaves {
        [0, 1, 6]
    } else if active_resident_chain {
        [0, 1, 8]
    } else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let (expected_operation_count, expected_leaf_node_counts) = if constant_leaves {
        (5, [2, 2])
    } else if parameter_leaves {
        (3, [1, 1])
    } else if widened_u8_exact_add_leaves || widened_u8_exact_subtract_leaves {
        (11, [5, 5])
    } else if active_resident_chain {
        (10, [7, 2])
    } else {
        (9, [4, 4])
    };
    let expected_parameter_count = if parameter_leaves { 2 } else { 1 };
    if abstracted.operations.len() != expected_operation_count
        || abstracted.parameters.len() != expected_parameter_count
        || optimized.parameters.len() != expected_parameter_count
        || abstracted
            .block_entries
            .iter()
            .zip(expected_offsets)
            .any(|(entry, offset)| entry.operation_offset != offset)
        || optimized.blocks[1].nodes.len() != expected_leaf_node_counts[0]
        || optimized.blocks[2].nodes.len() != expected_leaf_node_counts[1]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let Some(parameter) = optimized.parameters.get(*condition_parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    let Some(abstract_parameter) = abstracted.parameters.get(*condition_parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    if parameter.value != *condition_source
        || parameter.scalar_type != ScalarType::Boolean
        || abstract_parameter.value != *condition_source
        || abstract_parameter.scalar_type != ScalarType::Boolean
    {
        return Err(Error::UnsupportedCondition { function });
    }

    let entry_node = &optimized.blocks[0].nodes[0];
    if entry_node.operation != abstracted.operations[0] {
        return Err(Error::SourceCustodyMismatch);
    }
    let AbstractOperation::Conditional {
        condition,
        when_true: abstract_true,
        when_false: abstract_false,
    } = &entry_node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *condition != *condition_source
        || abstract_true.psi_edge != when_true.psi_edge
        || abstract_false.psi_edge != when_false.psi_edge
        || abstract_true.target != optimized.blocks[1].id
        || abstract_false.target != optimized.blocks[2].id
        || !abstract_true.bindings.is_empty()
        || !abstract_false.bindings.is_empty()
        || entry_node.successors.len() != 2
        || entry_node.successors[0].psi_edge != abstract_true.psi_edge
        || entry_node.successors[0].target != abstract_true.target
        || !entry_node.successors[0].bindings.is_empty()
        || entry_node.successors[1].psi_edge != abstract_false.psi_edge
        || entry_node.successors[1].target != abstract_false.target
        || !entry_node.successors[1].bindings.is_empty()
        || !entry_node.provenance.is_empty()
        || !entry_node.fuel.is_empty()
        || entry_node.successors[0].provenance != vec![PsiProvenance::Edge(abstract_true.psi_edge)]
        || entry_node.successors[1].provenance != vec![PsiProvenance::Edge(abstract_false.psi_edge)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let branch_true_fuel = exact_edge_fuel(entry_node, abstract_true.psi_edge, function)?;
    let branch_false_fuel = exact_edge_fuel(entry_node, abstract_false.psi_edge, function)?;
    if entry_node.successors[0].fuel.len() != branch_true_fuel.len()
        || entry_node.successors[1].fuel.len() != branch_false_fuel.len()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }

    let when_true = derive_leaf(
        function,
        when_true.psi_edge,
        when_true.control.as_ref(),
        &abstracted.operations[expected_offsets[1]..expected_offsets[2]],
        &optimized.blocks[1].nodes,
        abstracted,
        optimized,
        accepted_obligation_facts,
        [LegalizedTemporaryId(0), LegalizedTemporaryId(1)],
    )?;
    let when_false = derive_leaf(
        function,
        when_false.psi_edge,
        when_false.control.as_ref(),
        &abstracted.operations[expected_offsets[2]..],
        &optimized.blocks[2].nodes,
        abstracted,
        optimized,
        accepted_obligation_facts,
        [LegalizedTemporaryId(2), LegalizedTemporaryId(3)],
    )?;
    if let (
        SourceLeafValue::EntryParameter {
            parameter_index: true_index,
            register: true_register,
            ..
        },
        SourceLeafValue::EntryParameter {
            parameter_index: false_index,
            register: false_register,
            ..
        },
    ) = (&when_true.value, &when_false.value)
        && (when_true.source_value != when_false.source_value
            || true_index != false_index
            || true_register != false_register
            || *true_index == *condition_parameter_index)
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let expected_provenance = TerminalPsiProvenance {
        operations: source_operations(&when_true.value)
            .into_iter()
            .chain(source_operations(&when_false.value))
            .collect(),
        edges: vec![
            abstract_true.psi_edge,
            abstract_false.psi_edge,
            when_true.return_edge,
            when_false.return_edge,
        ],
    };
    if target.provenance != expected_provenance {
        return Err(Error::SourceCustodyMismatch);
    }

    Ok(SourceFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        recipe: if constant_leaves {
            LegalizationRecipe::ReturnU64ImmediateConditionalV1
        } else if parameter_leaves {
            LegalizationRecipe::ReturnU64EntryParameterConditionalV1
        } else if exact_add_leaves {
            LegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1
        } else if widened_u8_exact_add_leaves {
            LegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1
        } else if widened_u8_exact_subtract_leaves {
            LegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1
        } else if active_resident_chain {
            LegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1
        } else {
            LegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1
        },
        condition_source: *condition_source,
        condition_parameter_index: *condition_parameter_index,
        condition_register: *condition_register,
        condition_definition_site: parameter.site,
        entry_block: optimized.blocks[0].id,
        true_block: optimized.blocks[1].id,
        false_block: optimized.blocks[2].id,
        branch_true_edge: abstract_true.psi_edge,
        branch_false_edge: abstract_false.psi_edge,
        branch_true_fuel,
        branch_false_fuel,
        branch_true_bindings: abstract_true.bindings.clone(),
        branch_false_bindings: abstract_false.bindings.clone(),
        when_true,
        when_false,
    })
}

#[allow(clippy::too_many_arguments)]
fn derive_leaf(
    function: usize,
    arm_edge: EdgeId,
    target: &TargetIntegerControl,
    abstract_operations: &[AbstractOperation],
    nodes: &[omega_optimization_unit::OptimizationNode],
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
    temporaries: [LegalizedTemporaryId; 2],
) -> Result<SourceLeaf, LegalizationError> {
    if nodes.len() != abstract_operations.len()
        || nodes
            .iter()
            .zip(abstract_operations)
            .any(|(node, operation)| node.operation != *operation)
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let TargetIntegerControl::Return {
        psi_return_edge,
        source_value,
        expression,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let u64_integer_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u64_type = ScalarType::Integer(u64_integer_type);
    let (return_node, value) = match expression {
        TargetIntegerExpression::Immediate {
            source_value: expression_source,
            value: target_value,
        } => {
            if nodes.len() != 2 || source_value != expression_source {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } = &nodes[0].operation
            else {
                return Err(Error::MissingConstantDefinition { function, arm_edge });
            };
            if *result != *source_value
                || *value != *target_value
                || *scalar_type != u64_type
                || nodes[0].definitions.len() != 1
                || nodes[0].definitions[0].value != *source_value
                || nodes[0].provenance != vec![PsiProvenance::Operation(*psi_operation)]
            {
                return Err(Error::MissingConstantDefinition { function, arm_edge });
            }
            let constant_fuel = exact_operation_fuel(&nodes[0], *psi_operation, function)?;
            (
                &nodes[1],
                SourceLeafValue::Immediate {
                    value: *value,
                    constant_operation: *psi_operation,
                    definition_site: nodes[0].definitions[0].site,
                    constant_fuel,
                },
            )
        }
        TargetIntegerExpression::Parameter {
            source_value: expression_source,
            parameter_index,
            location: ScalarParameterLocation::Register(register),
        } => {
            if nodes.len() != 1 || source_value != expression_source {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let Some(parameter) = optimized.parameters.get(*parameter_index) else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            let Some(abstract_parameter) = abstracted.parameters.get(*parameter_index) else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            if parameter.value != *source_value
                || parameter.scalar_type != u64_type
                || abstract_parameter.value != *source_value
                || abstract_parameter.scalar_type != u64_type
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            (
                &nodes[0],
                SourceLeafValue::EntryParameter {
                    parameter_index: *parameter_index,
                    register: *register,
                    definition_site: parameter.site,
                },
            )
        }
        TargetIntegerExpression::IntegerWiden {
            psi_operation: widen_operation,
            source_type,
            operand,
        } => {
            let u8_integer_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
            let u8_type = ScalarType::Integer(u8_integer_type);
            let (is_subtract, arithmetic_operation, obligation, left, right) =
                match operand.as_ref() {
                    TargetIntegerExpression::ExactAdd {
                        psi_operation,
                        obligation,
                        left,
                        right,
                    } => (false, psi_operation, obligation, left, right),
                    TargetIntegerExpression::ExactSubtract {
                        psi_operation,
                        obligation,
                        left,
                        right,
                    } => (true, psi_operation, obligation, left, right),
                    _ => return Err(Error::UnsupportedSourceShape { function }),
                };
            if nodes.len() != 5 || *source_type != u8_integer_type {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let left = derive_immediate(function, arm_edge, left, &nodes[0], u8_type)?;
            let right = derive_immediate(function, arm_edge, right, &nodes[1], u8_type)?;
            let (
                abstract_arithmetic_operation,
                abstract_obligation,
                narrow_result,
                arithmetic_type,
                abstract_left,
                abstract_right,
            ) = match (&nodes[2].operation, is_subtract) {
                (
                    AbstractOperation::ExactIntegerAdd {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    false,
                )
                | (
                    AbstractOperation::ExactIntegerSubtract {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    true,
                ) => (psi_operation, obligation, result, scalar_type, left, right),
                _ => return Err(Error::UnsupportedSourceShape { function }),
            };
            if abstract_arithmetic_operation != arithmetic_operation
                || abstract_obligation != obligation
                || *arithmetic_type != u8_integer_type
                || *abstract_left != left.source_value
                || *abstract_right != right.source_value
                || nodes[2].definitions.len() != 1
                || nodes[2].definitions[0].value != *narrow_result
                || nodes[2].provenance != vec![PsiProvenance::Operation(*arithmetic_operation)]
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let AbstractOperation::IntegerWiden {
                psi_operation: abstract_widen_operation,
                result: widened_result,
                source_type: abstract_source_type,
                target_type: abstract_target_type,
                operand: abstract_operand,
            } = &nodes[3].operation
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            if abstract_widen_operation != widen_operation
                || *widened_result != *source_value
                || *narrow_result == *source_value
                || *abstract_source_type != u8_integer_type
                || *abstract_target_type != u64_integer_type
                || *abstract_operand != *narrow_result
                || nodes[3].definitions.len() != 1
                || nodes[3].definitions[0].value != *source_value
                || nodes[3].provenance != vec![PsiProvenance::Operation(*widen_operation)]
            {
                return Err(Error::UnsupportedSourceShape { function });
            }

            let narrow_value = if is_subtract {
                u8_integer_type.exact_sub(left.value, right.value)
            } else {
                u8_integer_type.exact_add(left.value, right.value)
            };
            let Some(narrow_value) = narrow_value else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            let Some(widened_value) =
                u8_integer_type.widen_value_to(u64_integer_type, narrow_value)
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            let Some(widened_left) = u8_integer_type.widen_value_to(u64_integer_type, left.value)
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            let Some(widened_right) = u8_integer_type.widen_value_to(u64_integer_type, right.value)
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            let recomputed_widened = if is_subtract {
                u64_integer_type.exact_sub(widened_left, widened_right)
            } else {
                u64_integer_type.exact_add(widened_left, widened_right)
            };
            if recomputed_widened != Some(widened_value) {
                return Err(Error::UnsupportedSourceShape { function });
            }

            let Some(accepted_fact) = accepted_obligation_facts.iter().find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *arithmetic_operation
                    && fact.obligation == *obligation
            }) else {
                return Err(Error::SourceCustodyMismatch);
            };
            if !optimized.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference {
                        obligation: referenced_obligation,
                        support,
                    } if *referenced_obligation == *obligation
                        && *support == *arithmetic_operation
                )
            }) {
                return Err(Error::SourceCustodyMismatch);
            }
            let arithmetic_fuel = exact_operation_fuel(&nodes[2], *arithmetic_operation, function)?;
            let widen_fuel = exact_operation_fuel(&nodes[3], *widen_operation, function)?;
            let value = if is_subtract {
                SourceLeafValue::WidenedExactSubtract {
                    source_type: u8_integer_type,
                    target_type: u64_integer_type,
                    theorem: LegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1,
                    obligation: *obligation,
                    accepted_fact: accepted_fact.identity,
                    subtract_operation: *arithmetic_operation,
                    narrow_result: *narrow_result,
                    subtract_definition_site: nodes[2].definitions[0].site,
                    subtract_fuel: arithmetic_fuel,
                    widen_operation: *widen_operation,
                    widen_definition_site: nodes[3].definitions[0].site,
                    widen_fuel,
                    left_temporary: temporaries[0],
                    right_temporary: temporaries[1],
                    left,
                    right,
                }
            } else {
                SourceLeafValue::WidenedExactAdd {
                    source_type: u8_integer_type,
                    target_type: u64_integer_type,
                    theorem: LegalizationTheorem::UnsignedExactAddCommutesWithWidenV1,
                    obligation: *obligation,
                    accepted_fact: accepted_fact.identity,
                    add_operation: *arithmetic_operation,
                    narrow_result: *narrow_result,
                    add_definition_site: nodes[2].definitions[0].site,
                    add_fuel: arithmetic_fuel,
                    widen_operation: *widen_operation,
                    widen_definition_site: nodes[3].definitions[0].site,
                    widen_fuel,
                    left_temporary: temporaries[0],
                    right_temporary: temporaries[1],
                    left,
                    right,
                }
            };
            (&nodes[4], value)
        }
        expression @ TargetIntegerExpression::ExactAdd {
            psi_operation,
            obligation,
            left,
            right,
        } if !is_active_resident_exact_add_chain(expression) => {
            if nodes.len() != 4 {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let left = derive_immediate(function, arm_edge, left, &nodes[0], u64_type)?;
            let right = derive_immediate(function, arm_edge, right, &nodes[1], u64_type)?;
            let AbstractOperation::ExactIntegerAdd {
                psi_operation: abstract_operation,
                obligation: abstract_obligation,
                result,
                scalar_type,
                left: abstract_left,
                right: abstract_right,
            } = &nodes[2].operation
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            if abstract_operation != psi_operation
                || abstract_obligation != obligation
                || *result != *source_value
                || *scalar_type != u64_integer_type
                || *abstract_left != left.source_value
                || *abstract_right != right.source_value
                || nodes[2].definitions.len() != 1
                || nodes[2].definitions[0].value != *source_value
                || nodes[2].provenance != vec![PsiProvenance::Operation(*psi_operation)]
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let add_fuel = exact_operation_fuel(&nodes[2], *psi_operation, function)?;
            let Some(accepted_fact) = accepted_obligation_facts.iter().find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            }) else {
                return Err(Error::SourceCustodyMismatch);
            };
            if !optimized.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference {
                        obligation: referenced_obligation,
                        support,
                    } if *referenced_obligation == *obligation && *support == *psi_operation
                )
            }) {
                return Err(Error::SourceCustodyMismatch);
            }
            (
                &nodes[3],
                SourceLeafValue::ExactAdd {
                    obligation: *obligation,
                    accepted_fact: accepted_fact.identity,
                    add_operation: *psi_operation,
                    definition_site: nodes[2].definitions[0].site,
                    add_fuel,
                    left,
                    right,
                },
            )
        }
        expression if is_active_resident_exact_add_chain(expression) => {
            if nodes.len() != 7 {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let TargetIntegerExpression::ExactAdd {
                psi_operation: result_operation,
                obligation: result_obligation,
                left: result_left,
                right: result_right,
            } = expression
            else {
                unreachable!("shape predicate admitted only exact addition")
            };
            let TargetIntegerExpression::ExactAdd {
                psi_operation: middle_operation,
                obligation: middle_obligation,
                left: middle_left,
                right: middle_right,
            } = result_right.as_ref()
            else {
                unreachable!("shape predicate admitted the middle addition")
            };
            let TargetIntegerExpression::ExactAdd {
                psi_operation: inner_operation,
                obligation: inner_obligation,
                left: inner_left,
                right: inner_right,
            } = middle_right.as_ref()
            else {
                unreachable!("shape predicate admitted the inner addition")
            };
            let resident = derive_immediate(function, arm_edge, result_left, &nodes[0], u64_type)?;
            let second_resident =
                derive_immediate(function, arm_edge, middle_left, &nodes[0], u64_type)?;
            if resident != second_resident {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let left = derive_immediate(function, arm_edge, inner_left, &nodes[1], u64_type)?;
            let right = derive_immediate(function, arm_edge, inner_right, &nodes[2], u64_type)?;
            let inner = derive_exact_add(
                function,
                optimized,
                accepted_obligation_facts,
                &nodes[3],
                *inner_operation,
                *inner_obligation,
                left.source_value,
                right.source_value,
                u64_integer_type,
            )?;
            let middle = derive_exact_add(
                function,
                optimized,
                accepted_obligation_facts,
                &nodes[4],
                *middle_operation,
                *middle_obligation,
                resident.source_value,
                inner.source_value,
                u64_integer_type,
            )?;
            let result = derive_exact_add(
                function,
                optimized,
                accepted_obligation_facts,
                &nodes[5],
                *result_operation,
                *result_obligation,
                resident.source_value,
                middle.source_value,
                u64_integer_type,
            )?;
            if result.source_value != *source_value {
                return Err(Error::UnsupportedSourceShape { function });
            }
            (
                &nodes[6],
                SourceLeafValue::ActiveResidentExactAddChain(Box::new(
                    SourceActiveResidentExactAddChain {
                        resident,
                        left,
                        right,
                        inner,
                        middle,
                        result,
                    },
                )),
            )
        }
        TargetIntegerExpression::ExactSubtract {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            if nodes.len() != 4 {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let left = derive_immediate(function, arm_edge, left, &nodes[0], u64_type)?;
            let right = derive_immediate(function, arm_edge, right, &nodes[1], u64_type)?;
            let AbstractOperation::ExactIntegerSubtract {
                psi_operation: abstract_operation,
                obligation: abstract_obligation,
                result,
                scalar_type,
                left: abstract_left,
                right: abstract_right,
            } = &nodes[2].operation
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            if abstract_operation != psi_operation
                || abstract_obligation != obligation
                || *result != *source_value
                || *scalar_type != u64_integer_type
                || *abstract_left != left.source_value
                || *abstract_right != right.source_value
                || nodes[2].definitions.len() != 1
                || nodes[2].definitions[0].value != *source_value
                || nodes[2].provenance != vec![PsiProvenance::Operation(*psi_operation)]
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let subtract_fuel = exact_operation_fuel(&nodes[2], *psi_operation, function)?;
            let Some(accepted_fact) = accepted_obligation_facts.iter().find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            }) else {
                return Err(Error::SourceCustodyMismatch);
            };
            if !optimized.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference {
                        obligation: referenced_obligation,
                        support,
                    } if *referenced_obligation == *obligation && *support == *psi_operation
                )
            }) {
                return Err(Error::SourceCustodyMismatch);
            }
            (
                &nodes[3],
                SourceLeafValue::ExactSubtract {
                    obligation: *obligation,
                    accepted_fact: accepted_fact.identity,
                    subtract_operation: *psi_operation,
                    definition_site: nodes[2].definitions[0].site,
                    subtract_fuel,
                    left,
                    right,
                },
            )
        }
        _ => return Err(Error::UnsupportedSourceShape { function }),
    };
    let AbstractOperation::Return {
        psi_edge,
        value: returned_value,
        scalar_type: returned_type,
        cleanup_actions,
        ..
    } = &return_node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *psi_edge != *psi_return_edge
        || *returned_value != *source_value
        || *returned_type != u64_type
        || !cleanup_actions.is_empty()
        || return_node.provenance != vec![PsiProvenance::Edge(*psi_return_edge)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let return_fuel = exact_edge_fuel(return_node, *psi_return_edge, function)?;
    if return_node.fuel.len() != return_fuel.len() {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok(SourceLeaf {
        return_edge: *psi_return_edge,
        source_value: *source_value,
        return_fuel,
        value,
    })
}

fn source_operations(value: &SourceLeafValue) -> Vec<OperationId> {
    match value {
        SourceLeafValue::Immediate {
            constant_operation, ..
        } => vec![*constant_operation],
        SourceLeafValue::EntryParameter { .. } => Vec::new(),
        SourceLeafValue::ExactAdd {
            add_operation,
            left,
            right,
            ..
        } => vec![
            left.constant_operation,
            right.constant_operation,
            *add_operation,
        ],
        SourceLeafValue::ExactSubtract {
            subtract_operation,
            left,
            right,
            ..
        } => vec![
            left.constant_operation,
            right.constant_operation,
            *subtract_operation,
        ],
        SourceLeafValue::WidenedExactAdd {
            add_operation,
            widen_operation,
            left,
            right,
            ..
        } => vec![
            left.constant_operation,
            right.constant_operation,
            *add_operation,
            *widen_operation,
        ],
        SourceLeafValue::WidenedExactSubtract {
            subtract_operation,
            widen_operation,
            left,
            right,
            ..
        } => vec![
            left.constant_operation,
            right.constant_operation,
            *subtract_operation,
            *widen_operation,
        ],
        SourceLeafValue::ActiveResidentExactAddChain(chain) => vec![
            chain.resident.constant_operation,
            chain.left.constant_operation,
            chain.right.constant_operation,
            chain.inner.operation,
            chain.middle.operation,
            chain.result.operation,
        ],
    }
}

fn is_active_resident_exact_add_chain(expression: &TargetIntegerExpression) -> bool {
    let TargetIntegerExpression::ExactAdd {
        left: result_left,
        right: result_right,
        ..
    } = expression
    else {
        return false;
    };
    let TargetIntegerExpression::Immediate {
        source_value: result_resident,
        ..
    } = result_left.as_ref()
    else {
        return false;
    };
    let TargetIntegerExpression::ExactAdd {
        left: middle_left,
        right: middle_right,
        ..
    } = result_right.as_ref()
    else {
        return false;
    };
    let TargetIntegerExpression::Immediate {
        source_value: middle_resident,
        ..
    } = middle_left.as_ref()
    else {
        return false;
    };
    matches!(
        middle_right.as_ref(),
        TargetIntegerExpression::ExactAdd { left, right, .. }
            if matches!(left.as_ref(), TargetIntegerExpression::Immediate { .. })
                && matches!(right.as_ref(), TargetIntegerExpression::Immediate { .. })
                && result_resident == middle_resident
    )
}

#[allow(clippy::too_many_arguments)]
fn derive_exact_add(
    function: usize,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
    node: &omega_optimization_unit::OptimizationNode,
    operation: OperationId,
    obligation: psi_core::ObligationId,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
    scalar_type: psi_core::IntegerType,
) -> Result<SourceExactAdd, LegalizationError> {
    let AbstractOperation::ExactIntegerAdd {
        psi_operation,
        obligation: abstract_obligation,
        result,
        scalar_type: abstract_type,
        left: abstract_left,
        right: abstract_right,
    } = &node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *psi_operation != operation
        || *abstract_obligation != obligation
        || *abstract_type != scalar_type
        || *abstract_left != left
        || *abstract_right != right
        || node.definitions.len() != 1
        || node.definitions[0].value != *result
        || node.provenance != vec![PsiProvenance::Operation(operation)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let Some(accepted_fact) = accepted_obligation_facts.iter().find(|fact| {
        fact.machine == optimized.machine
            && fact.operation == operation
            && fact.obligation == obligation
    }) else {
        return Err(Error::SourceCustodyMismatch);
    };
    if !optimized.facts.iter().any(|fact| {
        matches!(
            fact,
            OptimizationFact::OperationObligationReference {
                obligation: referenced_obligation,
                support,
            } if *referenced_obligation == obligation && *support == operation
        )
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(SourceExactAdd {
        source_value: *result,
        obligation,
        accepted_fact: accepted_fact.identity,
        operation,
        definition_site: node.definitions[0].site,
        fuel: exact_operation_fuel(node, operation, function)?,
    })
}

fn derive_immediate(
    function: usize,
    arm_edge: EdgeId,
    target: &TargetIntegerExpression,
    node: &omega_optimization_unit::OptimizationNode,
    expected_type: ScalarType,
) -> Result<SourceImmediate, LegalizationError> {
    let TargetIntegerExpression::Immediate {
        source_value,
        value: target_value,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let AbstractOperation::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value,
    } = &node.operation
    else {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    };
    if result != source_value
        || value != target_value
        || *scalar_type != expected_type
        || node.definitions.len() != 1
        || node.definitions[0].value != *source_value
        || node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
    {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    }
    Ok(SourceImmediate {
        source_value: *source_value,
        value: *value,
        constant_operation: *psi_operation,
        definition_site: node.definitions[0].site,
        fuel: exact_operation_fuel(node, *psi_operation, function)?,
    })
}

fn exact_edge_fuel(
    node: &omega_optimization_unit::OptimizationNode,
    edge: EdgeId,
    function: usize,
) -> Result<Vec<FuelSettlement>, LegalizationError> {
    let custody = node
        .successors
        .iter()
        .find(|successor| successor.psi_edge == edge)
        .map_or(node.fuel.as_slice(), |successor| successor.fuel.as_slice());
    let fuel = custody
        .iter()
        .copied()
        .filter(|settlement| settlement.site == PsiProvenance::Edge(edge))
        .collect::<Vec<_>>();
    if fuel.is_empty() || fuel.len() != custody.len() {
        return Err(Error::MissingFuelProvenance { function });
    }
    Ok(fuel)
}

fn exact_operation_fuel(
    node: &omega_optimization_unit::OptimizationNode,
    operation: OperationId,
    function: usize,
) -> Result<Vec<FuelSettlement>, LegalizationError> {
    let fuel = node
        .fuel
        .iter()
        .copied()
        .filter(|settlement| settlement.site == PsiProvenance::Operation(operation))
        .collect::<Vec<_>>();
    if fuel.is_empty() || fuel.len() != node.fuel.len() {
        return Err(Error::MissingFuelProvenance { function });
    }
    Ok(fuel)
}
