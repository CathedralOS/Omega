//! Dead scalar-node proposal shared by the dead-scalar and proof-check
//! elision passes: the closed shape a rule's admission selects and the
//! liveness/effect traversal that turns it into a rewrite candidate.

use abstract_operations::AbstractOperation;
use optimization_core::{AnalysisKind, OptimizationRuleContract};
use optimization_unit::{
    DeadScalarNodeRewrite, NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate,
};
use semantic_vocabulary::{OperationId, ScalarType, ValueId};

use crate::rules::support::{accepted_obligation_fact, node_elision_accounting};
use crate::{AnalysisProduct, RuleAnalysisView, RuleProposalError};

type Classifier = fn(&AbstractOperation) -> Option<DeadScalarShape>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Evidence {
    Structural,
    AcceptedObligation,
}

pub(in crate::rules) fn propose_unproved_dead_scalar_nodes(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    classify: Classifier,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    propose(unit, analyses, contract, classify, Evidence::Structural)
}

pub(in crate::rules) fn propose_proof_certified_dead_scalar_nodes(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    classify: Classifier,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    propose(
        unit,
        analyses,
        contract,
        classify,
        Evidence::AcceptedObligation,
    )
}

fn propose(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    classify: Classifier,
    evidence: Evidence,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ValueLiveness(liveness)) = analyses.get(AnalysisKind::ValueLiveness)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ValueLiveness,
        ));
    };
    let Some(AnalysisProduct::EffectSummaries(effects)) =
        analyses.get(AnalysisKind::EffectSummaries)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::EffectSummaries,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some(shape) = classify(&node.operation) else {
                    continue;
                };
                let Some(next) = block.nodes.get(node_index + 1) else {
                    continue;
                };
                if next
                    .provenance
                    .iter()
                    .any(|source| node.provenance.contains(source))
                {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                let live = liveness
                    .blocks
                    .iter()
                    .find(|row| row.machine == function.machine && row.block == block.id)
                    .and_then(|row| row.nodes.iter().find(|row| row.node == node_index));
                let effect = effects.nodes.iter().find(|row| {
                    row.machine == function.machine
                        && row.block == block.id
                        && row.node == node_index
                });
                if live.is_none_or(|row| row.exit.contains(&shape.result))
                    || effect.is_none_or(|row| {
                        row.revision != unit.identity
                            || row.class != crate::EffectClass::PureScalar
                            || row.observable != crate::EffectKnowledge::No
                            || row.structural_state != crate::EffectKnowledge::No
                            || row.crash != crate::EffectKnowledge::No
                            || row.suspension != crate::EffectKnowledge::No
                    })
                {
                    continue;
                }
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                let Some((affected_blocks, provenance)) =
                    node_elision_accounting(function, location, shape.result)
                else {
                    continue;
                };
                let patch = DeadScalarNodeRewrite {
                    location,
                    source_operation: shape.source_operation,
                    result: shape.result,
                    scalar_type: shape.scalar_type,
                };
                let candidate = match evidence {
                    Evidence::Structural => PsiRewriteCandidate::new_dead_scalar_node(
                        unit.identity,
                        contract,
                        affected_blocks,
                        provenance,
                        -1,
                        patch,
                    ),
                    Evidence::AcceptedObligation => {
                        PsiRewriteCandidate::new_proof_certified_dead_scalar_node(
                            unit.identity,
                            contract,
                            affected_blocks,
                            provenance,
                            accepted_obligation_fact(
                                unit,
                                function.machine,
                                shape.source_operation,
                            )?,
                            -1,
                            patch,
                        )
                    }
                };
                candidates.push(candidate.map_err(RuleProposalError::InvalidCandidate)?);
            }
        }
    }
    Ok(candidates)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::rules) struct DeadScalarShape {
    pub(in crate::rules) source_operation: OperationId,
    pub(in crate::rules) result: ValueId,
    pub(in crate::rules) scalar_type: ScalarType,
}
