//! Proof-certified exact self subtraction.

use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedExactIntegerSelfSubtractEliminationRule;

impl LiveProofCertifiedExactIntegerSelfSubtractEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedExactIntegerSelfSubtractEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
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
                    let O::ExactIntegerSubtract {
                        psi_operation,
                        obligation: _,
                        result,
                        scalar_type,
                        left,
                        right,
                    } = &node.operation
                    else {
                        continue;
                    };
                    if left != right
                        || !use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine && use_site.value == *result
                        })
                    {
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
                        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                            unit.identity,
                            Self::contract(),
                            vec![block.id],
                            vec![ProvenanceRewrite {
                                input: site,
                                disposition: ProvenanceDisposition::RealizedAt(site),
                                sources: node.provenance.clone(),
                                fuel: node.fuel.clone(),
                            }],
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
