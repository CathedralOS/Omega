use crate::StagedSelectedLoweringFunctionRelativeRealization;

use super::super::{FunctionFragmentEmissionError, StagedOptimizedFunctionFragmentEmissionSource};
use super::ordinary;

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
    realization: &StagedSelectedLoweringFunctionRelativeRealization,
) -> Result<ordinary::Emission, FunctionFragmentEmissionError> {
    let run = realization.homes().selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    match run.steps().last() {
        Some(step) => ordinary::compute(
            source,
            step.fold(),
            realization.layout(),
            realization.manifest().record(),
        ),
        None => ordinary::compute(
            source,
            selected_stage.selected(),
            realization.layout(),
            realization.manifest().record(),
        ),
    }
}
