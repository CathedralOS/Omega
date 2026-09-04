use omega_optimization_core::OptimizationSelectionIdentity;
use omega_optimization_validation::ValidatedPrePhysicalOptimizationManifest;
use omega_regalloc::ValidatedPostAllocationOptimizationManifest;

use crate::{
    StagedAllocationRecoveryFunctionRelativeRealization,
    StagedFixedFrameFunctionRelativeRealization,
    StagedFunctionRelativeLayoutOptimizationRealization,
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
    StagedOptimizedUnitFunctionRelativeRealization,
    StagedPostAllocationMachineFunctionRelativeRealization,
    StagedPostAllocationMachineFunctionRelativeSource,
    StagedSelectedLoweringFunctionRelativeRealization,
    ValidatedFunctionRelativeOptimizationRealizationManifest,
};

/// Complete currently admitted function-relative physical realization for one
/// explicitly selected optimized source. Every variant owns a validated
/// function-relative manifest and stops before fragment emission, object/image
/// construction, installation, or publication.
#[derive(Debug)]
pub enum StagedOptimizedVerifiedPhysicalPipeline {
    UnitBaseline {
        realization: StagedOptimizedUnitFunctionRelativeRealization,
    },
    StructuralUnit {
        realization: StagedOptimizedStructuralUnitFunctionRelativeRealization,
    },
    FixedFrame {
        realization: StagedFixedFrameFunctionRelativeRealization,
    },
    PostAllocationMachine {
        realization: StagedPostAllocationMachineFunctionRelativeRealization,
    },
    AllocationRecovery {
        realization: Box<StagedAllocationRecoveryFunctionRelativeRealization>,
    },
    FunctionRelativeLayout {
        realization: StagedFunctionRelativeLayoutOptimizationRealization,
    },
    SelectedLowering {
        realization: StagedSelectedLoweringFunctionRelativeRealization,
    },
}

impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn pre_physical_manifest(&self) -> &ValidatedPrePhysicalOptimizationManifest {
        match self {
            Self::UnitBaseline { realization } => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::StructuralUnit { realization } => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::FixedFrame { realization } => realization
                .homes()
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
                StagedPostAllocationMachineFunctionRelativeSource::AfterAllocationRecovery(
                    source,
                ) => source
                    .optimized_target()
                    .optimized()
                    .pre_physical_manifest(),
            },
            Self::AllocationRecovery { realization } => realization
                .source()
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
            Self::UnitBaseline { realization } => realization.homes().post_allocation_manifest(),
            Self::StructuralUnit { realization } => realization.homes().post_allocation_manifest(),
            Self::FixedFrame { realization } => realization.homes().post_allocation_manifest(),
            Self::PostAllocationMachine { realization } => match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => {
                    homes.post_allocation_manifest()
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    homes.post_allocation_manifest()
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterAllocationRecovery(
                    source,
                ) => source.post_allocation_manifest(),
            },
            Self::AllocationRecovery { realization } => {
                realization.source().post_allocation_manifest()
            }
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
            Self::UnitBaseline { realization } => realization.machine(),
            Self::StructuralUnit { realization } => realization.machine(),
            Self::FixedFrame { realization } => realization.machine(),
            Self::PostAllocationMachine { realization } => realization.machine(),
            Self::AllocationRecovery { realization } => realization.machine(),
            Self::FunctionRelativeLayout { realization } => realization.machine(),
            Self::SelectedLowering { realization } => realization.machine(),
        }
    }

    pub const fn selected_lowering_function_relative_realization(
        &self,
    ) -> Option<&StagedSelectedLoweringFunctionRelativeRealization> {
        match self {
            Self::UnitBaseline { .. }
            | Self::StructuralUnit { .. }
            | Self::FixedFrame { .. }
            | Self::PostAllocationMachine { .. }
            | Self::AllocationRecovery { .. }
            | Self::FunctionRelativeLayout { .. } => None,
            Self::SelectedLowering { realization } => Some(realization),
        }
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        match self {
            Self::UnitBaseline { realization } => realization.manifest(),
            Self::StructuralUnit { realization } => realization.manifest(),
            Self::FixedFrame { realization } => realization.manifest(),
            Self::AllocationRecovery { realization } => realization.manifest(),
            Self::PostAllocationMachine { realization } => realization.manifest(),
            Self::FunctionRelativeLayout { realization } => realization.manifest(),
            Self::SelectedLowering { realization } => realization.manifest(),
        }
    }

    pub fn selections(&self) -> OptimizationSelectionIdentity {
        match self {
            Self::UnitBaseline { realization } => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .selections()
                .identity(),
            Self::StructuralUnit { realization } => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .selections()
                .identity(),
            Self::FixedFrame { realization } => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .selections()
                .identity(),
            Self::PostAllocationMachine { realization } => realization.optimization().selections(),
            Self::AllocationRecovery { realization } => realization
                .source()
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
            Self::UnitBaseline { .. }
            | Self::StructuralUnit { .. }
            | Self::FixedFrame { .. }
            | Self::AllocationRecovery { .. }
            | Self::FunctionRelativeLayout { .. } => None,
            Self::PostAllocationMachine { realization } => match realization.source() {
                StagedPostAllocationMachineFunctionRelativeSource::Direct(_) => None,
                StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
                    Some(homes.selected_lowering_run().custody().identity())
                }
                StagedPostAllocationMachineFunctionRelativeSource::AfterAllocationRecovery(_) => {
                    None
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
            Self::UnitBaseline { .. }
            | Self::StructuralUnit { .. }
            | Self::FixedFrame { .. }
            | Self::AllocationRecovery { .. }
            | Self::FunctionRelativeLayout { .. }
            | Self::SelectedLowering { .. } => None,
        }
    }

    pub const fn allocation_recovery_function_relative_realization(
        &self,
    ) -> Option<&StagedAllocationRecoveryFunctionRelativeRealization> {
        match self {
            Self::AllocationRecovery { realization } => Some(realization),
            _ => None,
        }
    }
}
