//! Same-block and dominating scalar CSE validation and rewrite application.
use crate::candidates::accept_region_rewrite;
use crate::candidates::global_value_numbering::{ScalarCseProofClass, ScalarCseScope};
use crate::candidates::retire_node_custody;
use crate::candidates::rewrite_accounting::common_subexpression::reconstruct_local_cse_accounting;
use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationSafetyClass,
};
use optimization_unit::{
    LocalScalarCommonSubexpressionRewrite, NodeLocation, OptimizationFact, PsiOptimizationUnit,
    PsiRewriteCandidate, PsiRewritePatch, ScalarSubstitution, ValueDefinition, ValueDefinitionSite,
};
use std::collections::{BTreeMap, BTreeSet};

use super::admission::{independent_cse_expression, independently_accepted_operation_fact};
use super::dominance_reconstruction::independent_reachable_dominators;
use super::expression_keys::{
    independent_compatible_policy_scalar_leader, independent_compatible_policy_scalar_redundant,
};
use crate::candidates::sweep_output_nodes;

pub(super) fn validate_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    scope: ScalarCseScope,
    proof_class: ScalarCseProofClass,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let expected_safety = match proof_class {
        ScalarCseProofClass::ObligationFree => OptimizationSafetyClass::ExactOperationSemantics,
        ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy => {
            OptimizationSafetyClass::ProofCertified
        }
    };
    let expected_required = match scope {
        ScalarCseScope::SameBlock => {
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries])
        }
        ScalarCseScope::Dominating => AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
    };
    if candidate.required_analyses() != expected_required
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ])
        || candidate.safety_class() != expected_safety
        || candidate.predicted_cost_delta() != -1
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let patch = match (scope, candidate.patch()) {
        (
            ScalarCseScope::SameBlock,
            PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch),
        ) => patch,
        (
            ScalarCseScope::Dominating,
            PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch),
        ) => LocalScalarCommonSubexpressionRewrite {
            leader: patch.leader,
            redundant: patch.redundant,
            leader_operation: patch.leader_operation,
            redundant_operation: patch.redundant_operation,
            leader_result: patch.leader_result,
            redundant_result: patch.redundant_result,
            scalar_type: patch.scalar_type,
        },
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    if candidate.node_decision_point() != Some(patch.redundant) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let expected_substitution = [ScalarSubstitution {
        from: patch.redundant_result,
        to: patch.leader_result,
        scalar_type: patch.scalar_type,
    }];
    if candidate.substitutions() != expected_substitution {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if patch.leader.machine != patch.redundant.machine {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .function(patch.leader.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let leader_block = function
        .block(patch.leader.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant_block = function
        .block(patch.redundant.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let leader = leader_block
        .nodes
        .get(
            usize::try_from(patch.leader.node)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
        )
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant_index = usize::try_from(patch.redundant.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant = crate::candidates::candidate_node(redundant_block, patch.redundant.node)?;
    if redundant_block.nodes.get(redundant_index + 1).is_none() {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    match scope {
        ScalarCseScope::SameBlock
            if patch.leader.block != patch.redundant.block
                || patch.leader.node >= patch.redundant.node =>
        {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        ScalarCseScope::Dominating if patch.leader.block == patch.redundant.block => {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        _ => {}
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
    let admitted_expression = |operation: &O| {
        let row = match proof_class {
            ScalarCseProofClass::CompatiblePolicy => {
                independent_compatible_policy_scalar_leader(operation)?
            }
            _ => independent_cse_expression(operation, &value_types, proof_class)?,
        };
        match (proof_class, row.4) {
            (ScalarCseProofClass::ObligationFree, None) => Some(row),
            (ScalarCseProofClass::ProofCertified, Some(obligation))
                if independently_accepted_operation_fact(input, function, row.1, obligation)
                    .is_some() =>
            {
                Some(row)
            }
            (ScalarCseProofClass::CompatiblePolicy, None)
                if !function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                        if *support == row.1)
                }) =>
            {
                Some(row)
            }
            _ => None,
        }
    };
    let (leader_key, leader_operation, leader_result, leader_type, leader_obligation) =
        match proof_class {
            ScalarCseProofClass::CompatiblePolicy => {
                independent_compatible_policy_scalar_leader(&leader.operation)
            }
            _ => independent_cse_expression(&leader.operation, &value_types, proof_class),
        }
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let (
        redundant_key,
        redundant_operation,
        redundant_result,
        redundant_type,
        redundant_obligation,
    ) = match proof_class {
        ScalarCseProofClass::CompatiblePolicy => {
            independent_compatible_policy_scalar_redundant(&redundant.operation)
        }
        _ => independent_cse_expression(&redundant.operation, &value_types, proof_class),
    }
    .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if leader_key != redundant_key
        || leader_operation != patch.leader_operation
        || redundant_operation != patch.redundant_operation
        || leader_result != patch.leader_result
        || redundant_result != patch.redundant_result
        || leader_type != patch.scalar_type
        || redundant_type != patch.scalar_type
        || leader_result == redundant_result
        || leader_operation == redundant_operation
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let _proof_facts = match (proof_class, leader_obligation, redundant_obligation) {
        (ScalarCseProofClass::ObligationFree, None, None) => {
            if candidate.accepted_obligation_witness().is_some()
                || function.facts.iter().any(|fact| {
                    matches!(
                        fact,
                        OptimizationFact::OperationObligationReference { support, .. }
                            if *support == leader_operation || *support == redundant_operation
                    )
                })
            {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            None
        }
        (ScalarCseProofClass::ProofCertified, Some(leader), Some(redundant)) => {
            let leader_fact =
                independently_accepted_operation_fact(input, function, leader_operation, leader)
                    .ok_or(
                        OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                    )?;
            let redundant_fact = independently_accepted_operation_fact(
                input,
                function,
                redundant_operation,
                redundant,
            )
            .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if candidate.accepted_obligation_witness() != Some(redundant_fact) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            Some((leader_fact, redundant_fact))
        }
        (ScalarCseProofClass::CompatiblePolicy, None, Some(redundant)) => {
            if function.facts.iter().any(|fact| {
                matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                    if *support == leader_operation)
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            let redundant_fact = independently_accepted_operation_fact(
                input,
                function,
                redundant_operation,
                redundant,
            )
            .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if candidate.accepted_obligation_witness() != Some(redundant_fact) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            None
        }
        _ => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
    };
    if scope == ScalarCseScope::SameBlock {
        let canonical_leader = leader_block
            .nodes
            .iter()
            .take(redundant_index)
            .enumerate()
            .filter_map(|(node, candidate)| {
                let (key, _, _, scalar_type, _) = admitted_expression(&candidate.operation)?;
                (key == redundant_key && scalar_type == patch.scalar_type).then_some(NodeLocation {
                    machine: function.machine,
                    block: leader_block.id,
                    node: u32::try_from(node).ok()?,
                })
            })
            .next();
        if canonical_leader != Some(patch.leader) {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
    }
    if scope == ScalarCseScope::Dominating {
        let dominators = independent_reachable_dominators(function);
        if !dominators
            .get(&patch.redundant.block)
            .is_some_and(|rows| rows.contains(&patch.leader.block))
        {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        let canonical_leader = function
            .blocks
            .iter()
            .filter(|block| block.id != patch.redundant.block)
            .filter(|block| {
                dominators
                    .get(&patch.redundant.block)
                    .is_some_and(|rows| rows.contains(&block.id))
            })
            .flat_map(|block| {
                block
                    .nodes
                    .iter()
                    .enumerate()
                    .filter_map(|(node, candidate)| {
                        let (key, _, _, scalar_type, _) =
                            admitted_expression(&candidate.operation)?;
                        (key == redundant_key && scalar_type == patch.scalar_type).then_some(
                            NodeLocation {
                                machine: function.machine,
                                block: block.id,
                                node: u32::try_from(node).ok()?,
                            },
                        )
                    })
            })
            .min_by_key(|location| {
                (
                    dominators
                        .get(&location.block)
                        .map_or(usize::MAX, BTreeSet::len),
                    *location,
                )
            });
        if canonical_leader != Some(patch.leader)
            || function
                .node_uses()
                .filter(|use_site| use_site.value == redundant_result)
                .any(|use_site| {
                    if use_site.block == patch.leader.block {
                        patch.leader.node >= use_site.node
                    } else {
                        !dominators
                            .get(&use_site.block)
                            .is_some_and(|rows| rows.contains(&patch.leader.block))
                    }
                })
        {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
    }
    if leader.definitions
        != [ValueDefinition {
            value: leader_result,
            scalar_type: leader_type,
            site: ValueDefinitionSite::Node {
                block: leader_block.id,
                node: patch.leader.node,
            },
        }]
        || redundant.definitions
            != [ValueDefinition {
                value: redundant_result,
                scalar_type: redundant_type,
                site: ValueDefinitionSite::Node {
                    block: redundant_block.id,
                    node: patch.redundant.node,
                },
            }]
        || !leader.successors.is_empty()
        || !redundant.successors.is_empty()
        || !leader.ownership.is_empty()
        || !redundant.ownership.is_empty()
        || !function
            .blocks
            .iter()
            .flat_map(|row| &row.nodes)
            .flat_map(|row| &row.uses)
            .any(|row| row.value == redundant_result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let (expected_blocks, accepted_provenance) = reconstruct_local_cse_accounting(function, patch)
        .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    accept_region_rewrite(
        input,
        candidate,
        patch.leader.machine,
        &expected_blocks,
        match (scope, proof_class) {
            (ScalarCseScope::SameBlock, ScalarCseProofClass::ObligationFree) => {
                b"omega.validator.same-block-obligation-free-total-scalar-cse.v1".as_slice()
            }
            (ScalarCseScope::Dominating, ScalarCseProofClass::ObligationFree) => {
                b"omega.validator.dominator-total-scalar-cse.v1".as_slice()
            }
            (ScalarCseScope::SameBlock, ScalarCseProofClass::ProofCertified) => {
                b"omega.validator.same-block-proof-certified-total-scalar-cse.v1".as_slice()
            }
            (ScalarCseScope::Dominating, ScalarCseProofClass::ProofCertified) => {
                b"omega.validator.dominator-proof-certified-total-scalar-gvn.v1".as_slice()
            }
            (ScalarCseScope::SameBlock, ScalarCseProofClass::CompatiblePolicy) => {
                b"omega.validator.same-block-proof-certified-compatible-policy-scalar-cse.v1"
                    .as_slice()
            }
            (ScalarCseScope::Dominating, ScalarCseProofClass::CompatiblePolicy) => {
                b"omega.validator.dominator-proof-certified-compatible-policy-scalar-gvn.v1"
                    .as_slice()
            }
        },
        accepted_provenance,
        |output| {
            let output_function = output
                .functions
                .iter_mut()
                .find(|row| row.machine == patch.leader.machine)
                .expect("candidate function exists");
            let output_block = output_function
                .blocks
                .iter_mut()
                .find(|row| row.id == patch.redundant.block)
                .expect("candidate block exists");
            retire_node_custody(output_block, redundant_index)?;
            sweep_output_nodes(output_function, |node| {
                rewrite_scalar_value_uses(&mut node.operation, redundant_result, leader_result);
                Ok(())
            })?;
            Ok(())
        },
    )
}
