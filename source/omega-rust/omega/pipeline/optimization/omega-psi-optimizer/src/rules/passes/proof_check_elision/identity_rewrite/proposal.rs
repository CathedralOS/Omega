//! Shared analysis and immutable-candidate construction for exact identity rules.

use omega_abstract_operations::AbstractOperation;
use omega_optimization_core::{AnalysisKind, OptimizationRuleContract};
use omega_optimization_unit::{
    NodeLocation, ProofCertifiedScalarIdentityRewrite, PsiOptimizationUnit, PsiRewriteCandidate,
};
use psi_core::IntegerValue;

use crate::rules::passes::support::{
    accepted_obligation_fact, literal_integer_constant, node_elision_accounting,
};
use crate::{AnalysisProduct, RuleAnalysisView, RuleProposalError};

use super::ProofCertifiedScalarIdentityShape;

pub(in crate::rules::passes::proof_check_elision) fn propose_proof_certified_scalar_identities(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    classify: fn(&AbstractOperation) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let Some(AnalysisProduct::UseDefinition(use_definitions)) =
        analyses.get(AnalysisKind::UseDefinition)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::UseDefinition,
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
                let shapes = classify(&node.operation);
                if shapes.is_empty() {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                let effect = effects.nodes.iter().find(|row| {
                    row.machine == function.machine
                        && row.block == block.id
                        && row.node == node_index
                });
                if effect.is_none_or(|row| {
                    row.revision != unit.identity
                        || row.class != crate::EffectClass::PureScalar
                        || row.observable != crate::EffectKnowledge::No
                        || row.structural_state != crate::EffectKnowledge::No
                        || row.crash != crate::EffectKnowledge::No
                        || row.suspension != crate::EffectKnowledge::No
                }) {
                    continue;
                }
                let Some((patch_shape, constant_fact)) =
                    shapes.into_iter().find_map(|(shape, expected)| {
                        let (actual, fact) = literal_integer_constant(
                            constants,
                            function.machine,
                            shape.identity_operand,
                        )?;
                        (actual == expected).then_some((shape, fact))
                    })
                else {
                    continue;
                };
                if !use_definitions.uses.iter().any(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == patch_shape.result
                }) {
                    continue;
                }
                let Ok(obligation_fact) =
                    accepted_obligation_fact(unit, function.machine, patch_shape.source_operation)
                else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                let Some((affected_blocks, provenance)) =
                    node_elision_accounting(function, location, patch_shape.result)
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_scalar_identity(
                        unit.identity,
                        contract,
                        affected_blocks,
                        provenance,
                        constant_fact,
                        obligation_fact,
                        -1,
                        ProofCertifiedScalarIdentityRewrite {
                            location,
                            source_operation: patch_shape.source_operation,
                            result: patch_shape.result,
                            replacement: patch_shape.replacement,
                            scalar_type: patch_shape.scalar_type,
                            identity: patch_shape.identity,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}
