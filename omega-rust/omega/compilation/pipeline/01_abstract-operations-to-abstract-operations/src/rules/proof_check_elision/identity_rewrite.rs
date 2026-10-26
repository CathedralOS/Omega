//! Shared construction for proof-certified scalar identity rewrites: the
//! immutable shape an exact identity rule selects, the traversal that joins
//! it to accepted obligation and literal evidence, and the exact zero and one
//! literals the identity rules compare against.

use abstract_operations::AbstractOperation;
use optimization_core::{AnalysisKind, OptimizationRuleContract};
use optimization_unit::{
    IntegerConstantRewrite, NodeLocation, ProofCertifiedScalarIdentityKind,
    ProofCertifiedScalarIdentityRewrite, PsiOptimizationUnit, PsiRewriteCandidate,
    scalar_value_definition,
};
use semantic_vocabulary::{IntegerType, IntegerValue, OperationId, ScalarType, ValueId};

use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::control_flow_cleanup::node_realized_provenance;
use crate::rules::node_elision::node_elision_accounting;
use crate::{AnalysisProduct, RuleAnalysisView, RuleProposalError};

/// Self arithmetic tests liveness before effects and accepted obligations. Keep
/// this traversal separate from scalar identities, whose evidence order differs.
/// Contract and literal callbacks run only at their original constructor sites.
pub(super) fn propose_proof_certified_integer_constants(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    classify: fn(&AbstractOperation) -> Option<(OperationId, ValueId, IntegerType)>,
    constant: fn(IntegerType) -> IntegerValue,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some((psi_operation, result, scalar_type)) = classify(&node.operation) else {
                    continue;
                };
                if !use_definitions.uses.iter().any(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == result
                }) {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                let effect = effects.node(function.machine, block.id, node_index);
                if effect.is_none_or(|row| !row.is_pure_scalar_baseline(unit.identity)) {
                    continue;
                }
                let Ok(obligation_fact) =
                    accepted_obligation_fact(unit, function.machine, psi_operation)
                else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                        unit.identity,
                        contract,
                        vec![block.id],
                        vec![node_realized_provenance(location, node)],
                        obligation_fact,
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation: psi_operation,
                            result,
                            scalar_type,
                            constant: constant(scalar_type),
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

pub(super) fn propose_proof_certified_scalar_identities(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    classify: fn(&AbstractOperation) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
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
                let effect = effects.node(function.machine, block.id, node_index);
                if effect.is_none_or(|row| !row.is_pure_scalar_baseline(unit.identity)) {
                    continue;
                }
                let Some((patch_shape, constant_fact)) =
                    shapes.into_iter().find_map(|(shape, expected)| {
                        // Numeric equality is not carrier equality. These
                        // arithmetic identity laws retain the result carrier;
                        // mixed address/count operations keep their correct
                        // general path instead of proposing an invalid rewrite.
                        let result_type = ScalarType::Integer(shape.scalar_type);
                        if scalar_value_definition(function, shape.replacement)?.scalar_type
                            != result_type
                        {
                            return None;
                        }
                        if matches!(node.operation, AbstractOperation::IntegerArithmetic { .. })
                            && scalar_value_definition(function, shape.identity_operand)?
                                .scalar_type
                                != result_type
                        {
                            return None;
                        }
                        let (actual, fact) = constants
                            .literal_integer_constant(function.machine, shape.identity_operand)?;
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

#[derive(Debug, Clone, Copy)]
pub(super) struct ProofCertifiedScalarIdentityShape {
    pub(super) source_operation: OperationId,
    pub(super) result: ValueId,
    pub(super) replacement: ValueId,
    pub(super) identity_operand: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) identity: ProofCertifiedScalarIdentityKind,
}
