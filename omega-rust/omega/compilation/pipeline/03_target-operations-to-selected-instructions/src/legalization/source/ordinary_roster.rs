//! Project every function through the common graph validator.
use super::{AbstractOperationPlan, PsiOptimizationUnit, TargetOperationPlan};
use crate::LegalizationError as Error;
use crate::LegalizationError;
use crate::legalization::source::SourceFunctionRosters;

/// Project every function, attributing a refusal raised anywhere beneath to
/// this function's machine and to the node the projection had reached, so a
/// diagnostic names authored coordinates rather than the compiler's own
/// check.
pub(super) fn derive_remaining(
    rosters: &mut SourceFunctionRosters,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    for target_function in &target.functions {
        let mut current_operation = None;
        let function = derive_one(
            target_function,
            target,
            abstract_plan,
            unit,
            &mut current_operation,
        )
        .map_err(|error| error.attributed(target_function.machine, current_operation))?;
        rosters.scalar_functions.push(function);
    }
    Ok(())
}

fn derive_one(
    target_function: &target_operations::TargetFunction,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    current_operation: &mut Option<semantic_vocabulary::OperationId>,
) -> Result<legalized_operations::LegalizedScalarFunction, LegalizationError> {
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
    let ([abstracted], [optimized]) = (abstract_matches.as_slice(), optimized_matches.as_slice())
    else {
        return Err(Error::custody());
    };
    super::scalar_graph::derive(
        target_function,
        abstracted,
        optimized,
        target,
        abstract_plan,
        unit,
        current_operation,
    )
}
