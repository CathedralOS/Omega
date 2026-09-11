//! Complete the abstract-optimization phase for verified native input.

use crate::realization::diagnostics::realization_error;
use crate::realization::model::{NativeRealizationCoreRequest, NativeRealizationInput};
use diagnostics::Diagnostic;

/// Current verified abstract program, independent of pass selection.
#[derive(Debug)]
pub(crate) struct NativeOptimizationStageResult {
    pub(crate) program: abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan,
}

pub(crate) fn lower_realization_optimization_stage(
    input: NativeRealizationInput,
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<NativeOptimizationStageResult, Vec<Diagnostic>> {
    let input = input.into_optimization_input();
    if !request.optimization_selections.is_empty() {
        if !request.native_callbacks.is_empty() || !request.callback_thunks.is_empty() {
            return Err(realization_error(
                "optimized native callback custody",
                "retained callbacks require the ordinary custody-preserving pipeline",
            ));
        }
        if !request.ieee_float_fma.is_empty() {
            return Err(realization_error(
                "optimized nearest-FMA custody",
                "retained nearest-FMA occurrences require the ordinary custody-preserving pipeline",
            ));
        }
    }
    let program = run_abstract_optimization_stage(input, request)?;
    if request.optimization_selections.is_empty()
        && program.plan() != program.verified_input().plan()
    {
        return Err(realization_error(
            "abstract optimization identity",
            "empty selection changed the ordinary abstract-operation plan",
        ));
    }
    Ok(NativeOptimizationStageResult { program })
}

fn run_abstract_optimization_stage(
    input: terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<
    abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan,
    Vec<Diagnostic>,
> {
    let optimization_request =
        crate::compiler_baseline_request_v1(request.optimization_selections.selections());
    crate::optimize_verified_abstract_input(input, optimization_request)
        .map_err(|error| realization_error("canonical abstract optimization", error))
}
