use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};
use omega_regalloc::ValidatedSelectedAnalysis;

use super::{
    AllocationRecoverySourceKind, StagedAllocationRecoveryFunctionRelativeSource,
    active_resident_selected_stage, fixed_view_selected_stage,
};

impl StagedAllocationRecoveryFunctionRelativeSource {
    pub const fn kind(&self) -> AllocationRecoverySourceKind {
        match self {
            Self::FixedViewCopies(_) => AllocationRecoverySourceKind::FixedViewCopiesV1,
            Self::ActiveResidentRematerialization(_) => {
                AllocationRecoverySourceKind::ActiveResidentRematerializationV1
            }
        }
    }

    pub fn optimized_target(&self) -> &crate::ValidatedOptimizedTargetOperations {
        match self {
            Self::FixedViewCopies(homes) => fixed_view_selected_stage(homes).optimized_target(),
            Self::ActiveResidentRematerialization(source) => {
                active_resident_selected_stage(source).optimized_target()
            }
        }
    }

    pub fn selected_plan(&self) -> &omega_selected_instructions::SelectedInstructionPlan {
        match self {
            Self::FixedViewCopies(homes) => homes
                .reanalysis_stage()
                .transformation_stage()
                .copies()
                .selected_plan(),
            Self::ActiveResidentRematerialization(source) => {
                source.rematerialization().selected_plan()
            }
        }
    }

    pub fn selected_identity(
        &self,
    ) -> omega_selected_instructions::SelectedInstructionPlanIdentity {
        match self {
            Self::FixedViewCopies(homes) => homes
                .reanalysis_stage()
                .transformation_stage()
                .copies()
                .selected_identity(),
            Self::ActiveResidentRematerialization(source) => {
                source.rematerialization().selected_identity()
            }
        }
    }

    pub const fn homes(&self) -> &omega_regalloc::ValidatedRegisterHomes {
        match self {
            Self::FixedViewCopies(homes) => homes.homes(),
            Self::ActiveResidentRematerialization(source) => source.homes(),
        }
    }

    pub const fn post_allocation_manifest(
        &self,
    ) -> &omega_regalloc::ValidatedPostAllocationOptimizationManifest {
        match self {
            Self::FixedViewCopies(homes) => homes.post_allocation_manifest(),
            Self::ActiveResidentRematerialization(source) => source.post_allocation_manifest(),
        }
    }

    pub fn register_environment(&self) -> &crate::ValidatedTargetRegisterEnvironment {
        match self {
            Self::FixedViewCopies(homes) => fixed_view_selected_stage(homes).register_environment(),
            Self::ActiveResidentRematerialization(source) => {
                active_resident_selected_stage(source).register_environment()
            }
        }
    }

    pub fn expected_allocation_recovery_selections(&self) -> OptimizationSelections {
        let optimization = match self {
            Self::FixedViewCopies(_) => {
                Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1
            }
            Self::ActiveResidentRematerialization(_) => {
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1
            }
        };
        OptimizationSelections::new([optimization])
            .expect("one catalogued allocation-recovery rule is valid")
    }

    pub(in crate::stages::realization::allocation_recovery_function_relative_realization) fn validate_phase_selection(
        &self,
    ) -> Result<(), super::super::AllocationRecoveryFunctionRelativeRealizationError> {
        let selections = self.optimized_target().optimized().selections();
        if selections.for_phase(OptimizationExecutionPhase::AllocationRecovery)
            != self.expected_allocation_recovery_selections()
            || [
                OptimizationExecutionPhase::SelectedLowering,
                OptimizationExecutionPhase::PostAllocationMachine,
                OptimizationExecutionPhase::FunctionRelativeLayout,
            ]
            .into_iter()
            .any(|phase| !selections.for_phase(phase).is_empty())
        {
            return Err(super::super::AllocationRecoveryFunctionRelativeRealizationError::UnsupportedSelections);
        }
        Ok(())
    }
}
