use crate::StagedOptimizedFunctionFragmentEmissionSource;

/// One completed physical postcondition. Optimization history remains in the
/// retained replay inputs; it does not select the coordinator's result type.
/// No program data is cloned when this result moves into fragment emission.
#[derive(Debug)]
pub struct StagedOptimizedVerifiedPhysicalPipeline {
    pub(super) source: StagedOptimizedFunctionFragmentEmissionSource,
}

impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn into_function_fragment_emission_source(
        self,
    ) -> StagedOptimizedFunctionFragmentEmissionSource {
        self.source
    }
    pub fn pre_physical_manifest(
        &self,
    ) -> &omega_optimization_validation::ValidatedPrePhysicalOptimizationManifest {
        self.source.pre_physical_manifest()
    }
    pub fn post_allocation_manifest(
        &self,
    ) -> &omega_regalloc::ValidatedPostAllocationOptimizationManifest {
        self.source.post_allocation_manifest()
    }
    pub fn machine(&self) -> &crate::StagedOptimizedPostAllocationMachinePlan {
        self.source.machine()
    }
    pub fn function_relative_manifest(
        &self,
    ) -> &crate::ValidatedFunctionRelativeOptimizationRealizationManifest {
        self.source.function_relative_manifest()
    }
    pub fn selections(&self) -> omega_optimization_core::OptimizationSelectionIdentity {
        self.function_relative_manifest().record().selections
    }
    pub fn selected_lowering_completion(
        &self,
    ) -> Option<omega_optimization_core::SelectedLoweringOptimizationCompletionIdentity> {
        self.post_allocation_manifest()
            .record()
            .selected_lowering_completion
    }
    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<&crate::StagedOptimizedPostAllocationMachineOptimization> {
        self.source.post_allocation_machine_optimization()
    }
    #[cfg(test)]
    pub fn selected_lowering_function_relative_realization(
        &self,
    ) -> Option<&crate::StagedSelectedLoweringFunctionRelativeRealization> {
        self.selected_lowering_for_test()
    }
    #[cfg(test)]
    pub fn allocation_recovery_function_relative_realization(
        &self,
    ) -> Option<&crate::StagedAllocationRecoveryFunctionRelativeRealization> {
        self.allocation_recovery_for_test()
    }
}

impl From<crate::StagedOptimizedUnitFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(realization: crate::StagedOptimizedUnitFunctionRelativeRealization) -> Self {
        Self {
            source: StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(Box::new(
                realization,
            )),
        }
    }
}

impl From<crate::StagedOptimizedStructuralUnitFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(realization: crate::StagedOptimizedStructuralUnitFunctionRelativeRealization) -> Self {
        Self {
            source: StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(Box::new(
                realization,
            )),
        }
    }
}

impl From<crate::StagedFixedFrameFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(realization: crate::StagedFixedFrameFunctionRelativeRealization) -> Self {
        Self {
            source: StagedOptimizedFunctionFragmentEmissionSource::FixedFrame(Box::new(
                realization,
            )),
        }
    }
}

impl From<crate::StagedPostAllocationMachineFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(realization: crate::StagedPostAllocationMachineFunctionRelativeRealization) -> Self {
        Self {
            source: StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(
                realization,
            )),
        }
    }
}

impl From<crate::StagedAllocationRecoveryFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(realization: crate::StagedAllocationRecoveryFunctionRelativeRealization) -> Self {
        Self {
            source: StagedOptimizedFunctionFragmentEmissionSource::AllocationRecovery(Box::new(
                realization,
            )),
        }
    }
}

impl From<crate::StagedFunctionRelativeLayoutOptimizationRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(realization: crate::StagedFunctionRelativeLayoutOptimizationRealization) -> Self {
        Self {
            source: StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(
                realization,
            )),
        }
    }
}

impl From<crate::StagedSelectedLoweringFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(realization: crate::StagedSelectedLoweringFunctionRelativeRealization) -> Self {
        Self {
            source: StagedOptimizedFunctionFragmentEmissionSource::SelectedLowering(Box::new(
                realization,
            )),
        }
    }
}
