//! Proof-certified versus exact-operation binary witness construction.

use optimization_core::{OptimizationSafetyClass, ScalarConstantFactIdentity};
use optimization_unit::{IntegerEvaluationWitness, PsiOptimizationUnit};
use semantic_vocabulary::{MachineId, OperationId};

use crate::RuleProposalError;
use crate::rules::passes::support::accepted_obligation_fact;

pub(super) fn build(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
    safety: OptimizationSafetyClass,
    left_fact: ScalarConstantFactIdentity,
    right_fact: ScalarConstantFactIdentity,
) -> Result<IntegerEvaluationWitness, RuleProposalError> {
    if safety == OptimizationSafetyClass::ProofCertified {
        Ok(IntegerEvaluationWitness::ProofCertifiedBinary {
            left_fact,
            right_fact,
            obligation_fact: accepted_obligation_fact(unit, machine, operation)?,
        })
    } else {
        Ok(IntegerEvaluationWitness::Binary {
            left_fact,
            right_fact,
        })
    }
}
