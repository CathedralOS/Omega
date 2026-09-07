use super::StagedOptimizedVerifiedPhysicalPipeline;
use machine_emission::FunctionFragmentReplayInputs;

impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn into_structural_unit_for_test(
        self,
    ) -> Option<machine_emission::StagedOptimizedStructuralUnitFunctionRelativeRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::StructuralUnit(realization) => Some(*realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn fixed_frame_for_test(
        &self,
    ) -> Option<&machine_emission::StagedFixedFrameFunctionRelativeRealization> {
        match self.source.replay_for_test() {
            FunctionFragmentReplayInputs::FixedFrame(realization) => Some(realization),
            _ => None,
        }
    }
    pub fn into_fixed_frame_for_test(
        self,
    ) -> Option<machine_emission::StagedFixedFrameFunctionRelativeRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::FixedFrame(realization) => Some(*realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn post_allocation_machine_mut_for_test(
        &mut self,
    ) -> Option<&mut machine_emission::StagedPostAllocationMachineFunctionRelativeRealization> {
        match self.source.replay_mut() {
            FunctionFragmentReplayInputs::PostAllocationMachine(realization) => Some(realization),
            _ => None,
        }
    }
    pub fn post_allocation_machine_for_test(
        &self,
    ) -> Option<&machine_emission::StagedPostAllocationMachineFunctionRelativeRealization> {
        match self.source.replay_for_test() {
            FunctionFragmentReplayInputs::PostAllocationMachine(realization) => Some(realization),
            _ => None,
        }
    }
    pub fn into_post_allocation_machine_for_test(
        self,
    ) -> Option<machine_emission::StagedPostAllocationMachineFunctionRelativeRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::PostAllocationMachine(realization) => Some(*realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn fixed_frame_mut_for_test(
        &mut self,
    ) -> Option<&mut machine_emission::StagedFixedFrameFunctionRelativeRealization> {
        match self.source.replay_mut() {
            FunctionFragmentReplayInputs::FixedFrame(realization) => Some(realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn selected_lowering_for_test(
        &self,
    ) -> Option<&machine_emission::StagedSelectedLoweringFunctionRelativeRealization> {
        match self.source.replay_for_test() {
            FunctionFragmentReplayInputs::SelectedLowering(realization) => Some(realization),
            _ => None,
        }
    }
    pub fn into_selected_lowering_for_test(
        self,
    ) -> Option<machine_emission::StagedSelectedLoweringFunctionRelativeRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::SelectedLowering(realization) => Some(*realization),
            _ => None,
        }
    }
}
