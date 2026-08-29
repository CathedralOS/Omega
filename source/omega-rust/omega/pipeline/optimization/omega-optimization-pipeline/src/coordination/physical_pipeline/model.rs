use omega_optimization_core::OptimizationSelectionIdentity;
use omega_optimization_validation::ValidatedPrePhysicalOptimizationManifest;
use omega_regalloc::ValidatedPostAllocationOptimizationManifest;

use crate::{
    StagedAarch64CbnzFunctionRelativeRealization,
    StagedFunctionRelativeLayoutOptimizationRealization, StagedOptimizedAarch64CbnzFusion,
    StagedOptimizedAarch64MovnFunctionRelativeRealization,
    StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization,
    StagedSelectedLoweringAarch64MovnFunctionRelativeRealization,
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
        realization: StagedAarch64CbnzFunctionRelativeRealization,
    },
    PostAllocationMachineMovn {
        realization: StagedOptimizedAarch64MovnFunctionRelativeRealization,
    },
    SelectedLoweringPostAllocationMachine {
        realization: StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization,
    },
    SelectedLoweringPostAllocationMachineMovn {
        realization: StagedSelectedLoweringAarch64MovnFunctionRelativeRealization,
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
            Self::PostAllocationMachine { realization } => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::PostAllocationMachineMovn { realization } => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::SelectedLoweringPostAllocationMachine { realization } => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::SelectedLoweringPostAllocationMachineMovn { realization } => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
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
            Self::PostAllocationMachine { realization } => {
                realization.homes().post_allocation_manifest()
            }
            Self::PostAllocationMachineMovn { realization } => {
                realization.homes().post_allocation_manifest()
            }
            Self::SelectedLoweringPostAllocationMachine { realization } => {
                realization.homes().post_allocation_manifest()
            }
            Self::SelectedLoweringPostAllocationMachineMovn { realization } => {
                realization.homes().post_allocation_manifest()
            }
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
            Self::PostAllocationMachineMovn { realization } => realization.machine(),
            Self::SelectedLoweringPostAllocationMachine { realization } => realization.machine(),
            Self::SelectedLoweringPostAllocationMachineMovn { realization } => {
                realization.machine()
            }
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
            | Self::PostAllocationMachineMovn { .. }
            | Self::SelectedLoweringPostAllocationMachine { .. }
            | Self::SelectedLoweringPostAllocationMachineMovn { .. }
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
            Self::PostAllocationMachineMovn { realization } => Some(realization.manifest()),
            Self::SelectedLoweringPostAllocationMachine { realization } => {
                Some(realization.manifest())
            }
            Self::SelectedLoweringPostAllocationMachineMovn { realization } => {
                Some(realization.manifest())
            }
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
            Self::PostAllocationMachine { realization } => {
                realization.fusion().custody().selections()
            }
            Self::PostAllocationMachineMovn { realization } => {
                realization.materialization().custody().selections()
            }
            Self::SelectedLoweringPostAllocationMachine { realization } => {
                realization.fusion().custody().selections()
            }
            Self::SelectedLoweringPostAllocationMachineMovn { realization } => {
                realization.materialization().custody().selections()
            }
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
            | Self::PostAllocationMachine { .. }
            | Self::PostAllocationMachineMovn { .. }
            | Self::AllocationRecovery { .. }
            | Self::ActiveResidentRematerialization { .. }
            | Self::FunctionRelativeLayout { .. } => None,
            Self::SelectedLoweringPostAllocationMachine { realization } => Some(
                realization
                    .homes()
                    .selected_lowering_run()
                    .custody()
                    .identity(),
            ),
            Self::SelectedLoweringPostAllocationMachineMovn { realization } => Some(
                realization
                    .homes()
                    .selected_lowering_run()
                    .custody()
                    .identity(),
            ),
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
    ) -> Option<&StagedOptimizedAarch64CbnzFusion> {
        match self {
            Self::PostAllocationMachine { realization } => Some(realization.fusion()),
            Self::SelectedLoweringPostAllocationMachine { realization } => {
                Some(realization.fusion())
            }
            Self::PsiOnly { .. }
            | Self::PostAllocationMachineMovn { .. }
            | Self::SelectedLoweringPostAllocationMachineMovn { .. }
            | Self::AllocationRecovery { .. }
            | Self::ActiveResidentRematerialization { .. }
            | Self::FunctionRelativeLayout { .. }
            | Self::SelectedLowering { .. } => None,
        }
    }

    pub const fn post_allocation_movn_optimization(
        &self,
    ) -> Option<&StagedOptimizedAarch64MovnMaterialization> {
        match self {
            Self::PostAllocationMachineMovn { realization } => Some(realization.materialization()),
            Self::SelectedLoweringPostAllocationMachineMovn { realization } => {
                Some(realization.materialization())
            }
            _ => None,
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
