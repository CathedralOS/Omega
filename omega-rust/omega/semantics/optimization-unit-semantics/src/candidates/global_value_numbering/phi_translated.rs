//! Phi-translated join validation and rewrite application.

use super::admission::*;
use super::dominance_reconstruction::*;
use super::expression_keys::*;
use super::*;

/// Independently validate one obligation-free, proof-certified, or
/// proof-certified compatible-policy scalar
/// expression translated through every incoming binding of an acyclic join.
/// The redundant result identity becomes a new join parameter; every incoming
/// edge supplies the canonical available leader for its translated expression.
pub(super) fn validate_phi_translated_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    proof_class: ScalarCseProofClass,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let expected_safety = match proof_class {
        ScalarCseProofClass::ObligationFree => OptimizationSafetyClass::ExactOperationSemantics,
        ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy => {
            OptimizationSafetyClass::ProofCertified
        }
    };
    if candidate.required_analyses()
        != AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ])
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ])
        || candidate.safety_class() != expected_safety
        || candidate.predicted_cost_delta() != -1
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.redundant) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|row| row.machine == patch.redundant.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let join = function
        .blocks
        .iter()
        .find(|row| row.id == patch.redundant.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant_index = usize::try_from(patch.redundant.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant = join
        .nodes
        .get(redundant_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == join.id
        || join.nodes.get(redundant_index + 1).is_none()
        || usize::try_from(patch.parameter_position).ok() != Some(join.parameters.len())
        || join
            .parameters
            .iter()
            .any(|row| row.value == patch.redundant_result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let value_types = function
        .parameters
        .iter()
        .map(|row| (row.value, row.scalar_type))
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .map(|row| (row.value, row.scalar_type))
        }))
        .chain(function.blocks.iter().flat_map(|block| {
            block.nodes.iter().flat_map(|node| {
                node.definitions
                    .iter()
                    .map(|row| (row.value, row.scalar_type))
            })
        }))
        .collect::<BTreeMap<_, _>>();
    let (_, redundant_operation, redundant_result, redundant_type, redundant_obligation) =
        match proof_class {
            ScalarCseProofClass::CompatiblePolicy => {
                independent_compatible_policy_scalar_redundant(&redundant.operation)
            }
            _ => independent_cse_expression(&redundant.operation, &value_types, proof_class),
        }
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if redundant_operation != patch.redundant_operation
        || redundant_result != patch.redundant_result
        || redundant_type != patch.scalar_type
        || redundant.definitions
            != [ValueDefinition {
                value: redundant_result,
                scalar_type: redundant_type,
                site: ValueDefinitionSite::Node {
                    block: join.id,
                    node: patch.redundant.node,
                },
            }]
        || !redundant.successors.is_empty()
        || !redundant.ownership.is_empty()
        || !function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.uses)
            .any(|use_site| use_site.value == redundant_result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let _redundant_fact = match (proof_class, redundant_obligation) {
        (ScalarCseProofClass::ObligationFree, None) => {
            if candidate.accepted_obligation_witness().is_some()
                || function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                        if *support == redundant_operation)
                })
            {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            None
        }
        (
            ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy,
            Some(obligation),
        ) => {
            let fact = independently_accepted_operation_fact(
                input,
                function,
                redundant_operation,
                obligation,
            )
            .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if candidate.accepted_obligation_witness() != Some(fact) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            Some(fact)
        }
        _ => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
    };

    let dominators = independent_reachable_dominators(function);
    let mut expected_incoming = Vec::new();
    for source in &function.blocks {
        for edge in source
            .nodes
            .iter()
            .flat_map(|node| &node.successors)
            .filter(|edge| edge.target == join.id)
        {
            if edge.bindings.len() != join.parameters.len() {
                return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
            }
            let mut translated = redundant.operation.clone();
            for (parameter, binding) in join.parameters.iter().zip(&edge.bindings) {
                if binding.parameter != parameter.value
                    || binding.scalar_type != parameter.scalar_type
                    || value_types.get(&binding.argument) != Some(&binding.scalar_type)
                {
                    return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
                }
                rewrite_scalar_value_uses(&mut translated, parameter.value, binding.argument);
            }
            let (translated_key, _, _, translated_type, _) = match proof_class {
                ScalarCseProofClass::CompatiblePolicy => {
                    independent_compatible_policy_scalar_redundant(&translated)
                }
                _ => independent_cse_expression(&translated, &value_types, proof_class),
            }
            .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            let mut available_leaders = Vec::new();
            let mut missing_leader_evidence = false;
            for leader_block in &function.blocks {
                for (node_index, node) in leader_block.nodes.iter().enumerate() {
                    let available = if leader_block.id == source.id {
                        node_index + 1 < source.nodes.len()
                    } else {
                        dominators
                            .get(&source.id)
                            .is_some_and(|rows| rows.contains(&leader_block.id))
                    };
                    if !available {
                        continue;
                    }
                    let Some((key, operation, result, scalar_type, obligation)) = (match proof_class
                    {
                        ScalarCseProofClass::CompatiblePolicy => {
                            independent_compatible_policy_scalar_leader(&node.operation)
                        }
                        _ => independent_cse_expression(&node.operation, &value_types, proof_class),
                    }) else {
                        continue;
                    };
                    let admitted = match (proof_class, obligation) {
                        (ScalarCseProofClass::ObligationFree, None) => !function
                            .facts
                            .iter()
                            .any(|fact| matches!(fact, OptimizationFact::OperationObligationReference { support, .. } if *support == operation)),
                        (ScalarCseProofClass::ProofCertified, Some(obligation)) => {
                            independently_accepted_operation_fact(
                                input,
                                function,
                                operation,
                                obligation,
                            )
                            .is_some()
                        }
                        (ScalarCseProofClass::CompatiblePolicy, None) => !function
                            .facts
                            .iter()
                            .any(|fact| matches!(fact, OptimizationFact::OperationObligationReference { support, .. } if *support == operation)),
                        _ => false,
                    };
                    if !admitted
                        && ((proof_class == ScalarCseProofClass::ProofCertified
                            && obligation.is_some())
                            || proof_class == ScalarCseProofClass::CompatiblePolicy)
                        && key == translated_key
                        && scalar_type == translated_type
                    {
                        missing_leader_evidence = true;
                    }
                    if admitted && key == translated_key && scalar_type == translated_type {
                        available_leaders.push((
                            NodeLocation {
                                machine: function.machine,
                                block: leader_block.id,
                                node: u32::try_from(node_index).map_err(|_| {
                                    OptimizationUnitValidationError::CandidateLocationMissing
                                })?,
                            },
                            operation,
                            result,
                            obligation,
                        ));
                    }
                }
            }
            let canonical = available_leaders
                .into_iter()
                .min_by_key(|(location, _, _, _)| {
                    (
                        dominators
                            .get(&location.block)
                            .map_or(usize::MAX, BTreeSet::len),
                        *location,
                    )
                })
                .ok_or(if missing_leader_evidence {
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch
                } else {
                    OptimizationUnitValidationError::CandidatePatchMismatch
                })?;
            expected_incoming.push(PhiTranslatedScalarIncoming {
                source: source.id,
                edge: edge.psi_edge,
                leader: canonical.0,
                leader_operation: canonical.1,
                leader_result: canonical.2,
            });
        }
    }
    expected_incoming.sort_by_key(|row| (row.edge, row.source));
    if expected_incoming.len() < 2 || patch.incoming != expected_incoming {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_phi_translated_cse_accounting(function, &patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|row| row.machine == patch.redundant.machine)
        .expect("candidate function exists");
    let output_join = output_function
        .blocks
        .iter_mut()
        .find(|row| row.id == patch.redundant.block)
        .expect("candidate join exists");
    output_join.parameters.push(ValueDefinition {
        value: patch.redundant_result,
        scalar_type: patch.scalar_type,
        site: ValueDefinitionSite::BlockParameter {
            block: patch.redundant.block,
            position: patch.parameter_position,
        },
    });
    let removed = output_join.nodes.remove(redundant_index);
    let receiver = output_join
        .nodes
        .get_mut(redundant_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&removed.provenance);
    receiver.fuel.extend_from_slice(&removed.fuel);
    for incoming in &patch.incoming {
        let source = output_function
            .blocks
            .iter_mut()
            .find(|row| row.id == incoming.source)
            .expect("incoming source exists");
        let node = source
            .nodes
            .iter_mut()
            .find(|node| {
                node.successors
                    .iter()
                    .any(|edge| edge.psi_edge == incoming.edge)
            })
            .expect("incoming edge exists");
        let edge = node
            .successors
            .iter()
            .find(|edge| edge.psi_edge == incoming.edge)
            .expect("incoming edge exists");
        let mut bindings = edge.bindings.clone();
        bindings.push(abstract_operations::ValueBinding {
            parameter: patch.redundant_result,
            argument: incoming.leader_result,
            scalar_type: patch.scalar_type,
        });
        if !rewrite_successor_operation(
            &mut node.operation,
            incoming.edge,
            patch.redundant.block,
            &bindings,
        ) {
            return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
        }
    }
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
            node.effect = optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = effect
                .checked_add(1)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|row| row.machine == function.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|row| row.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(match proof_class {
            ScalarCseProofClass::ObligationFree => {
                b"omega.validator.phi-translated-obligation-free-total-scalar-gvn.v1"
            }
            ScalarCseProofClass::ProofCertified => {
                b"omega.validator.phi-translated-proof-certified-total-scalar-gvn.v1"
            }
            ScalarCseProofClass::CompatiblePolicy => {
                b"omega.validator.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1"
            }
        }),
        provenance: accepted_provenance,
    })
}
