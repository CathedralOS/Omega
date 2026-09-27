//! Propose removal of an unused scalar node and account for its realization.
//!
//! Dead-scalar and proof-check-elision rules supply their operation classifier
//! and evidence requirement. The proposal walk joins liveness and effect facts,
//! then retains affected blocks, provenance and fuel for independent validation.
//! Value-numbering rules reuse `node_elision_accounting` when they substitute an
//! existing value before removing a redundant node.

use abstract_operations::AbstractOperation;
use optimization_core::{AnalysisKind, OptimizationRuleContract};
use optimization_unit::{
    DeadScalarNodeRewrite, NodeLocation, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};
use semantic_vocabulary::{BlockId, OperationId, ScalarType, ValueId};

use super::accepted_obligations::accepted_obligation_fact;
use crate::{AnalysisProduct, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DeadScalarShape {
    pub(super) source_operation: OperationId,
    pub(super) result: ValueId,
    pub(super) scalar_type: ScalarType,
}

type Classifier = fn(&AbstractOperation) -> Option<DeadScalarShape>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Evidence {
    Structural,
    AcceptedObligation,
}

pub(super) fn propose_unproved_dead_scalar_nodes(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    classify: Classifier,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    propose(unit, analyses, contract, classify, Evidence::Structural)
}

pub(super) fn propose_proof_certified_dead_scalar_nodes(
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

pub(super) fn node_elision_accounting(
    function: &PsiOptimizationFunction,
    removed_location: NodeLocation,
    removed_result: ValueId,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let block_position = function
        .blocks
        .iter()
        .position(|block| block.id == removed_location.block)?;
    let node_position = usize::try_from(removed_location.node).ok()?;
    let block = &function.blocks[block_position];
    let removed = block.nodes.get(node_position)?;
    block.nodes.get(node_position.checked_add(1)?)?;
    let mut provenance = vec![ProvenanceRewrite {
        input: PsiRealizationSite::Node(removed_location),
        disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(removed_location)),
        sources: removed.provenance.clone(),
        fuel: removed.fuel.clone(),
    }];
    for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
        if node.provenance.is_empty() {
            continue;
        }
        let old = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        };
        let new = NodeLocation {
            node: old.node.checked_sub(1)?,
            ..old
        };
        provenance.push(ProvenanceRewrite {
            input: PsiRealizationSite::Node(old),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(new)),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    let mut affected = vec![block.id];
    for later in function.blocks.iter().skip(block_position + 1) {
        affected.push(later.id);
        for (index, node) in later.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: later.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    for use_block in &function.blocks {
        if affected.contains(&use_block.id)
            || !use_block
                .nodes
                .iter()
                .flat_map(|node| &node.uses)
                .any(|row| row.value == removed_result)
        {
            continue;
        }
        affected.push(use_block.id);
        for (index, node) in use_block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: use_block.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    affected.sort();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected, provenance))
}
