use super::super::{
    FunctionFragmentEmissionSourceKind, StagedOptimizedFunctionFragmentEmissionSource,
};

pub(super) fn of(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> FunctionFragmentEmissionSourceKind {
    match source {
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(_) => {
            FunctionFragmentEmissionSourceKind::X86Rel8V1
        }
        StagedOptimizedFunctionFragmentEmissionSource::SelectedLowering(_) => {
            FunctionFragmentEmissionSourceKind::SelectedLoweringV1
        }
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(realization) => {
            FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                optimization: realization.optimization().optimization(),
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::AllocationRecovery(_) => {
            FunctionFragmentEmissionSourceKind::AllocationRecoveryV1
        }
        StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(_) => {
            FunctionFragmentEmissionSourceKind::UnitBaselineV1
        }
        StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(_) => {
            FunctionFragmentEmissionSourceKind::StructuralUnitV1
        }
    }
}
