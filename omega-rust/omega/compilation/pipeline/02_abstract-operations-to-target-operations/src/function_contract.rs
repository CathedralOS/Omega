//! Retain function storage declarations alongside the target ABI that realizes them.
//!
//! Selection needs this contract, not the earlier body's operations. The input
//! has already been admitted; independent correspondence checks remain in stage 03.

use crate::LoweringError;
use abstract_operations::AbstractOperationPlan;
use optimization_unit::PsiOptimizationUnit;
use target_operations::{TargetOperationPlan, TargetStructuralContract};

pub(crate) fn retain(
    source: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    target: &mut TargetOperationPlan,
) -> Result<(), LoweringError> {
    let invalid = |machine, reason| LoweringError::FunctionContract { machine, reason };
    if source.functions.len() != unit.functions.len()
        || source.functions.len() != target.functions.len()
    {
        return Err(invalid(target.entry, "function contract roster differs"));
    }
    for ((source, optimized), target) in source
        .functions
        .iter()
        .zip(&unit.functions)
        .zip(&mut target.functions)
    {
        if source.machine != optimized.machine || source.machine != target.machine {
            return Err(invalid(
                target.machine,
                "function contract identity differs",
            ));
        }
        let has_storage = target.mixed_structural_scalar_abi.is_some()
            || !target.graph.parameters.is_empty()
            || !optimized.declared_places.is_empty();
        target.structural_contract = has_storage.then(|| TargetStructuralContract {
            parameters: source.structural_parameters.clone(),
            places: optimized.structural_places.clone(),
            entry_claims: source.entry_claims.clone(),
            published_service_ceiling: source.published_service_ceiling.clone(),
            result: source.result.structural().cloned(),
        });
    }
    Ok(())
}
