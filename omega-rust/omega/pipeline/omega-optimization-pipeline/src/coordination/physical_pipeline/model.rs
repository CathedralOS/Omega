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
    /// Consume the completed physical route into the one fragment-emission
    /// entrance. Route identity remains explicit in the source discriminant;
    /// callers cannot reclassify one realization as another.
    pub fn into_function_fragment_emission_source(
        self,
    ) -> crate::StagedOptimizedFunctionFragmentEmissionSource {
        match self {
            Self::UnitBaseline { realization } => {
                crate::StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(Box::new(
                    realization,
                ))
            }
            Self::StructuralUnit { realization } => {
                crate::StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(Box::new(
                    realization,
                ))
            }
            Self::FixedFrame { realization } => {
                crate::StagedOptimizedFunctionFragmentEmissionSource::FixedFrame(Box::new(
                    realization,
                ))
            }
            Self::PostAllocationMachine { realization } => {
                crate::StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(
                    Box::new(realization),
                )
            }
            Self::AllocationRecovery { realization } => {
                crate::StagedOptimizedFunctionFragmentEmissionSource::AllocationRecovery(
                    realization,
                )
            }
            Self::FunctionRelativeLayout { realization } => {
                crate::StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(
                    realization,
                ))
            }
            Self::SelectedLowering { realization } => {
                crate::StagedOptimizedFunctionFragmentEmissionSource::SelectedLowering(Box::new(
                    realization,
                ))
            }
        }
    }

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
            Self::PostAllocationMachine { realization } => realization
                .allocation()
                .current()
                .target_input()
                .optimized()
                .pre_physical_manifest(),
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

    pub fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        match self {
            Self::UnitBaseline { realization } => realization.homes().post_allocation_manifest(),
            Self::StructuralUnit { realization } => realization.homes().post_allocation_manifest(),
            Self::FixedFrame { realization } => realization.homes().post_allocation_manifest(),
            Self::PostAllocationMachine { realization } => realization
                .allocation()
                .current()
                .post_allocation_manifest(),
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

    pub fn selected_lowering_completion(
        &self,
    ) -> Option<omega_optimization_core::SelectedLoweringOptimizationCompletionIdentity> {
        match self {
            Self::UnitBaseline { .. }
            | Self::StructuralUnit { .. }
            | Self::FixedFrame { .. }
            | Self::AllocationRecovery { .. }
            | Self::FunctionRelativeLayout { .. } => None,
            Self::PostAllocationMachine { realization } => {
                realization
                    .allocation()
                    .current()
                    .post_allocation_manifest()
                    .record()
                    .selected_lowering_completion
            }
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
