//! Obligation-free total-scalar translation across join parameters.



use optimization_core::{AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{
    NodeLocation, PhiTranslatedScalarGvnRewrite, PsiOptimizationUnit,
    PsiRewriteCandidate,
};

use super::expression_keys::total_scalar_expression;

use crate::rules::global_value_numbering::phi_translated_accounting::collect_phi_translated_incoming;
use crate::rules::global_value_numbering::{exact_pure_scalar_effect, phi_translated_contract};
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PhiTranslatedObligationFreeScalarGvnRule;

impl PhiTranslatedObligationFreeScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        phi_translated_contract(
            b"omega.psi-rule.phi-translated-obligation-free-total-scalar-gvn.v1",
            OptimizationSafetyClass::ExactOperationSemantics,
        )
    }
}

impl PsiOptimizationRule for PhiTranslatedObligationFreeScalarGvnRule {
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
            let mut expressions = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let Some((key, operation, result, scalar_type)) =
                        total_scalar_expression(&node.operation, &value_types)
                    else {
                        continue;
                    };
                    if exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) {
                        expressions.push((
                            key,
                            NodeLocation {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                            operation,
                            result,
                            scalar_type,
                        ));
                    }
                }
            }

            for (key, redundant, redundant_operation, redundant_result, scalar_type) in &expressions
            {
                let Some((incoming, affected_blocks, provenance, parameter_position)) =
                    collect_phi_translated_incoming(
                        function,
                        *redundant,
                        *redundant_result,
                        *scalar_type,
                        use_definitions,
                        &value_types,
                        machine_dominators,
                        &expressions,
                        |parameters| key.references_any(parameters),
                        |translation| key.translate(translation),
                        false,
                    )
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        -1,
                        PhiTranslatedScalarGvnRewrite {
                            redundant: *redundant,
                            redundant_operation: *redundant_operation,
                            redundant_result: *redundant_result,
                            scalar_type: *scalar_type,
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
