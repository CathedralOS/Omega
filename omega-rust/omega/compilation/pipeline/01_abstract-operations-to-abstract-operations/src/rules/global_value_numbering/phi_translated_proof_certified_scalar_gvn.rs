//! Proof-certified total-scalar translation across join parameters.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate};

use super::expression_keys::proof_certified_scalar_expression;
use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::global_value_numbering::phi_translated_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    PhiTranslatedProofCertifiedScalarGvnRule,
    phi_translated_contract(
        b"omega.psi-rule.phi-translated-proof-certified-total-scalar-gvn.v1",
        OptimizationSafetyClass::ProofCertified,
    ),
    propose = propose,
);

fn propose(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    super::propose_phi_translated_scalar_gvn(
        unit,
        analyses,
        |unit, function, effects, _| {
            let mut leaders = Vec::new();
            let mut redundants = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let Some((key, operation, result, scalar_type)) =
                        proof_certified_scalar_expression(&node.operation)
                    else {
                        continue;
                    };
                    let Some(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, operation).ok()
                    else {
                        continue;
                    };
                    if !super::exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) {
                        continue;
                    }
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    leaders.push((key, location, operation, result, scalar_type));
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
            (leaders, redundants)
        },
        |key, parameters| key.references_any(parameters),
        |key, translation| Some(key.translate(translation)),
        |found| {
            PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                unit.identity,
                contract,
                found.affected_blocks,
                found.provenance,
                found.extras,
                -1,
                found.patch,
            )
            .map_err(RuleProposalError::InvalidCandidate)
        },
    )
}
