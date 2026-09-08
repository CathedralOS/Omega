//! Publication eligibility is input classification, not attempted legalization.
//! Independent construction/replay still checks source, proof and physical custody.

use super::shared::*;

pub(crate) fn accepts(
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> bool {
    if native.functions.is_empty()
        || super::custody::validate_source_custody(native, plan, unit, None).is_err()
    {
        return false;
    }
    if super::structural::accepts_publication_input(native, plan, unit) {
        return true;
    }
    native.functions.iter().all(|function| {
        let abstracts = plan
            .functions
            .iter()
            .filter(|value| value.machine == function.machine)
            .collect::<Vec<_>>();
        let optimized = unit
            .functions
            .iter()
            .filter(|value| value.machine == function.machine)
            .collect::<Vec<_>>();
        let ([abstracted], [optimized]) = (abstracts.as_slice(), optimized.as_slice()) else {
            return false;
        };
        eligible_function(native, function, abstracted, optimized, plan, unit)
    })
}

#[allow(clippy::too_many_arguments)]
fn eligible_function(
    native: &TargetOperationPlan,
    function: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> bool {
    if matches!(function.operation, TargetOperation::RankedU32Countdown(_))
        || function.mixed_structural_scalar_abi.is_some()
    {
        return crate::legalization::scalar_graph_input::match_input(
            function, abstracted, optimized, native, plan, unit,
        )
        .is_ok();
    }
    if function.mixed_structural_scalar_abi.is_some()
        || !abstracted.structural_parameters.is_empty()
        || !optimized.structural_parameters.is_empty()
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.declared_places.is_empty()
        || !abstracted.published_service_ceiling.is_empty()
        || !optimized.published_service_ceiling.is_empty()
    {
        return false;
    }
    crate::legalization::scalar_graph_input::match_input(
        function, abstracted, optimized, native, plan, unit,
    )
    .is_ok()
}
