use super::StagedOptimizedVerifiedPhysicalPipeline;

impl StagedOptimizedVerifiedPhysicalPipeline {
    pub fn fixed_frame_for_test(
        &self,
    ) -> &machine_emission::StagedFixedFrameFunctionRelativeRealization {
        self.source.replay_for_test().fixed_frame()
    }

    pub fn fixed_frame_mut_for_test(
        &mut self,
    ) -> &mut machine_emission::StagedFixedFrameFunctionRelativeRealization {
        self.source.replay_mut().fixed_frame_mut()
    }

    pub fn into_fixed_frame_for_test(
        self,
    ) -> machine_emission::StagedFixedFrameFunctionRelativeRealization {
        self.source.into_replay_for_test().into_fixed_frame()
    }
}
