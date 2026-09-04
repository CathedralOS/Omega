//! Optimizer module role: executable entrance. Exact function-fragment emission routes.
//!
//! This map keeps each source custody route visible, then joins ordinary or
//! structural fragment construction to canonical statistics and manifest sealing.

mod allocation_recovery;
mod fixed_frame;
mod manifest;
mod ordinary;
mod ordinary_function;
mod post_allocation_machine;
mod selected_lowering;
mod source_kind;
mod statistics;
mod structural_unit;
mod unit_baseline;
mod x86_rel8;

use omega_machine_code::FunctionFragmentEmissionPlan;

use super::{
    FunctionFragmentEmissionError, StagedOptimizedFunctionFragmentEmissionSource,
    ValidatedFunctionFragmentEmissionManifest,
};

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<
    (
        FunctionFragmentEmissionPlan,
        ValidatedFunctionFragmentEmissionManifest,
    ),
    FunctionFragmentEmissionError,
> {
    match source {
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(realization) => {
            x86_rel8::compute(source, realization)
        }
        StagedOptimizedFunctionFragmentEmissionSource::SelectedLowering(realization) => {
            selected_lowering::compute(source, realization)
        }
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(realization) => {
            post_allocation_machine::compute(source, realization)
        }
        StagedOptimizedFunctionFragmentEmissionSource::AllocationRecovery(realization) => {
            allocation_recovery::compute(source, realization)
        }
        StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(realization) => {
            unit_baseline::compute(source, realization)
        }
        StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(realization) => {
            structural_unit::compute(source, realization)
        }
        StagedOptimizedFunctionFragmentEmissionSource::FixedFrame(realization) => {
            fixed_frame::compute(source, realization)
        }
    }
}
