use omega_optimization_core::OptimizationSelectionIdentity;
use omega_optimization_validation::ValidatedPrePhysicalOptimizationManifest;
use omega_regalloc::ValidatedPostAllocationOptimizationManifest;

use crate::{
    StagedFunctionRelativeLayoutOptimizationRealization,
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
    StagedPostAllocationMachineFunctionRelativeRealization,
    StagedPostAllocationMachineFunctionRelativeSource,
    StagedSelectedLoweringFunctionRelativeRealization,
    ValidatedFunctionRelativeOptimizationRealizationManifest,
};

/// Complete currently admitted physical validation for one explicitly selected
/// optimized source. All variants stop before frame construction, machine
/// emission, object construction, installation, or publication.
#[derive(Debug)]
pub enum StagedOptimizedVerifiedPhysicalPipeline {
    PsiOnly {
        homes: StagedOptimizedRegisterHomes,
        machine: StagedOptimizedPostAllocationMachinePlan,
    },
    PostAllocationMachine {
        realization: StagedPostAllocationMachineFunctionRelativeRealization,
    },
    AllocationRecovery {
        homes: StagedOptimizedRegisterHomesAfterFixedViewCopies,
        machine: StagedOptimizedPostAllocationMachinePlan,
    },
    ActiveResidentRematerialization {
        realization: Box<StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization>,
    },
    FunctionRelativeLayout {
        realization: StagedFunctionRelativeLayoutOptimizationRealization,
    },
    SelectedLowering {
        realization: StagedSelectedLoweringFunctionRelativeRealization,
    },
}

impl StagedOptimizedVerifiedPhysicalPipeline {
    pub const fn pre_physical_manifest(&self) -> &ValidatedPrePhysicalOptimizationManifest {
        match self {
            Self::PsiOnly { homes, .. } => homes
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::PostAllocationMachine { realization } => match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => homes
                    .legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .optimized_target()
                    .optimized()
                    .pre_physical_manifest(),
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    homes
                        .selected_lowering_run()
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage()
                        .optimized_target()
                        .optimized()
                        .pre_physical_manifest()
                }
            },
            Self::AllocationRecovery { homes, .. } => homes
                .reanalysis_stage()
                .transformation_stage()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::ActiveResidentRematerialization { realization } => realization
                .source()
                .pre_layout()
                .source()
                .source()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::FunctionRelativeLayout { realization } => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::SelectedLowering { realization } => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
        }
    }

    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        match self {
            Self::PsiOnly { homes, .. } => homes.post_allocation_manifest(),
            Self::PostAllocationMachine { realization } => match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => {
                    homes.post_allocation_manifest()
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    homes.post_allocation_manifest()
                }
            },
            Self::AllocationRecovery { homes, .. } => homes.post_allocation_manifest(),
            Self::ActiveResidentRematerialization { realization } => realization
                .source()
                .pre_layout()
                .source()
                .post_allocation_manifest(),
            Self::FunctionRelativeLayout { realization } => {
                realization.homes().post_allocation_manifest()
            }
            Self::SelectedLowering { realization } => {
                realization.homes().post_allocation_manifest()
            }
        }
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        match self {
            Self::PsiOnly { machine, .. } => machine,
            Self::PostAllocationMachine { realization } => realization.machine(),
            Self::AllocationRecovery { machine, .. } => machine,
            Self::ActiveResidentRematerialization { realization } => {
                realization.source().pre_layout().machine()
            }
            Self::FunctionRelativeLayout { realization } => realization.machine(),
            Self::SelectedLowering { realization } => realization.machine(),
        }
    }

    pub const fn function_relative_realization(
        &self,
    ) -> Option<&StagedSelectedLoweringFunctionRelativeRealization> {
        match self {
            Self::PsiOnly { .. }
            | Self::PostAllocationMachine { .. }
            | Self::AllocationRecovery { .. }
            | Self::ActiveResidentRematerialization { .. }
            | Self::FunctionRelativeLayout { .. } => None,
            Self::SelectedLowering { realization } => Some(realization),
        }
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> Option<&ValidatedFunctionRelativeOptimizationRealizationManifest> {
        match self {
            Self::PsiOnly { .. } | Self::AllocationRecovery { .. } => None,
            Self::ActiveResidentRematerialization { realization } => Some(realization.manifest()),
            Self::PostAllocationMachine { realization } => Some(realization.manifest()),
            Self::FunctionRelativeLayout { realization } => Some(realization.manifest()),
            Self::SelectedLowering { realization } => Some(realization.manifest()),
        }
    }

    pub fn selections(&self) -> OptimizationSelectionIdentity {
        match self {
            Self::PsiOnly { homes, .. } => homes
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .selections()
                .identity(),
            Self::PostAllocationMachine { realization } => realization.optimization().selections(),
            Self::AllocationRecovery { homes, .. } => homes
                .reanalysis_stage()
                .transformation_stage()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .selections()
                .identity(),
            Self::ActiveResidentRematerialization { realization } => realization
                .source()
                .pre_layout()
                .source()
                .source()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .selections()
                .identity(),
            Self::FunctionRelativeLayout { realization } => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .selections()
                .identity(),
            Self::SelectedLowering { realization } => realization
                .homes()
                .selected_lowering_run()
                .custody()
                .selections(),
        }
    }

    pub const fn selected_lowering_completion(
        &self,
    ) -> Option<omega_optimization_core::SelectedLoweringOptimizationCompletionIdentity> {
        match self {
            Self::PsiOnly { .. }
            | Self::AllocationRecovery { .. }
            | Self::ActiveResidentRematerialization { .. }
            | Self::FunctionRelativeLayout { .. } => None,
            Self::PostAllocationMachine { realization } => match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(_) => None,
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    Some(homes.selected_lowering_run().custody().identity())
                }
            },
            Self::SelectedLowering { realization } => Some(
                realization
                    .homes()
                    .selected_lowering_run()
                    .custody()
                    .identity(),
            ),
        }
    }

    pub const fn post_allocation_machine_optimization(
        &self,
    ) -> Option<&StagedOptimizedPostAllocationMachineOptimization> {
        match self {
            Self::PostAllocationMachine { realization } => Some(realization.optimization()),
            Self::PsiOnly { .. }
            | Self::AllocationRecovery { .. }
            | Self::ActiveResidentRematerialization { .. }
            | Self::FunctionRelativeLayout { .. }
            | Self::SelectedLowering { .. } => None,
        }
    }

    pub const fn active_resident_rematerialization_function_relative_realization(
        &self,
    ) -> Option<&StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization> {
        match self {
            Self::ActiveResidentRematerialization { realization } => Some(realization),
            _ => None,
        }
    }
}
