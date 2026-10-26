//! Project every target function through common instruction construction.
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
    let call_descriptor_graph = std::cell::OnceCell::new();
    let mut call_descriptor_origins =
        crate::legalization::scalar_graph_input::byte_views::CallDescriptorOrigins::new(
            unit,
            target,
            abstract_plan,
            &call_descriptor_graph,
        );
    for target_function in &target.functions {
        let mut current_operation = None;
        let function = derive_one(
            target_function,
            target,
            abstract_plan,
            unit,
            &mut current_operation,
            &mut call_descriptor_origins,
        )
        .map_err(|error| error.attributed(target_function.machine, current_operation))?;
        rosters.scalar_functions.push(function);
    }
    Ok(())
}

fn derive_one<'a, 'g>(
    target_function: &target_operations::TargetFunction,
    target: &'a TargetOperationPlan,
    abstract_plan: &'a AbstractOperationPlan,
    unit: &'a PsiOptimizationUnit,
    current_operation: &mut Option<semantic_vocabulary::OperationId>,
    call_descriptor_origins: &mut crate::legalization::scalar_graph_input::byte_views::CallDescriptorOrigins<'a, 'g>,
) -> Result<legalized_operations::LegalizedScalarFunction, LegalizationError>
where
    'a: 'g,
{
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
        call_descriptor_origins,
    )
}
