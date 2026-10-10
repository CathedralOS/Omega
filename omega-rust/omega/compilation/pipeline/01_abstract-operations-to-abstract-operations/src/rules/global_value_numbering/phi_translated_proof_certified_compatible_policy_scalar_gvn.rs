//! Proof-certified translation from exact redundants to total-policy leaders.



use optimization_core::{AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{
    NodeLocation, OptimizationFact, PhiTranslatedScalarGvnRewrite,
    PsiOptimizationUnit, PsiRewriteCandidate,
};

use super::exact_pure_scalar_effect;
use super::expression_keys::{compatible_policy_scalar_leader, compatible_policy_scalar_redundant};

use crate::rules::accepted_obligations::accepted_obligation_fact;
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
            let mut leaders = Vec::new();
            let mut redundants = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    if let Some((key, operation, result, scalar_type)) =
                        compatible_policy_scalar_leader(&node.operation)
                        && exact_pure_scalar_effect(
                            unit,
                            effects,
                            function.machine,
                            block.id,
                            node_index,
                        )
                        && !function.facts.iter().any(|fact| {
                            matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                                if *support == operation)
                        })
                    {
                        leaders.push((key, location, operation, result, scalar_type));
                    }
                    if let Some((key, operation, result, scalar_type)) =
                        compatible_policy_scalar_redundant(&node.operation)
                        && let Ok(obligation_fact) =
                            accepted_obligation_fact(unit, function.machine, operation)
                        && exact_pure_scalar_effect(
                            unit,
                            effects,
                            function.machine,
                            block.id,
                            node_index,
                        )
                    {
                        redundants.push((
                            key,
                            location,
                            operation,
                            result,
                            scalar_type,
                            obligation_fact,
                        ));
                    }
                }
            }

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
