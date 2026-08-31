use crate::{
    StagedPostAllocationMachineFunctionRelativeRealization,
    StagedPostAllocationMachineFunctionRelativeSource,
};

use super::super::{FunctionFragmentEmissionError, StagedOptimizedFunctionFragmentEmissionSource};
use super::ordinary;

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
    realization: &StagedPostAllocationMachineFunctionRelativeRealization,
) -> Result<ordinary::Emission, FunctionFragmentEmissionError> {
    match realization.source() {
        StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => {
            let selected = homes
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
        StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
            let run = homes.selected_lowering_run();
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
    }
}
