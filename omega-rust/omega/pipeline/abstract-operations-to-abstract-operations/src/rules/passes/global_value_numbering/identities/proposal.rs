//! Shared analysis and immutable-candidate construction for total scalar identities.

use std::collections::BTreeMap;

use abstract_operations::AbstractOperation;
use optimization_core::{AnalysisKind, OptimizationRuleContract};
use optimization_unit::{
    NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityRewrite,
};
use semantic_vocabulary::ScalarType;

use crate::rules::passes::support::{literal_integer_constant, node_elision_accounting};
use crate::{AnalysisProduct, RuleAnalysisView, RuleProposalError};

use super::super::effect_admission::exact_pure_scalar_effect;
use super::TotalScalarIdentityShape;

pub(super) fn propose_total_scalar_identities(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    classify: fn(&AbstractOperation) -> Vec<TotalScalarIdentityShape>,
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
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let shapes = classify(&node.operation);
                if shapes.is_empty() {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                if !exact_pure_scalar_effect(unit, effects, function.machine, block.id, node_index)
                {
                    continue;
                }
                let Some((shape, constant_fact)) = shapes.into_iter().find_map(|shape| {
                    if value_types.get(&shape.law_operand)
                        != Some(&ScalarType::Integer(shape.law_operand_type))
                    {
                        return None;
                    }
                    let (actual, fact) =
                        literal_integer_constant(constants, function.machine, shape.law_operand)?;
                    (actual == shape.expected_law_value).then_some((shape, fact))
                }) else {
                    continue;
                };
                if !use_definitions.uses.iter().any(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == shape.result
                }) {
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
                candidates.push(
                    PsiRewriteCandidate::new_total_scalar_identity(
                        unit.identity,
                        contract,
                        affected_blocks,
                        provenance,
                        constant_fact,
                        -1,
                        TotalScalarIdentityRewrite {
                            location,
                            source_operation: shape.source_operation,
                            result: shape.result,
                            replacement: shape.replacement,
                            scalar_type: shape.scalar_type,
                            identity: shape.identity,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}
