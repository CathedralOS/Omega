//! Proof-certified remainder by one.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{
    IntegerConstantRewrite, NodeLocation, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};
use psi_core::IntegerCarrier;

use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::super::{
    PROOF_CHECK_ELISION_PASS_NAME, accepted_obligation_fact, literal_integer_constant,
};
use super::identity_rewrite::{integer_one, integer_zero};

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerRemainderByOneEliminationRule;

impl LiveProofCertifiedIntegerRemainderByOneEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedIntegerRemainderByOneEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
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
                    let (psi_operation, result, scalar_type, left, right) = match &node.operation {
                        O::ExactIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        }
                        | O::WrappingIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        }
                        | O::SaturatingIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        } => (psi_operation, result, scalar_type, left, right),
                        _ => continue,
                    };
                    // Keep the earlier self-remainder rule as the sole owner
                    // of the overlapping `1 % 1`/`x % x` shape.
                    if left == right
                        || scalar_type.carrier() != IntegerCarrier::Fixed
                        || !use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine && use_site.value == *result
                        })
                    {
                        continue;
                    }
                    let Some((constant, constant_fact)) =
                        literal_integer_constant(constants, function.machine, *right)
                    else {
                        continue;
                    };
                    if constant != integer_one(*scalar_type) || !scalar_type.admits(constant) {
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
                    let Ok(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, *psi_operation)
                    else {
                        continue;
                    };
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let site = PsiRealizationSite::Node(location);
                    candidates.push(
                        PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
                            unit.identity,
                            Self::contract(),
                            vec![block.id],
                            vec![ProvenanceRewrite {
                                input: site,
                                disposition: ProvenanceDisposition::RealizedAt(site),
                                sources: node.provenance.clone(),
                                fuel: node.fuel.clone(),
                            }],
                            constant_fact,
                            obligation_fact,
                            -1,
                            IntegerConstantRewrite {
                                location,
                                source_operation: *psi_operation,
                                result: *result,
                                scalar_type: *scalar_type,
                                constant: integer_zero(*scalar_type),
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}
