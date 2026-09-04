use crate::{
    StagedAllocationRecoveryFunctionRelativeRealization,
    StagedAllocationRecoveryFunctionRelativeSource,
};

use super::super::{FunctionFragmentEmissionError, StagedOptimizedFunctionFragmentEmissionSource};
use super::ordinary;

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
    realization: &StagedAllocationRecoveryFunctionRelativeRealization,
) -> Result<ordinary::Emission, FunctionFragmentEmissionError> {
    match realization.source() {
        StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(homes) => {
            ordinary::compute(
                source,
                homes.reanalysis_stage().transformation_stage().copies(),
                realization.layout(),
                realization.manifest().record(),
            )
        }
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(
            rematerialization,
        ) => ordinary::compute(
            source,
            rematerialization.rematerialization(),
            realization.layout(),
            realization.manifest().record(),
        ),
    }
}
