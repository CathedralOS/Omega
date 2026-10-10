//! Proof-certified translation from exact redundants to total-policy leaders.



use optimization_core::{AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{
    PhiTranslatedScalarGvnRewrite,
    PsiOptimizationUnit, PsiRewriteCandidate,
};

use crate::rules::global_value_numbering::collect_compatible_policy_scalar_rows;

use crate::rules::global_value_numbering::phi_translated_accounting::collect_phi_translated_incoming;
use crate::rules::global_value_numbering::phi_translated_contract;
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule;

impl PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        phi_translated_contract(
            b"omega.psi-rule.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
        required_analysis!(analyses, AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators)));
        let (use_definitions, effects) = analyses.use_definitions_and_effects()?;

        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .function(function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            let value_types = super::function_value_types(function);
            let (leaders, redundants) =
                collect_compatible_policy_scalar_rows(unit, function, effects);

            for (
                key,
                redundant,
                redundant_operation,
                redundant_result,
                scalar_type,
                obligation_fact,
            ) in redundants
            {
                let Some((incoming, affected_blocks, provenance, parameter_position)) =
                    collect_phi_translated_incoming(
                        function,
                        redundant,
                        redundant_result,
                        scalar_type,
                        use_definitions,
                        &value_types,
                        machine_dominators,
                        &leaders,
                        |parameters| key.references_any(parameters),
                        |translation| Some(key.translate(translation)),
                        true,
                    )
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        obligation_fact,
                        -1,
                        PhiTranslatedScalarGvnRewrite {
                            redundant,
                            redundant_operation,
                            redundant_result,
                            scalar_type,
                            parameter_position,
                            incoming,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}
