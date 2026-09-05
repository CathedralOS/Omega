use super::StagedOptimizedVerifiedPhysicalPipeline;
use crate::FunctionFragmentReplayInputs;

impl StagedOptimizedVerifiedPhysicalPipeline {
    pub(crate) fn into_structural_unit_for_test(
        self,
    ) -> Option<crate::StagedOptimizedStructuralUnitFunctionRelativeRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::StructuralUnit(realization) => Some(*realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub(crate) fn fixed_frame_for_test(
        &self,
    ) -> Option<&crate::StagedFixedFrameFunctionRelativeRealization> {
        match self.source.replay() {
            FunctionFragmentReplayInputs::FixedFrame(realization) => Some(realization),
            _ => None,
        }
    }
    pub(crate) fn into_fixed_frame_for_test(
        self,
    ) -> Option<crate::StagedFixedFrameFunctionRelativeRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::FixedFrame(realization) => Some(*realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub(crate) fn post_allocation_machine_mut_for_test(
        &mut self,
    ) -> Option<&mut crate::StagedPostAllocationMachineFunctionRelativeRealization> {
        match self.source.replay_mut() {
            FunctionFragmentReplayInputs::PostAllocationMachine(realization) => Some(realization),
            _ => None,
        }
    }
    pub(crate) fn post_allocation_machine_for_test(
        &self,
    ) -> Option<&crate::StagedPostAllocationMachineFunctionRelativeRealization> {
        match self.source.replay() {
            FunctionFragmentReplayInputs::PostAllocationMachine(realization) => Some(realization),
            _ => None,
        }
    }
    pub(crate) fn into_post_allocation_machine_for_test(
        self,
    ) -> Option<crate::StagedPostAllocationMachineFunctionRelativeRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::PostAllocationMachine(realization) => Some(*realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub(crate) fn allocation_recovery_for_test(
        &self,
    ) -> Option<&crate::StagedAllocationRecoveryFunctionRelativeRealization> {
        match self.source.replay() {
            FunctionFragmentReplayInputs::AllocationRecovery(realization) => Some(realization),
            _ => None,
        }
    }
    pub(crate) fn into_allocation_recovery_for_test(
        self,
    ) -> Option<Box<crate::StagedAllocationRecoveryFunctionRelativeRealization>> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::AllocationRecovery(realization) => Some(realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub(crate) fn function_relative_layout_mut_for_test(
        &mut self,
    ) -> Option<&mut crate::StagedFunctionRelativeLayoutOptimizationRealization> {
        match self.source.replay_mut() {
            FunctionFragmentReplayInputs::X86Rel8Direct(realization) => Some(realization),
            _ => None,
        }
    }

    pub(crate) fn into_function_relative_layout_for_test(
        self,
    ) -> Option<crate::StagedFunctionRelativeLayoutOptimizationRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::X86Rel8Direct(realization) => Some(*realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub(crate) fn selected_lowering_for_test(
        &self,
    ) -> Option<&crate::StagedSelectedLoweringFunctionRelativeRealization> {
        match self.source.replay() {
            FunctionFragmentReplayInputs::SelectedLowering(realization) => Some(realization),
            _ => None,
        }
    }
    pub(crate) fn into_selected_lowering_for_test(
        self,
    ) -> Option<crate::StagedSelectedLoweringFunctionRelativeRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::SelectedLowering(realization) => Some(*realization),
            _ => None,
        }
    }
}
