use crate::StagedFixedFrameFunctionRelativeRealization;

use super::super::{FunctionFragmentEmissionError, StagedOptimizedFunctionFragmentEmissionSource};
use super::ordinary;

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
    realization: &StagedFixedFrameFunctionRelativeRealization,
) -> Result<ordinary::Emission, FunctionFragmentEmissionError> {
    let selected = realization
        .homes()
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .selected();
    ordinary::compute(
        source,
        selected,
        realization.layout(),
        realization.manifest().record(),
    )
}
