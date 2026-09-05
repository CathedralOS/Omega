use super::StagedOptimizedVerifiedPhysicalPipeline;
use omega_machine_emission::FunctionFragmentReplayInputs;

impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn into_structural_unit_for_test(
        self,
    ) -> Option<omega_machine_emission::StagedOptimizedStructuralUnitFunctionRelativeRealization>
    {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::StructuralUnit(realization) => Some(*realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn fixed_frame_for_test(
        &self,
    ) -> Option<&omega_machine_emission::StagedFixedFrameFunctionRelativeRealization> {
        match self.source.replay_for_test() {
            FunctionFragmentReplayInputs::FixedFrame(realization) => Some(realization),
            _ => None,
        }
    }
    pub fn into_fixed_frame_for_test(
        self,
    ) -> Option<omega_machine_emission::StagedFixedFrameFunctionRelativeRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::FixedFrame(realization) => Some(*realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn post_allocation_machine_mut_for_test(
        &mut self,
    ) -> Option<&mut omega_machine_emission::StagedPostAllocationMachineFunctionRelativeRealization>
    {
        match self.source.replay_mut() {
            FunctionFragmentReplayInputs::PostAllocationMachine(realization) => Some(realization),
            _ => None,
        }
    }
    pub fn post_allocation_machine_for_test(
        &self,
    ) -> Option<&omega_machine_emission::StagedPostAllocationMachineFunctionRelativeRealization>
    {
        match self.source.replay_for_test() {
            FunctionFragmentReplayInputs::PostAllocationMachine(realization) => Some(realization),
            _ => None,
        }
    }
    pub fn into_post_allocation_machine_for_test(
        self,
    ) -> Option<omega_machine_emission::StagedPostAllocationMachineFunctionRelativeRealization>
    {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::PostAllocationMachine(realization) => Some(*realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn allocation_recovery_for_test(
        &self,
    ) -> Option<&omega_machine_emission::StagedAllocationRecoveryFunctionRelativeRealization> {
        match self.source.replay_for_test() {
            FunctionFragmentReplayInputs::AllocationRecovery(realization) => Some(realization),
            _ => None,
        }
    }
    pub fn into_allocation_recovery_for_test(
        self,
    ) -> Option<Box<omega_machine_emission::StagedAllocationRecoveryFunctionRelativeRealization>>
    {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::AllocationRecovery(realization) => Some(realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn function_relative_layout_mut_for_test(
        &mut self,
    ) -> Option<&mut omega_machine_emission::StagedFunctionRelativeLayoutOptimizationRealization>
    {
        match self.source.replay_mut() {
            FunctionFragmentReplayInputs::X86Rel8Direct(realization) => Some(realization),
            _ => None,
        }
    }

    pub fn into_function_relative_layout_for_test(
        self,
    ) -> Option<omega_machine_emission::StagedFunctionRelativeLayoutOptimizationRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::X86Rel8Direct(realization) => Some(*realization),
            _ => None,
        }
    }
}
impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn selected_lowering_for_test(
        &self,
    ) -> Option<&omega_machine_emission::StagedSelectedLoweringFunctionRelativeRealization> {
        match self.source.replay_for_test() {
            FunctionFragmentReplayInputs::SelectedLowering(realization) => Some(realization),
            _ => None,
        }
    }
    pub fn into_selected_lowering_for_test(
        self,
    ) -> Option<omega_machine_emission::StagedSelectedLoweringFunctionRelativeRealization> {
        match self.source.into_replay_for_test() {
            FunctionFragmentReplayInputs::SelectedLowering(realization) => Some(*realization),
            _ => None,
        }
    }
}
