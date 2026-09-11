//! Project every function through the common graph validator.

use super::*;

pub(super) fn derive_remaining(
    rosters: &mut SourceFunctionRosters,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    for target_function in &target.functions {
        let abstract_matches = abstract_plan
            .functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let optimized_matches = unit
            .functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let ([abstracted], [optimized]) =
            (abstract_matches.as_slice(), optimized_matches.as_slice())
        else {
            return Err(Error::SourceCustodyMismatch);
        };
        rosters.scalar_functions.push(super::scalar_graph::derive(
            target_function,
            abstracted,
            optimized,
            target,
            abstract_plan,
            unit,
        )?);
    }
    Ok(())
}
