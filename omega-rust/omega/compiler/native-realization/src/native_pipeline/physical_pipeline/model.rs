use machine_emission::StagedOptimizedFunctionFragmentEmissionSource;

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
    ) -> &optimization_validation::ValidatedPrePhysicalOptimizationManifest {
        self.source.pre_physical_manifest()
    }
    pub fn post_allocation_manifest(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest {
        self.source.post_allocation_manifest()
    }
    pub fn machine(
        &self,
    ) -> &register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan {
        self.source.machine()
    }
    pub fn function_relative_manifest(
        &self,
    ) -> &machine_emission::ValidatedFunctionRelativeOptimizationRealizationManifest {
        self.source.function_relative_manifest()
    }
    pub fn selections(&self) -> optimization_core::OptimizationSelectionIdentity {
        self.function_relative_manifest().record().selections
    }
    pub fn selected_lowering_completion(
        &self,
    ) -> Option<optimization_core::SelectedLoweringOptimizationCompletionIdentity> {
        self.post_allocation_manifest()
            .record()
            .selected_lowering_completion
    }
    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<&post_allocation_machine_to_post_allocation_machine::StagedOptimizedPostAllocationMachineOptimization>{
        self.source.post_allocation_machine_optimization()
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn selected_lowering_function_relative_realization(
        &self,
    ) -> Option<&machine_emission::StagedSelectedLoweringFunctionRelativeRealization> {
        self.selected_lowering_for_test()
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn allocation_recovery_function_relative_realization(
        &self,
    ) -> Option<&machine_emission::StagedAllocationRecoveryFunctionRelativeRealization> {
        self.allocation_recovery_for_test()
    }
}

impl From<machine_emission::StagedOptimizedUnitFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(realization: machine_emission::StagedOptimizedUnitFunctionRelativeRealization) -> Self {
        Self {
            source: realization.into(),
        }
    }
}

impl From<machine_emission::StagedOptimizedStructuralUnitFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(
        realization: machine_emission::StagedOptimizedStructuralUnitFunctionRelativeRealization,
    ) -> Self {
        Self {
            source: realization.into(),
        }
    }
}

impl From<machine_emission::StagedFixedFrameFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(realization: machine_emission::StagedFixedFrameFunctionRelativeRealization) -> Self {
        Self {
            source: realization.into(),
        }
    }
}

impl From<machine_emission::StagedPostAllocationMachineFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(
        realization: machine_emission::StagedPostAllocationMachineFunctionRelativeRealization,
    ) -> Self {
        Self {
            source: realization.into(),
        }
    }
}

impl From<machine_emission::StagedAllocationRecoveryFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(
        realization: machine_emission::StagedAllocationRecoveryFunctionRelativeRealization,
    ) -> Self {
        Self {
            source: realization.into(),
        }
    }
}

impl From<machine_emission::StagedFunctionRelativeLayoutOptimizationRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(
        realization: machine_emission::StagedFunctionRelativeLayoutOptimizationRealization,
    ) -> Self {
        Self {
            source: realization.into(),
        }
    }
}

impl From<machine_emission::StagedSelectedLoweringFunctionRelativeRealization>
    for StagedOptimizedVerifiedPhysicalPipeline
{
    fn from(
        realization: machine_emission::StagedSelectedLoweringFunctionRelativeRealization,
    ) -> Self {
        Self {
            source: realization.into(),
        }
    }
}
