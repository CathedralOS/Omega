use super::FunctionFragmentEmissionSourceKind;
use omega_regalloc::ValidatedSelectedAnalysis;

use crate::{
    StagedAllocationRecoveryFunctionRelativeRealization,
    StagedFixedFrameFunctionRelativeRealization,
    StagedFunctionRelativeLayoutOptimizationRealization,
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
    StagedOptimizedUnitFunctionRelativeRealization,
    StagedPostAllocationMachineFunctionRelativeRealization,
    StagedSelectedLoweringFunctionRelativeRealization,
};

#[derive(Debug)]
/// Retained inputs for independently replaying the completed realization.
/// These roles are replay inputs only. Current program data are retained separately.
pub(crate) enum FunctionFragmentReplayInputs {
    X86Rel8Direct(Box<StagedFunctionRelativeLayoutOptimizationRealization>),
    SelectedLowering(Box<StagedSelectedLoweringFunctionRelativeRealization>),
    PostAllocationMachine(Box<StagedPostAllocationMachineFunctionRelativeRealization>),
    AllocationRecovery(Box<StagedAllocationRecoveryFunctionRelativeRealization>),
    UnitBaseline(Box<StagedOptimizedUnitFunctionRelativeRealization>),
    StructuralUnit(Box<StagedOptimizedStructuralUnitFunctionRelativeRealization>),
    FixedFrame(Box<StagedFixedFrameFunctionRelativeRealization>),
}

impl FunctionFragmentReplayInputs {
    pub fn source_kind(&self) -> FunctionFragmentEmissionSourceKind {
        match self {
            Self::X86Rel8Direct(_) => FunctionFragmentEmissionSourceKind::X86Rel8V1,
            Self::SelectedLowering(_) => FunctionFragmentEmissionSourceKind::SelectedLoweringV1,
            Self::PostAllocationMachine(realization) => {
                FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                    optimization: realization.optimization().optimization(),
                }
            }
            Self::AllocationRecovery(_) => FunctionFragmentEmissionSourceKind::AllocationRecoveryV1,
            Self::UnitBaseline(_) => FunctionFragmentEmissionSourceKind::UnitBaselineV1,
            Self::StructuralUnit(_) => FunctionFragmentEmissionSourceKind::StructuralUnitV1,
            Self::FixedFrame(_) => FunctionFragmentEmissionSourceKind::CanonicalFixedFrameBodyV1,
        }
    }
}

impl FunctionFragmentReplayInputs {
    pub fn machine(&self) -> &crate::StagedOptimizedPostAllocationMachinePlan {
        match self {
            Self::UnitBaseline(realization) => realization.machine(),
            Self::StructuralUnit(realization) => realization.machine(),
            Self::FixedFrame(realization) => realization.machine(),
            Self::PostAllocationMachine(realization) => realization.machine(),
            Self::AllocationRecovery(realization) => realization.machine(),
            Self::X86Rel8Direct(realization) => realization.machine(),
            Self::SelectedLowering(realization) => realization.machine(),
        }
    }

    pub fn resolved_layout(&self) -> &crate::StagedOptimizedResolvedSelectedFormLayout {
        match self {
            Self::UnitBaseline(realization) => realization.layout(),
            Self::StructuralUnit(realization) => realization.layout(),
            Self::FixedFrame(realization) => realization.layout(),
            Self::PostAllocationMachine(realization) => realization.layout(),
            Self::AllocationRecovery(realization) => realization.layout(),
            Self::X86Rel8Direct(realization) => realization.layout(),
            Self::SelectedLowering(realization) => realization.layout(),
        }
    }

    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<&crate::StagedOptimizedPostAllocationMachineOptimization> {
        match self {
            Self::PostAllocationMachine(realization) => Some(realization.optimization()),
            _ => None,
        }
    }

    pub const fn fixed_frame_realization(
        &self,
    ) -> Option<&StagedFixedFrameFunctionRelativeRealization> {
        match self {
            Self::FixedFrame(realization) => Some(realization),
            _ => None,
        }
    }

    pub fn register_homes(&self) -> &omega_regalloc::ValidatedRegisterHomes {
        match self {
            Self::X86Rel8Direct(realization) => realization.homes().homes(),
            Self::SelectedLowering(realization) => realization.homes().homes(),
            Self::PostAllocationMachine(realization) => realization.allocation().current().homes(),
            Self::AllocationRecovery(realization) => realization.allocation().current().homes(),
            Self::UnitBaseline(realization) => realization.homes().homes(),
            Self::StructuralUnit(realization) => realization.homes().homes(),
            Self::FixedFrame(realization) => realization.homes().homes(),
        }
    }

    pub fn register_environment(&self) -> &crate::ValidatedTargetRegisterEnvironment {
        match self {
            Self::X86Rel8Direct(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::SelectedLowering(realization) => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::PostAllocationMachine(realization) => {
                realization.allocation().current().register_environment()
            }
            Self::AllocationRecovery(realization) => {
                realization.allocation().current().register_environment()
            }
            Self::UnitBaseline(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::StructuralUnit(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::FixedFrame(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
        }
    }

    pub const fn exit_contract(&self) -> &crate::ValidatedWholeFunctionExitContract {
        match self {
            Self::X86Rel8Direct(realization) => realization.exit_contract(),
            Self::SelectedLowering(realization) => realization.exit_contract(),
            Self::PostAllocationMachine(realization) => realization.exit_contract(),
            Self::AllocationRecovery(realization) => realization.exit_contract(),
            Self::UnitBaseline(realization) => realization.exit_contract(),
            Self::StructuralUnit(realization) => realization.exit_contract(),
            Self::FixedFrame(realization) => realization.exit_contract(),
        }
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &crate::ValidatedFunctionRelativeOptimizationRealizationManifest {
        match self {
            Self::X86Rel8Direct(realization) => realization.manifest(),
            Self::SelectedLowering(realization) => realization.manifest(),
            Self::PostAllocationMachine(realization) => realization.manifest(),
            Self::AllocationRecovery(realization) => realization.manifest(),
            Self::UnitBaseline(realization) => realization.manifest(),
            Self::StructuralUnit(realization) => realization.manifest(),
            Self::FixedFrame(realization) => realization.manifest(),
        }
    }

    pub fn post_allocation_manifest(
        &self,
    ) -> &omega_regalloc::ValidatedPostAllocationOptimizationManifest {
        match self {
            Self::X86Rel8Direct(realization) => realization.homes().post_allocation_manifest(),
            Self::SelectedLowering(realization) => realization.homes().post_allocation_manifest(),
            Self::PostAllocationMachine(realization) => realization
                .allocation()
                .current()
                .post_allocation_manifest(),
            Self::AllocationRecovery(realization) => realization
                .allocation()
                .current()
                .post_allocation_manifest(),
            Self::UnitBaseline(realization) => realization.homes().post_allocation_manifest(),
            Self::StructuralUnit(realization) => realization.homes().post_allocation_manifest(),
            Self::FixedFrame(realization) => realization.homes().post_allocation_manifest(),
        }
    }
}

impl FunctionFragmentReplayInputs {
    pub fn shared_selected_plan(
        &self,
    ) -> std::sync::Arc<omega_selected_instructions::SelectedInstructionPlan> {
        match self {
            Self::X86Rel8Direct(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .shared_selected_plan(),
            Self::SelectedLowering(realization) => {
                shared_selected_after_lowering(realization.homes())
            }
            Self::PostAllocationMachine(realization) => realization
                .allocation()
                .current()
                .selected()
                .shared_selected_plan(),
            Self::AllocationRecovery(realization) => realization
                .allocation()
                .current()
                .selected()
                .shared_selected_plan(),
            Self::UnitBaseline(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .shared_selected_plan(),
            Self::StructuralUnit(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .shared_selected_plan(),
            Self::FixedFrame(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .shared_selected_plan(),
        }
    }

    pub fn target_input_owner(&self) -> &std::sync::Arc<crate::ValidatedOptimizedTargetOperations> {
        match self {
            Self::X86Rel8Direct(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target_owner(),
            Self::SelectedLowering(realization) => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target_owner(),
            Self::PostAllocationMachine(realization) => {
                realization.allocation().current().target_input_owner()
            }
            Self::AllocationRecovery(realization) => {
                realization.allocation().current().target_input_owner()
            }
            Self::UnitBaseline(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target_owner(),
            Self::StructuralUnit(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target_owner(),
            Self::FixedFrame(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target_owner(),
        }
    }

    pub fn encoding(&self) -> &crate::StagedOptimizedSelectedFormEncoding {
        match self {
            Self::X86Rel8Direct(realization) => realization.encoding(),
            Self::SelectedLowering(realization) => realization.encoding(),
            Self::PostAllocationMachine(realization) => realization.encoding(),
            Self::AllocationRecovery(realization) => realization.encoding(),
            Self::UnitBaseline(realization) => realization.encoding(),
            Self::StructuralUnit(realization) => realization.encoding(),
            Self::FixedFrame(realization) => realization.encoding(),
        }
    }
}
fn shared_selected_after_lowering(
    homes: &crate::StagedOptimizedRegisterHomesAfterSelectedLowering,
) -> std::sync::Arc<omega_selected_instructions::SelectedInstructionPlan> {
    let run = homes.selected_lowering_run();
    match run.steps().last() {
        Some(step) => step.fold().shared_selected_plan(),
        None => run
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .selected()
            .shared_selected_plan(),
    }
}
