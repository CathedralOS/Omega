//! Ordinary structural-call custody.
//! Shared graph callees retain their normal result qualifications and returned
//! claim frontier. Checking decided each call's custody once and recorded it
//! in the call plan; lowering reads that record and does not decide it again.
use super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError,
    Multiplicity, unsupported,
};
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;

mod result_uses;
mod shared_temporary;
pub(super) use result_uses::{result_use_operations, validate_consumer, validate_usage};

/// An ordinary structural call: its plan, result binding and custody.
fn ordinary_structural_call(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<(
    &checked_trees::CheckedUnitCallPlan,
    &checked_trees::CheckedUnitStructuralResultBindingPlan,
    &checked_trees::CheckedStructuralCallCustodyPlan,
)> {
    let call = operation.call().filter(|call| call.is_module())?;
    Some((call, call.structural_result()?, call.structural_custody()?))
}

/// Caller result use cannot change the callee's normal result contract. Both
/// straight-line and graph callers rejoin the same complete body signature.
pub(super) fn validate_body_result(
    checked: &CheckedTrees,
    operation: &CheckedUnitEffectOperationPlan,
    expected: checked_trees::CheckedControlResultPlan,
) -> Result<(), LoweringError> {
    let unit_call = operation.call().is_some_and(|call| {
        call.is_module() && *call.result() == checked_trees::CheckedUnitCallResult::Unit
    });
    match ordinary_structural_call(operation) {
        None if unit_call && expected == checked_trees::CheckedControlResultPlan::Unit => Ok(()),
        Some((call, result, custody)) => {
            let checked_trees::CheckedControlResultPlan::Structural(signature) = expected else {
                return unsupported("structural call has no matching body result");
            };
            let contract = checked
                .facts
                .contract_plans
                .for_machine(call.target.machine)
                .ok_or(LoweringError::Unsupported(
                    "structural call contract missing",
                ))?;
            let Some(target_contract_commitment) = call.contract_commitment() else {
                return unsupported("structural call result or commitment disagrees with its body");
            };
            if target_contract_commitment.is_zero()
                || target_contract_commitment != contract.commitment
                || result.statement_index != call.coordinate.statement_index
                || result.type_identity != signature.type_identity
                || result.multiplicity != signature.multiplicity
                || signature.qualifications != custody.result_qualifications
            {
                return unsupported("structural call result or commitment disagrees with its body");
            }
            Ok(())
        }
        _ => unsupported("call cannot erase or invent a body result"),
    }
}
