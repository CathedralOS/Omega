use super::super::{FunctionFragmentEmissionError, StagedOptimizedFunctionFragmentEmissionSource};
use super::ordinary;
use crate::StagedPostAllocationMachineFunctionRelativeRealization;

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
    realization: &StagedPostAllocationMachineFunctionRelativeRealization,
) -> Result<ordinary::Emission, FunctionFragmentEmissionError> {
    let allocation = realization.allocation().current();
    ordinary::compute(
        source,
        allocation.selected(),
        realization.layout(),
        realization.manifest().record(),
    )
}
