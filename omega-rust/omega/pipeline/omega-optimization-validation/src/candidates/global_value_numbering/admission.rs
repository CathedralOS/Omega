//! Proof-class expression reconstruction and accepted-obligation evidence.

use super::expression_keys::*;
use super::*;

pub(crate) fn independent_cse_expression(
    operation: &O,
    value_types: &BTreeMap<ValueId, ScalarType>,
    proof_class: ScalarCseProofClass,
) -> Option<(
    IndependentScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    Option<psi_core::ObligationId>,
)> {
    match proof_class {
        ScalarCseProofClass::ObligationFree => {
            let (key, operation, result, scalar_type) =
                independent_total_scalar_expression(operation, value_types)?;
            Some((
                IndependentScalarExpressionKey::ObligationFree(key),
                operation,
                result,
                scalar_type,
                None,
            ))
        }
        ScalarCseProofClass::ProofCertified => {
            let (key, operation, result, scalar_type, obligation) =
                independent_proof_scalar_expression(operation)?;
            Some((
                IndependentScalarExpressionKey::ProofCertified(key),
                operation,
                result,
                scalar_type,
                Some(obligation),
            ))
        }
        ScalarCseProofClass::CompatiblePolicy => None,
    }
}

pub(crate) fn independently_accepted_operation_fact(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    operation: OperationId,
    obligation: psi_core::ObligationId,
) -> Option<omega_optimization_core::AcceptedObligationFactIdentity> {
    function
        .facts
        .iter()
        .any(|fact| {
            matches!(
                fact,
                OptimizationFact::OperationObligationReference {
                    obligation: reference,
                    support,
                } if *support == operation && *reference == obligation
            )
        })
        .then(|| {
            input
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.machine == function.machine
                        && fact.operation == operation
                        && fact.obligation == obligation
                })
                .map(|fact| fact.identity)
        })
        .flatten()
}
