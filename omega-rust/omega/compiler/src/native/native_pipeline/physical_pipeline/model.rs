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
    ) -> &abstract_operations_to_abstract_operations::validation::ValidatedPrePhysicalOptimizationManifest{
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
