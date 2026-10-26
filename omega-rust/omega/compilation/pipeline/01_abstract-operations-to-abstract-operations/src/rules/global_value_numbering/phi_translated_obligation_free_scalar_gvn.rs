//! Obligation-free total-scalar translation across join parameters.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate};

use super::expression_keys::total_scalar_expression;
use crate::rules::global_value_numbering::phi_translated_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    PhiTranslatedObligationFreeScalarGvnRule,
    phi_translated_contract(
        b"omega.psi-rule.phi-translated-obligation-free-total-scalar-gvn.v1",
        OptimizationSafetyClass::ExactOperationSemantics,
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
        |unit, function, effects, value_types| {
            let mut leaders = Vec::new();
            let mut redundants = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let Some((key, operation, result, scalar_type)) =
                        total_scalar_expression(&node.operation, value_types)
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
                    redundants.push((key, location, operation, result, scalar_type, ()));
                }
            }
            (leaders, redundants)
        },
        |key, parameters| key.references_any(parameters),
        |key, translation| key.translate(translation),
        |found| {
            PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
                unit.identity,
                contract,
                found.affected_blocks,
                found.provenance,
                -1,
                found.patch,
            )
            .map_err(RuleProposalError::InvalidCandidate)
        },
    )
}
