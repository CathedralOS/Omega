use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass};
use omega_optimization_unit::{
    IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};

use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::super::constant_evaluation_contract;
use super::integer_constant;

macro_rules! integer_unary_rule {
    ($name:ident, $rule_name:literal, $kind:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                constant_evaluation_contract(
                    $rule_name,
                    OptimizationSafetyClass::ExactOperationSemantics,
                )
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                propose_integer_unary_constants(unit, analyses, Self::contract(), $kind)
            }
        }
    };
}

integer_unary_rule!(
    IntegerWidenConstantsRule,
    b"omega.psi-rule.integer-widen-constants.v1",
    IntegerUnaryKind::Widen
);
integer_unary_rule!(
    IntegerBitwiseNotConstantsRule,
    b"omega.psi-rule.integer-bitwise-not-constants.v1",
    IntegerUnaryKind::BitwiseNot
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegerUnaryKind {
    Widen,
    BitwiseNot,
}

fn propose_integer_unary_constants(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: IntegerUnaryKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let (source_operation, result, source_type, target_type, operand) =
                    match (&node.operation, kind) {
                        (
                            O::IntegerWiden {
                                psi_operation,
                                result,
                                source_type,
                                target_type,
                                operand,
                            },
                            IntegerUnaryKind::Widen,
                        ) => (
                            *psi_operation,
                            *result,
                            *source_type,
                            *target_type,
                            *operand,
                        ),
                        (
                            O::IntegerBitwiseNot {
                                psi_operation,
                                result,
                                scalar_type,
                                operand,
                            },
                            IntegerUnaryKind::BitwiseNot,
                        ) => (
                            *psi_operation,
                            *result,
                            *scalar_type,
                            *scalar_type,
                            *operand,
                        ),
                        _ => continue,
                    };
                let Some((operand_value, operand_fact)) =
                    integer_constant(constants, function.machine, operand)
                else {
                    #[cfg(test)]
                    eprintln!("linear thread refused accounting");
                    continue;
                };
                let constant = match kind {
                    IntegerUnaryKind::Widen => {
                        source_type.widen_value_to(target_type, operand_value)
                    }
                    IntegerUnaryKind::BitwiseNot => source_type.bitwise_not(operand_value),
                };
                let Some(constant) = constant else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_integer_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        IntegerEvaluationWitness::Unary { operand_fact },
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation,
                            result,
                            scalar_type: target_type,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}
