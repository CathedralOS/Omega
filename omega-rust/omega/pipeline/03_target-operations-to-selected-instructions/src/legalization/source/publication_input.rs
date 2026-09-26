//! Publication eligibility is input classification, not attempted legalization.
//! Independent construction/replay still checks source, proof and physical custody.

use abstract_operations_to_target_operations::target_operations::TargetOperationPlan;
use terminal_psi_to_abstract_operations::abstract_operations::AbstractOperationPlan;
use terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationUnit;

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
    function: &abstract_operations_to_target_operations::target_operations::TargetFunction,
    abstracted: &terminal_psi_to_abstract_operations::abstract_operations::AbstractFunction,
    optimized: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> bool {
    crate::legalization::scalar_graph_input::match_input(
        function, abstracted, optimized, native, plan, unit,
    )
    .is_ok()
}
