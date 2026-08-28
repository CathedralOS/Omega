use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use omega_optimization_unit::{
    OptimizationFact, OwnershipEvent, PsiOptimizationUnit, PsiProvenance,
};
use omega_terminal_abstract_operations::{
    TerminalAbstractOperation, TerminalAbstractOperationPlan,
};
use omega_terminal_legalized_operations::{
    TerminalLegalizationRecipe, TerminalLegalizationTheorem, TerminalLegalizedFunction,
    TerminalLegalizedImmediate, TerminalLegalizedLeaf, TerminalLegalizedLeafValue,
    TerminalLegalizedOperationPlan, TerminalLegalizedStructuralUnitFunction,
    TerminalLegalizedTemporaryId, TerminalLegalizedUnitFunction,
};
use omega_terminal_target_operations::{
    TerminalPsiProvenance, TerminalScalarParameterLocation, TerminalTargetIntegerControl,
    TerminalTargetIntegerExpression, TerminalTargetOperation, TerminalTargetOperationPlan,
    TerminalTargetUnitOperation,
};
use psi_core::{EdgeId, IntegerSign, OperationId, ScalarType, StructuralPlaceKind};
use psi_terminal::StructuralPlaceDeclaration;

use crate::{TerminalLegalizationError, TerminalLegalizationError as Error};

/// Independently replay a proposed V7 legal projection against all three raw
/// custody inputs. This module deliberately compares fields in place instead
/// of constructing a second plan with the producer's derivation strategy.
pub(crate) fn replay_terminal_legalized_plan(
    target: &TerminalTargetOperationPlan,
    abstract_plan: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &TerminalLegalizedOperationPlan,
) -> Result<usize, TerminalLegalizationError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
        || target.terminal_psi != abstract_plan.terminal_psi
        || target.terminal_psi != unit.terminal_psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
    {
        return Err(Error::SourceCustodyMismatch);
    }
    if proposed.terminal_psi != target.terminal_psi
        || proposed.optimization_unit != unit.identity
        || proposed.fuel_schedule != unit.fuel_schedule
        || proposed.target != target.target
        || proposed.entry != target.entry
        || proposed.functions.len()
            + proposed.unit_functions.len()
            + proposed.structural_unit_functions.len()
            != target.functions.len()
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let mut decomposition_count = 0usize;
    for (index, target_function) in target.functions.iter().enumerate() {
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
            return Err(Error::SourceCustodyMismatch);
        };
        let count = if matches!(
            target_function.operation,
            TerminalTargetOperation::UnitBody(_)
        ) {
            let plain = proposed
                .unit_functions
                .iter()
                .filter(|candidate| candidate.machine == target_function.machine)
                .collect::<Vec<_>>();
            let structural = proposed
                .structural_unit_functions
                .iter()
                .filter(|candidate| candidate.machine == target_function.machine)
                .collect::<Vec<_>>();
            match (plain.as_slice(), structural.as_slice()) {
                ([legalized], []) => {
                    replay_unit_function(index, target_function, abstracted, optimized, legalized)?
                }
                ([], [legalized]) => replay_structural_unit_function(
                    index,
                    target_function,
                    abstracted,
                    optimized,
                    legalized,
                    target,
                    abstract_plan,
                    unit,
                )?,
                _ => return Err(Error::NonCanonicalLegalizedPlan),
            }
        } else {
            let mut matches = proposed
                .functions
                .iter()
                .filter(|candidate| candidate.machine == target_function.machine);
            let legalized = matches.next().ok_or(Error::NonCanonicalLegalizedPlan)?;
            if matches.next().is_some() {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
            replay_function(
                index,
                target.target.architecture,
                target_function,
                abstracted,
                optimized,
                &unit.accepted_obligation_facts,
                legalized,
            )?
        };
        decomposition_count = decomposition_count
            .checked_add(count)
            .ok_or(Error::NonCanonicalLegalizedPlan)?;
    }
    Ok(decomposition_count)
}

#[allow(clippy::too_many_arguments)]
fn replay_structural_unit_function(
    function: usize,
    target: &omega_terminal_target_operations::TerminalTargetFunction,
    abstracted: &omega_terminal_abstract_operations::TerminalAbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    proposed: &TerminalLegalizedStructuralUnitFunction,
    target_plan: &TerminalTargetOperationPlan,
    abstract_plan: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<usize, TerminalLegalizationError> {
    let TerminalTargetOperation::UnitBody(body) = &target.operation else {
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
    ) = match (
        body.operations.as_slice(),
        abstracted.operations.as_slice(),
        optimized_block.nodes.as_slice(),
    ) {
        (
            [target_return @ TerminalTargetUnitOperation::Return { .. }],
            [abstract_return @ TerminalAbstractOperation::ReturnUnit { .. }],
            [optimized_return],
        ) => (
            None,
            target_return,
            None,
            abstract_return,
            None,
            optimized_return,
        ),
        (
            [
                target_call @ TerminalTargetUnitOperation::Call { .. },
                target_return @ TerminalTargetUnitOperation::Return { .. },
            ],
            [
                abstract_call @ TerminalAbstractOperation::CallUnit { .. },
                abstract_return @ TerminalAbstractOperation::ReturnUnit { .. },
            ],
            [optimized_call, optimized_return],
        ) => (
            Some(target_call),
            target_return,
            Some(abstract_call),
            abstract_return,
            Some(optimized_call),
            optimized_return,
        ),
        (
            [
                target_call @ TerminalTargetUnitOperation::InstalledProviderCall { .. },
                target_return @ TerminalTargetUnitOperation::Return { .. },
            ],
            [
                abstract_call @ TerminalAbstractOperation::BoundaryCall { .. },
                abstract_return @ TerminalAbstractOperation::ReturnUnit { .. },
            ],
            [optimized_call, optimized_return],
        ) => (
            Some(target_call),
            target_return,
            Some(abstract_call),
            abstract_return,
            Some(optimized_call),
            optimized_return,
        ),
        _ => return Err(Error::UnsupportedSourceShape { function }),
    };
    let TerminalTargetUnitOperation::Return {
        psi_edge,
        cleanup_actions,
    } = target_return
    else {
        unreachable!()
    };
    let expected_provenance = TerminalPsiProvenance {
        operations: abstract_call
            .and_then(|operation| match operation {
                TerminalAbstractOperation::CallUnit { psi_operation, .. } => Some(*psi_operation),
                TerminalAbstractOperation::BoundaryCall { psi_operation, .. } => {
                    Some(*psi_operation)
                }
                _ => None,
            })
            .into_iter()
            .collect(),
        edges: vec![*psi_edge],
    };
    let expected_return_effect_input = u64::from(abstract_call.is_some());
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
        || abstracted.result
            != omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Unit
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
        || !matches!(abstract_return, TerminalAbstractOperation::ReturnUnit { psi_edge: edge, cleanup_actions } if edge == psi_edge && cleanup_actions.is_empty())
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
fn replay_structural_call(
    function: usize,
    target_call: &TerminalTargetUnitOperation,
    abstract_call: &TerminalAbstractOperation,
    optimized_call: &omega_optimization_unit::OptimizationNode,
    proposed: &omega_terminal_legalized_operations::TerminalLegalizedCallUnit,
    caller_parameters: &[omega_terminal_legalized_operations::TerminalLegalizedCallUnitParameter],
    caller_claims: &[psi_terminal::EntryClaim],
    target_plan: &TerminalTargetOperationPlan,
    abstract_plan: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), TerminalLegalizationError> {
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
            TerminalTargetUnitOperation::Call {
                psi_operation: target_operation,
                callee: target_callee,
                arguments: target_arguments,
                claim_transfers: target_transfers,
            },
            TerminalAbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                claim_transfers,
            },
        ) if target_operation == psi_operation
            && target_callee == callee
            && target_transfers == claim_transfers => (
                *psi_operation,
                *callee,
                structural_arguments,
                target_arguments,
                claim_transfers,
                omega_terminal_legalized_operations::TerminalLegalizedCallUnitSource::AuthoredCallUnit,
                OwnershipEvent::ClaimTransfer(
                    claim_transfers.iter().map(|transfer| transfer.claim).collect(),
                ),
            ),
        (
            TerminalTargetUnitOperation::InstalledProviderCall {
                psi_operation: target_operation,
                boundary: target_boundary,
                provider,
                source_arguments,
                arguments: target_arguments,
                claim_transfers: target_transfers,
                completion_claim_sources: target_sources,
                completion_receipts: target_receipts,
            },
            TerminalAbstractOperation::BoundaryCall {
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
                .any(|candidate| candidate == provider) => (
                *psi_operation,
                provider.candidate,
                structural_arguments,
                target_arguments,
                target_transfers,
                omega_terminal_legalized_operations::TerminalLegalizedCallUnitSource::InstalledProvider {
                    boundary: *boundary,
                    provider: provider.clone(),
                    completion_claim_sources: completion_claim_sources.clone(),
                    completion_receipts: completion_receipts.clone(),
                },
                OwnershipEvent::ClaimCompletion(
                    completion_receipts.iter().map(|receipt| receipt.claim).collect(),
                ),
            ),
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
    let TerminalTargetOperation::UnitBody(callee_body) = &target_callee.operation else {
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
    if abstract_callee.result
        != omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Unit
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

fn replay_unit_function(
    function: usize,
    target: &omega_terminal_target_operations::TerminalTargetFunction,
    abstracted: &omega_terminal_abstract_operations::TerminalAbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    proposed: &TerminalLegalizedUnitFunction,
) -> Result<usize, TerminalLegalizationError> {
    let TerminalTargetOperation::UnitBody(body) = &target.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [target_return] = body.operations.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let omega_terminal_target_operations::TerminalTargetUnitOperation::Return {
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
            omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Unit
        )
        || !abstracted.parameters.is_empty()
        || !optimized.parameters.is_empty()
        // This closed Unit record has no structural ABI or ownership
        // vocabulary. Independent replay must fail rather than validate a
        // proposal that erased those source declarations.
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
        || !matches!(abstract_return, TerminalAbstractOperation::ReturnUnit { psi_edge: edge, cleanup_actions } if edge == psi_edge && cleanup_actions.is_empty())
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    if proposed.machine != target.machine
        || proposed.attachment != target.attachment
        || proposed.provenance != target.provenance
        || proposed.entry_block != optimized_block.id
        || proposed.return_edge != *psi_edge
        || proposed.return_fuel != optimized_return.fuel
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(0)
}

#[allow(clippy::too_many_arguments)]
fn replay_function(
    function: usize,
    architecture: omega_target::Architecture,
    target: &omega_terminal_target_operations::TerminalTargetFunction,
    abstracted: &omega_terminal_abstract_operations::TerminalAbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed: &TerminalLegalizedFunction,
) -> Result<usize, TerminalLegalizationError> {
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
    if proposed.machine != target.machine
        || proposed.attachment != target.attachment
        || proposed.entry_block != optimized.blocks[0].id
        || proposed.true_block != optimized.blocks[1].id
        || proposed.false_block != optimized.blocks[2].id
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let TerminalTargetOperation::ReturnIntegerConditionalControl {
        condition_source,
        condition_parameter_index,
        condition_location: TerminalScalarParameterLocation::Register(condition_register),
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
    if condition_register.architecture() != architecture {
        return Err(Error::SourceCustodyMismatch);
    }

    let recipe_matches_target = match proposed.recipe {
        TerminalLegalizationRecipe::ReturnU64ImmediateConditionalV1 => {
            [when_true, when_false].iter().all(|arm| {
                matches!(
                    arm.control.as_ref(),
                    TerminalTargetIntegerControl::Return {
                        expression: TerminalTargetIntegerExpression::Immediate { .. },
                        ..
                    }
                )
            })
        }
        TerminalLegalizationRecipe::ReturnU64EntryParameterConditionalV1 => {
            [when_true, when_false].iter().all(|arm| {
                matches!(
                    arm.control.as_ref(),
                    TerminalTargetIntegerControl::Return {
                        expression: TerminalTargetIntegerExpression::Parameter { .. },
                        ..
                    }
                )
            })
        }
        TerminalLegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1 => [
            when_true, when_false,
        ]
        .iter()
        .all(|arm| {
            matches!(
                arm.control.as_ref(),
                TerminalTargetIntegerControl::Return {
                    expression: TerminalTargetIntegerExpression::ExactAdd { left, right, .. },
                    ..
                } if matches!(left.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
                    && matches!(right.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
            )
        }),
        TerminalLegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1 => [
            when_true, when_false,
        ]
        .iter()
        .all(|arm| {
            matches!(
                arm.control.as_ref(),
                TerminalTargetIntegerControl::Return {
                    expression: TerminalTargetIntegerExpression::ExactSubtract { left, right, .. },
                    ..
                } if matches!(left.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
                    && matches!(right.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
            )
        }),
        TerminalLegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1 => [
            when_true, when_false,
        ]
        .iter()
        .all(|arm| {
            matches!(
                arm.control.as_ref(),
                TerminalTargetIntegerControl::Return {
                    expression: TerminalTargetIntegerExpression::IntegerWiden {
                        source_type,
                        operand,
                        ..
                    },
                    ..
                } if *source_type
                    == psi_core::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")
                    && matches!(
                        operand.as_ref(),
                        TerminalTargetIntegerExpression::ExactAdd { left, right, .. }
                            if matches!(left.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
                                && matches!(right.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
                    )
            )
        }),
        TerminalLegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1 => [
            when_true, when_false,
        ]
        .iter()
        .all(|arm| {
            matches!(
                arm.control.as_ref(),
                TerminalTargetIntegerControl::Return {
                    expression: TerminalTargetIntegerExpression::IntegerWiden {
                        source_type,
                        operand,
                        ..
                    },
                    ..
                } if *source_type
                    == psi_core::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")
                    && matches!(
                        operand.as_ref(),
                        TerminalTargetIntegerExpression::ExactSubtract { left, right, .. }
                            if matches!(left.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
                                && matches!(right.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
                    )
            )
        }),
        TerminalLegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1 => {
            matches!(
                (when_true.control.as_ref(), when_false.control.as_ref()),
                (
                    TerminalTargetIntegerControl::Return { expression, .. },
                    TerminalTargetIntegerControl::Return {
                        expression: TerminalTargetIntegerExpression::Immediate { .. },
                        ..
                    }
                ) if replay_active_resident_chain_shape(expression)
            )
        }
    };
    if !recipe_matches_target {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let (offsets, operation_count, leaf_node_counts, parameter_count) = match proposed.recipe {
        TerminalLegalizationRecipe::ReturnU64ImmediateConditionalV1 => ([0, 1, 3], 5, [2, 2], 1),
        TerminalLegalizationRecipe::ReturnU64EntryParameterConditionalV1 => {
            ([0, 1, 2], 3, [1, 1], 2)
        }
        TerminalLegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1
        | TerminalLegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1 => {
            ([0, 1, 5], 9, [4, 4], 1)
        }
        TerminalLegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1
        | TerminalLegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1 => {
            ([0, 1, 6], 11, [5, 5], 1)
        }
        TerminalLegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1 => {
            ([0, 1, 8], 10, [7, 2], 1)
        }
    };
    if abstracted.operations.len() != operation_count
        || abstracted.parameters.len() != parameter_count
        || optimized.parameters.len() != parameter_count
        || abstracted
            .block_entries
            .iter()
            .zip(offsets)
            .any(|(entry, offset)| entry.operation_offset != offset)
        || optimized.blocks[1].nodes.len() != leaf_node_counts[0]
        || optimized.blocks[2].nodes.len() != leaf_node_counts[1]
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
    if proposed.condition_source != *condition_source
        || proposed.condition_parameter_index != *condition_parameter_index
        || proposed.condition_register != *condition_register
        || proposed.condition_definition_site != parameter.site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let entry_node = &optimized.blocks[0].nodes[0];
    if entry_node.operation != abstracted.operations[0] {
        return Err(Error::SourceCustodyMismatch);
    }
    let TerminalAbstractOperation::Conditional {
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
        || entry_node.successors[0].bindings != abstract_true.bindings
        || entry_node.successors[1].psi_edge != abstract_false.psi_edge
        || entry_node.successors[1].target != abstract_false.target
        || entry_node.successors[1].bindings != abstract_false.bindings
        || !entry_node.provenance.is_empty()
        || !entry_node.fuel.is_empty()
        || entry_node.successors[0].provenance != vec![PsiProvenance::Edge(abstract_true.psi_edge)]
        || entry_node.successors[1].provenance != vec![PsiProvenance::Edge(abstract_false.psi_edge)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    if proposed.branch_true_edge != abstract_true.psi_edge
        || proposed.branch_false_edge != abstract_false.psi_edge
        || proposed.branch_true_bindings != abstract_true.bindings
        || proposed.branch_false_bindings != abstract_false.bindings
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_edge_fuel(
        function,
        abstract_true.psi_edge,
        &entry_node.successors[0].fuel,
        &proposed.branch_true_fuel,
    )?;
    replay_edge_fuel(
        function,
        abstract_false.psi_edge,
        &entry_node.successors[1].fuel,
        &proposed.branch_false_fuel,
    )?;

    let true_operations = replay_leaf(
        function,
        proposed.recipe,
        when_true.psi_edge,
        when_true.control.as_ref(),
        &abstracted.operations[offsets[1]..offsets[2]],
        &optimized.blocks[1].nodes,
        abstracted,
        optimized,
        accepted_facts,
        &proposed.when_true,
        architecture,
        0,
    )?;
    let false_operations = replay_leaf(
        function,
        proposed.recipe,
        when_false.psi_edge,
        when_false.control.as_ref(),
        &abstracted.operations[offsets[2]..],
        &optimized.blocks[2].nodes,
        abstracted,
        optimized,
        accepted_facts,
        &proposed.when_false,
        architecture,
        2,
    )?;
    if let (
        TerminalLegalizedLeafValue::EntryParameter {
            parameter_index: true_index,
            register: true_register,
            ..
        },
        TerminalLegalizedLeafValue::EntryParameter {
            parameter_index: false_index,
            register: false_register,
            ..
        },
    ) = (&proposed.when_true.value, &proposed.when_false.value)
        && (proposed.when_true.source_value != proposed.when_false.source_value
            || true_index != false_index
            || true_register != false_register
            || *true_index == *condition_parameter_index)
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let expected_provenance = TerminalPsiProvenance {
        operations: true_operations
            .into_iter()
            .chain(false_operations)
            .collect(),
        edges: vec![
            abstract_true.psi_edge,
            abstract_false.psi_edge,
            proposed.when_true.return_edge,
            proposed.when_false.return_edge,
        ],
    };
    if target.provenance != expected_provenance {
        return Err(Error::SourceCustodyMismatch);
    }
    if proposed.provenance != expected_provenance {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(usize::from(matches!(
        proposed.recipe,
        TerminalLegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1
            | TerminalLegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1
    )) * 2)
}

#[allow(clippy::too_many_arguments)]
fn replay_leaf(
    function: usize,
    recipe: TerminalLegalizationRecipe,
    arm_edge: EdgeId,
    target: &TerminalTargetIntegerControl,
    abstract_operations: &[TerminalAbstractOperation],
    nodes: &[omega_optimization_unit::OptimizationNode],
    abstracted: &omega_terminal_abstract_operations::TerminalAbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed: &TerminalLegalizedLeaf,
    architecture: omega_target::Architecture,
    temporary_base: u32,
) -> Result<Vec<OperationId>, TerminalLegalizationError> {
    if nodes.len() != abstract_operations.len()
        || nodes
            .iter()
            .zip(abstract_operations)
            .any(|(node, operation)| node.operation != *operation)
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let TerminalTargetIntegerControl::Return {
        psi_return_edge,
        source_value,
        expression,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if proposed.return_edge != *psi_return_edge || proposed.source_value != *source_value {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let u64_integer = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u64_type = ScalarType::Integer(u64_integer);
    let (return_node, operations) = match (recipe, expression, &proposed.value) {
        (
            TerminalLegalizationRecipe::ReturnU64ImmediateConditionalV1
            | TerminalLegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1,
            TerminalTargetIntegerExpression::Immediate {
                source_value: expression_source,
                value: target_value,
            },
            TerminalLegalizedLeafValue::Immediate {
                value,
                constant_operation,
                definition_site,
                constant_fuel,
            },
        ) => {
            if nodes.len() != 2 || source_value != expression_source {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let operation = replay_constant(
                function,
                arm_edge,
                *source_value,
                *target_value,
                &nodes[0],
                *constant_operation,
                *definition_site,
                constant_fuel,
                u64_type,
            )?;
            if *value != *target_value {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
            (&nodes[1], vec![operation])
        }
        (
            TerminalLegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1,
            expression,
            TerminalLegalizedLeafValue::ActiveResidentExactAddChain(chain),
        ) if replay_active_resident_chain_shape(expression) => {
            if nodes.len() != 7 {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let TerminalTargetIntegerExpression::ExactAdd {
                psi_operation: result_operation,
                obligation: result_obligation,
                left: result_left,
                right: result_right,
            } = expression
            else {
                unreachable!("shape replay admitted exact add")
            };
            let TerminalTargetIntegerExpression::ExactAdd {
                psi_operation: middle_operation,
                obligation: middle_obligation,
                left: middle_left,
                right: middle_right,
            } = result_right.as_ref()
            else {
                unreachable!("shape replay admitted middle exact add")
            };
            let TerminalTargetIntegerExpression::ExactAdd {
                psi_operation: inner_operation,
                obligation: inner_obligation,
                left: inner_left,
                right: inner_right,
            } = middle_right.as_ref()
            else {
                unreachable!("shape replay admitted inner exact add")
            };
            let resident = &chain.resident;
            let left = &chain.left;
            let right = &chain.right;
            let inner = &chain.inner;
            let middle = &chain.middle;
            let result = &chain.result;
            replay_immediate(
                function,
                arm_edge,
                result_left,
                &nodes[0],
                resident,
                u64_type,
            )?;
            replay_immediate(
                function,
                arm_edge,
                middle_left,
                &nodes[0],
                resident,
                u64_type,
            )?;
            replay_immediate(function, arm_edge, inner_left, &nodes[1], left, u64_type)?;
            replay_immediate(function, arm_edge, inner_right, &nodes[2], right, u64_type)?;
            replay_exact_add_node(
                function,
                optimized,
                accepted_facts,
                &nodes[3],
                *inner_operation,
                *inner_obligation,
                left.source_value,
                right.source_value,
                inner,
            )?;
            replay_exact_add_node(
                function,
                optimized,
                accepted_facts,
                &nodes[4],
                *middle_operation,
                *middle_obligation,
                resident.source_value,
                inner.source_value,
                middle,
            )?;
            replay_exact_add_node(
                function,
                optimized,
                accepted_facts,
                &nodes[5],
                *result_operation,
                *result_obligation,
                resident.source_value,
                middle.source_value,
                result,
            )?;
            if result.source_value != *source_value {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
            (
                &nodes[6],
                vec![
                    resident.constant_operation,
                    left.constant_operation,
                    right.constant_operation,
                    inner.operation,
                    middle.operation,
                    result.operation,
                ],
            )
        }
        (
            TerminalLegalizationRecipe::ReturnU64EntryParameterConditionalV1,
            TerminalTargetIntegerExpression::Parameter {
                source_value: expression_source,
                parameter_index,
                location: TerminalScalarParameterLocation::Register(register),
            },
            TerminalLegalizedLeafValue::EntryParameter {
                parameter_index: proposed_index,
                register: proposed_register,
                definition_site,
            },
        ) => {
            if nodes.len() != 1
                || source_value != expression_source
                || register.architecture() != architecture
            {
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
            if proposed_index != parameter_index
                || proposed_register != register
                || *definition_site != parameter.site
            {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
            (&nodes[0], Vec::new())
        }
        (
            TerminalLegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1,
            TerminalTargetIntegerExpression::ExactAdd {
                psi_operation,
                obligation,
                left,
                right,
            },
            TerminalLegalizedLeafValue::ExactAdd {
                obligation: proposed_obligation,
                accepted_fact,
                add_operation,
                definition_site,
                add_fuel,
                left: proposed_left,
                right: proposed_right,
            },
        ) => {
            replay_exact_binary(
                function,
                arm_edge,
                true,
                *source_value,
                *psi_operation,
                *obligation,
                left,
                right,
                nodes,
                optimized,
                accepted_facts,
                *proposed_obligation,
                *accepted_fact,
                *add_operation,
                *definition_site,
                add_fuel,
                proposed_left,
                proposed_right,
                u64_type,
            )?;
            (
                &nodes[3],
                vec![
                    proposed_left.constant_operation,
                    proposed_right.constant_operation,
                    *add_operation,
                ],
            )
        }
        (
            TerminalLegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1,
            TerminalTargetIntegerExpression::IntegerWiden {
                psi_operation: widen_operation,
                source_type,
                operand,
            },
            TerminalLegalizedLeafValue::WidenedExactAdd {
                source_type: proposed_source_type,
                target_type: proposed_target_type,
                theorem,
                obligation: proposed_obligation,
                accepted_fact,
                add_operation: proposed_add_operation,
                narrow_result,
                add_definition_site,
                add_fuel,
                widen_operation: proposed_widen_operation,
                widen_definition_site,
                widen_fuel,
                left_temporary,
                right_temporary,
                left: proposed_left,
                right: proposed_right,
            },
        ) => {
            let TerminalTargetIntegerExpression::ExactAdd {
                psi_operation: add_operation,
                obligation,
                left,
                right,
            } = operand.as_ref()
            else {
                return Err(Error::NonCanonicalLegalizedPlan);
            };
            let target_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
            replay_widened_exact_binary(
                function,
                arm_edge,
                true,
                *source_value,
                *source_type,
                target_type,
                *add_operation,
                *obligation,
                left,
                right,
                *widen_operation,
                nodes,
                optimized,
                accepted_facts,
                *proposed_source_type,
                *proposed_target_type,
                *theorem,
                *proposed_obligation,
                *accepted_fact,
                *proposed_add_operation,
                *narrow_result,
                *add_definition_site,
                add_fuel,
                *proposed_widen_operation,
                *widen_definition_site,
                widen_fuel,
                *left_temporary,
                *right_temporary,
                proposed_left,
                proposed_right,
                temporary_base,
            )?;
            (
                &nodes[4],
                vec![
                    proposed_left.constant_operation,
                    proposed_right.constant_operation,
                    *proposed_add_operation,
                    *proposed_widen_operation,
                ],
            )
        }
        (
            TerminalLegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1,
            TerminalTargetIntegerExpression::IntegerWiden {
                psi_operation: widen_operation,
                source_type,
                operand,
            },
            TerminalLegalizedLeafValue::WidenedExactSubtract {
                source_type: proposed_source_type,
                target_type: proposed_target_type,
                theorem,
                obligation: proposed_obligation,
                accepted_fact,
                subtract_operation: proposed_subtract_operation,
                narrow_result,
                subtract_definition_site,
                subtract_fuel,
                widen_operation: proposed_widen_operation,
                widen_definition_site,
                widen_fuel,
                left_temporary,
                right_temporary,
                left: proposed_left,
                right: proposed_right,
            },
        ) => {
            let TerminalTargetIntegerExpression::ExactSubtract {
                psi_operation: subtract_operation,
                obligation,
                left,
                right,
            } = operand.as_ref()
            else {
                return Err(Error::NonCanonicalLegalizedPlan);
            };
            let target_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
            replay_widened_exact_binary(
                function,
                arm_edge,
                false,
                *source_value,
                *source_type,
                target_type,
                *subtract_operation,
                *obligation,
                left,
                right,
                *widen_operation,
                nodes,
                optimized,
                accepted_facts,
                *proposed_source_type,
                *proposed_target_type,
                *theorem,
                *proposed_obligation,
                *accepted_fact,
                *proposed_subtract_operation,
                *narrow_result,
                *subtract_definition_site,
                subtract_fuel,
                *proposed_widen_operation,
                *widen_definition_site,
                widen_fuel,
                *left_temporary,
                *right_temporary,
                proposed_left,
                proposed_right,
                temporary_base,
            )?;
            (
                &nodes[4],
                vec![
                    proposed_left.constant_operation,
                    proposed_right.constant_operation,
                    *proposed_subtract_operation,
                    *proposed_widen_operation,
                ],
            )
        }
        (
            TerminalLegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1,
            TerminalTargetIntegerExpression::ExactSubtract {
                psi_operation,
                obligation,
                left,
                right,
            },
            TerminalLegalizedLeafValue::ExactSubtract {
                obligation: proposed_obligation,
                accepted_fact,
                subtract_operation,
                definition_site,
                subtract_fuel,
                left: proposed_left,
                right: proposed_right,
            },
        ) => {
            replay_exact_binary(
                function,
                arm_edge,
                false,
                *source_value,
                *psi_operation,
                *obligation,
                left,
                right,
                nodes,
                optimized,
                accepted_facts,
                *proposed_obligation,
                *accepted_fact,
                *subtract_operation,
                *definition_site,
                subtract_fuel,
                proposed_left,
                proposed_right,
                u64_type,
            )?;
            (
                &nodes[3],
                vec![
                    proposed_left.constant_operation,
                    proposed_right.constant_operation,
                    *subtract_operation,
                ],
            )
        }
        _ => return Err(Error::NonCanonicalLegalizedPlan),
    };

    let TerminalAbstractOperation::Return {
        psi_edge,
        value,
        scalar_type,
        cleanup_actions,
        ..
    } = &return_node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *psi_edge != *psi_return_edge
        || *value != *source_value
        || *scalar_type != u64_type
        || !cleanup_actions.is_empty()
        || return_node.provenance != vec![PsiProvenance::Edge(*psi_return_edge)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    replay_edge_fuel(
        function,
        *psi_return_edge,
        &return_node.fuel,
        &proposed.return_fuel,
    )?;
    Ok(operations)
}

fn replay_active_resident_chain_shape(expression: &TerminalTargetIntegerExpression) -> bool {
    let TerminalTargetIntegerExpression::ExactAdd {
        left: result_left,
        right: result_right,
        ..
    } = expression
    else {
        return false;
    };
    let TerminalTargetIntegerExpression::Immediate {
        source_value: result_resident,
        ..
    } = result_left.as_ref()
    else {
        return false;
    };
    let TerminalTargetIntegerExpression::ExactAdd {
        left: middle_left,
        right: middle_right,
        ..
    } = result_right.as_ref()
    else {
        return false;
    };
    let TerminalTargetIntegerExpression::Immediate {
        source_value: middle_resident,
        ..
    } = middle_left.as_ref()
    else {
        return false;
    };
    matches!(
        middle_right.as_ref(),
        TerminalTargetIntegerExpression::ExactAdd { left, right, .. }
            if matches!(left.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
                && matches!(right.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
                && result_resident == middle_resident
    )
}

#[allow(clippy::too_many_arguments)]
fn replay_exact_add_node(
    function: usize,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    node: &omega_optimization_unit::OptimizationNode,
    operation: OperationId,
    obligation: psi_core::ObligationId,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
    proposed: &omega_terminal_legalized_operations::TerminalLegalizedExactAdd,
) -> Result<(), TerminalLegalizationError> {
    let TerminalAbstractOperation::ExactIntegerAdd {
        psi_operation,
        obligation: abstract_obligation,
        result,
        scalar_type,
        left: abstract_left,
        right: abstract_right,
    } = &node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let u64_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if *psi_operation != operation
        || *abstract_obligation != obligation
        || *scalar_type != u64_type
        || *abstract_left != left
        || *abstract_right != right
        || node.definitions.len() != 1
        || node.definitions[0].value != *result
        || node.provenance != vec![PsiProvenance::Operation(operation)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let Some(fact) = accepted_facts.iter().find(|fact| {
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
                obligation: referenced,
                support,
            } if *referenced == obligation && *support == operation
        )
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    if proposed.source_value != *result
        || proposed.obligation != obligation
        || proposed.accepted_fact != fact.identity
        || proposed.operation != operation
        || proposed.definition_site != node.definitions[0].site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(function, operation, &node.fuel, &proposed.fuel)
}

#[allow(clippy::too_many_arguments)]
fn replay_widened_exact_binary(
    function: usize,
    arm_edge: EdgeId,
    add: bool,
    final_value: psi_core::ValueId,
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    operation: OperationId,
    obligation: psi_core::ObligationId,
    target_left: &TerminalTargetIntegerExpression,
    target_right: &TerminalTargetIntegerExpression,
    widen_operation: OperationId,
    nodes: &[omega_optimization_unit::OptimizationNode],
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed_source_type: psi_core::IntegerType,
    proposed_target_type: psi_core::IntegerType,
    theorem: TerminalLegalizationTheorem,
    proposed_obligation: psi_core::ObligationId,
    proposed_fact: omega_optimization_core::AcceptedObligationFactIdentity,
    proposed_operation: OperationId,
    proposed_narrow_result: psi_core::ValueId,
    proposed_operation_site: omega_optimization_unit::ValueDefinitionSite,
    proposed_operation_fuel: &[omega_optimization_unit::FuelSettlement],
    proposed_widen_operation: OperationId,
    proposed_widen_site: omega_optimization_unit::ValueDefinitionSite,
    proposed_widen_fuel: &[omega_optimization_unit::FuelSettlement],
    left_temporary: TerminalLegalizedTemporaryId,
    right_temporary: TerminalLegalizedTemporaryId,
    proposed_left: &TerminalLegalizedImmediate,
    proposed_right: &TerminalLegalizedImmediate,
    temporary_base: u32,
) -> Result<(), TerminalLegalizationError> {
    let u8_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let u64_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if source_type != u8_type || target_type != u64_type || nodes.len() != 5 {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let expected_theorem = if add {
        TerminalLegalizationTheorem::UnsignedExactAddCommutesWithWidenV1
    } else {
        TerminalLegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1
    };
    if proposed_source_type != source_type
        || proposed_target_type != target_type
        || theorem != expected_theorem
        || left_temporary != TerminalLegalizedTemporaryId(temporary_base)
        || right_temporary != TerminalLegalizedTemporaryId(temporary_base + 1)
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let narrow_scalar = ScalarType::Integer(source_type);
    replay_immediate(
        function,
        arm_edge,
        target_left,
        &nodes[0],
        proposed_left,
        narrow_scalar,
    )?;
    replay_immediate(
        function,
        arm_edge,
        target_right,
        &nodes[1],
        proposed_right,
        narrow_scalar,
    )?;

    let (abstract_operation, abstract_obligation, narrow_result, abstract_source_type, left, right) =
        match (&nodes[2].operation, add) {
            (
                TerminalAbstractOperation::ExactIntegerAdd {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                true,
            )
            | (
                TerminalAbstractOperation::ExactIntegerSubtract {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                false,
            ) => (psi_operation, obligation, result, scalar_type, left, right),
            _ => return Err(Error::UnsupportedSourceShape { function }),
        };
    if *abstract_operation != operation
        || *abstract_obligation != obligation
        || *abstract_source_type != source_type
        || *left != proposed_left.source_value
        || *right != proposed_right.source_value
        || nodes[2].definitions.len() != 1
        || nodes[2].definitions[0].value != *narrow_result
        || nodes[2].definitions[0].scalar_type != narrow_scalar
        || nodes[2].provenance != vec![PsiProvenance::Operation(operation)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let Some(fact) = accepted_facts.iter().find(|fact| {
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
                obligation: referenced,
                support,
            } if *referenced == obligation && *support == operation
        )
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    if proposed_obligation != obligation
        || proposed_fact != fact.identity
        || proposed_operation != operation
        || proposed_narrow_result != *narrow_result
        || proposed_operation_site != nodes[2].definitions[0].site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(function, operation, &nodes[2].fuel, proposed_operation_fuel)?;

    let TerminalAbstractOperation::IntegerWiden {
        psi_operation: abstract_widen,
        result: widen_result,
        source_type: abstract_widen_source,
        target_type: abstract_widen_target,
        operand,
    } = &nodes[3].operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *abstract_widen != widen_operation
        || *widen_result != final_value
        || *narrow_result == final_value
        || *abstract_widen_source != source_type
        || *abstract_widen_target != target_type
        || *operand != *narrow_result
        || nodes[3].definitions.len() != 1
        || nodes[3].definitions[0].value != final_value
        || nodes[3].definitions[0].scalar_type != ScalarType::Integer(target_type)
        || nodes[3].provenance != vec![PsiProvenance::Operation(widen_operation)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    if proposed_widen_operation != widen_operation
        || proposed_widen_site != nodes[3].definitions[0].site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(
        function,
        widen_operation,
        &nodes[3].fuel,
        proposed_widen_fuel,
    )?;

    let narrow_result = if add {
        source_type.exact_add(proposed_left.value, proposed_right.value)
    } else {
        source_type.exact_sub(proposed_left.value, proposed_right.value)
    };
    let Some(narrow_result) = narrow_result else {
        return Err(Error::SourceCustodyMismatch);
    };
    let Some(widened_narrow_result) = source_type.widen_value_to(target_type, narrow_result) else {
        return Err(Error::SourceCustodyMismatch);
    };
    let Some(widened_left) = source_type.widen_value_to(target_type, proposed_left.value) else {
        return Err(Error::SourceCustodyMismatch);
    };
    let Some(widened_right) = source_type.widen_value_to(target_type, proposed_right.value) else {
        return Err(Error::SourceCustodyMismatch);
    };
    let widened_result = if add {
        target_type.exact_add(widened_left, widened_right)
    } else {
        target_type.exact_sub(widened_left, widened_right)
    };
    if widened_result != Some(widened_narrow_result) {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_exact_binary(
    function: usize,
    arm_edge: EdgeId,
    add: bool,
    result_value: psi_core::ValueId,
    operation: OperationId,
    obligation: psi_core::ObligationId,
    target_left: &TerminalTargetIntegerExpression,
    target_right: &TerminalTargetIntegerExpression,
    nodes: &[omega_optimization_unit::OptimizationNode],
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed_obligation: psi_core::ObligationId,
    proposed_fact: omega_optimization_core::AcceptedObligationFactIdentity,
    proposed_operation: OperationId,
    proposed_site: omega_optimization_unit::ValueDefinitionSite,
    proposed_fuel: &[omega_optimization_unit::FuelSettlement],
    proposed_left: &TerminalLegalizedImmediate,
    proposed_right: &TerminalLegalizedImmediate,
    u64_type: ScalarType,
) -> Result<(), TerminalLegalizationError> {
    if nodes.len() != 4 {
        return Err(Error::UnsupportedSourceShape { function });
    }
    replay_immediate(
        function,
        arm_edge,
        target_left,
        &nodes[0],
        proposed_left,
        u64_type,
    )?;
    replay_immediate(
        function,
        arm_edge,
        target_right,
        &nodes[1],
        proposed_right,
        u64_type,
    )?;
    let (abstract_operation, abstract_obligation, result, scalar_type, left, right) =
        match (&nodes[2].operation, add) {
            (
                TerminalAbstractOperation::ExactIntegerAdd {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                true,
            )
            | (
                TerminalAbstractOperation::ExactIntegerSubtract {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                false,
            ) => (psi_operation, obligation, result, scalar_type, left, right),
            _ => return Err(Error::UnsupportedSourceShape { function }),
        };
    let u64_integer = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if *abstract_operation != operation
        || *abstract_obligation != obligation
        || *result != result_value
        || *scalar_type != u64_integer
        || *left != proposed_left.source_value
        || *right != proposed_right.source_value
        || nodes[2].definitions.len() != 1
        || nodes[2].definitions[0].value != result_value
        || nodes[2].provenance != vec![PsiProvenance::Operation(operation)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let Some(fact) = accepted_facts.iter().find(|fact| {
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
                obligation: referenced,
                support,
            } if *referenced == obligation && *support == operation
        )
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    if proposed_obligation != obligation
        || proposed_fact != fact.identity
        || proposed_operation != operation
        || proposed_site != nodes[2].definitions[0].site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(function, operation, &nodes[2].fuel, proposed_fuel)
}

fn replay_immediate(
    function: usize,
    arm_edge: EdgeId,
    target: &TerminalTargetIntegerExpression,
    node: &omega_optimization_unit::OptimizationNode,
    proposed: &TerminalLegalizedImmediate,
    expected_type: ScalarType,
) -> Result<(), TerminalLegalizationError> {
    let TerminalTargetIntegerExpression::Immediate {
        source_value,
        value,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    replay_constant(
        function,
        arm_edge,
        *source_value,
        *value,
        node,
        proposed.constant_operation,
        proposed.definition_site,
        &proposed.fuel,
        expected_type,
    )?;
    if proposed.source_value != *source_value || proposed.value != *value {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_constant(
    function: usize,
    arm_edge: EdgeId,
    source_value: psi_core::ValueId,
    target_value: psi_core::IntegerValue,
    node: &omega_optimization_unit::OptimizationNode,
    proposed_operation: OperationId,
    proposed_site: omega_optimization_unit::ValueDefinitionSite,
    proposed_fuel: &[omega_optimization_unit::FuelSettlement],
    expected_type: ScalarType,
) -> Result<OperationId, TerminalLegalizationError> {
    let TerminalAbstractOperation::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value,
    } = &node.operation
    else {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    };
    if *result != source_value
        || *value != target_value
        || *scalar_type != expected_type
        || node.definitions.len() != 1
        || node.definitions[0].value != source_value
        || node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
    {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    }
    if proposed_operation != *psi_operation || proposed_site != node.definitions[0].site {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(function, *psi_operation, &node.fuel, proposed_fuel)?;
    Ok(*psi_operation)
}

fn replay_operation_fuel(
    function: usize,
    operation: OperationId,
    source: &[omega_optimization_unit::FuelSettlement],
    proposed: &[omega_optimization_unit::FuelSettlement],
) -> Result<(), TerminalLegalizationError> {
    if source.is_empty()
        || source
            .iter()
            .any(|settlement| settlement.site != PsiProvenance::Operation(operation))
    {
        return Err(Error::MissingFuelProvenance { function });
    }
    if proposed != source {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}

fn replay_edge_fuel(
    function: usize,
    edge: EdgeId,
    source: &[omega_optimization_unit::FuelSettlement],
    proposed: &[omega_optimization_unit::FuelSettlement],
) -> Result<(), TerminalLegalizationError> {
    if source.is_empty()
        || source
            .iter()
            .any(|settlement| settlement.site != PsiProvenance::Edge(edge))
    {
        return Err(Error::MissingFuelProvenance { function });
    }
    if proposed != source {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}
