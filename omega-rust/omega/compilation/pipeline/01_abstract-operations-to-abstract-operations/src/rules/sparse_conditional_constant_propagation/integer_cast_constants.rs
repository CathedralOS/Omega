//! The proof-certified exact integer-cast constant fold: the rule, and the
//! classification that joins a constant operand to its accepted cast
//! obligation.

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    OptimizationRuleContract, OptimizationSafetyClass, ScalarConstantFactIdentity,
};
use optimization_unit::{IntegerEvaluationWitness, PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::{MachineId, OperationId};

use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::sparse_conditional_constant_propagation::{
    constant_evaluation_contract, integer_constant,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    ExactIntegerCastConstantsRule,
    constant_evaluation_contract(
        b"omega.psi-rule.exact-integer-cast-constants.v1",
        OptimizationSafetyClass::ProofCertified,
    ),
    propose = propose,
);

fn propose(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    super::propose_integer_constant_folds(
        unit,
        analyses,
        contract,
        |unit, machine, node, constants| {
            let O::IntegerExactCast {
                psi_operation,
                result,
                source_type,
                target_type,
                operand,
                ..
            } = node.operation
            else {
                return Ok(None);
            };
            let Some((operand_value, operand_fact)) = integer_constant(constants, machine, operand)
            else {
                return Ok(None);
            };
            let Some(constant) = source_type.exact_cast_value_to(target_type, operand_value) else {
                return Ok(None);
            };
            Ok(Some(super::IntegerConstantFold {
                source_operation: psi_operation,
                result,
                scalar_type: target_type,
                constant,
                witness: proof_certified_unary_witness(unit, machine, psi_operation, operand_fact)?,
            }))
        },
    )
}

fn proof_certified_unary_witness(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
    operand_fact: ScalarConstantFactIdentity,
) -> Result<IntegerEvaluationWitness, RuleProposalError> {
    Ok(IntegerEvaluationWitness::ProofCertifiedUnary {
        operand_fact,
        obligation_fact: accepted_obligation_fact(unit, machine, operation)?,
    })
}
