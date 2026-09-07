use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::StagedFixedFrameFunctionRelativeRealization;

#[derive(Debug)]
/// Retained inputs for independently replaying the completed realization.
/// These roles are replay inputs only. Current program data are retained separately.
pub struct FunctionFragmentReplayInputs {
    realization: StagedFixedFrameFunctionRelativeRealization,
}

impl From<StagedFixedFrameFunctionRelativeRealization> for FunctionFragmentReplayInputs {
    fn from(realization: StagedFixedFrameFunctionRelativeRealization) -> Self {
        Self { realization }
    }
}
impl FunctionFragmentReplayInputs {
    pub fn fixed_frame(&self) -> &StagedFixedFrameFunctionRelativeRealization {
        &self.realization
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn fixed_frame_mut(&mut self) -> &mut StagedFixedFrameFunctionRelativeRealization {
        &mut self.realization
    }
    pub fn into_fixed_frame(self) -> StagedFixedFrameFunctionRelativeRealization {
        self.realization
    }
}

impl FunctionFragmentReplayInputs {
    fn allocation(&self) -> &selected_instructions_to_register_homes::RetainedAllocation {
        self.realization.allocation()
    }
}

impl FunctionFragmentReplayInputs {
    pub fn machine(
        &self,
    ) -> &register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan {
        self.realization.machine()
    }

    pub fn resolved_layout(&self) -> &machine_code::ResolvedMachineLayout {
        self.layout_optimization().layout()
    }

    pub fn layout_optimization(
        &self,
    ) -> &resolved_layout_to_resolved_layout::ResolvedLayoutOptimization {
        self.realization.layout_optimization()
    }

    /// The canonical frame protocol retained for independent replay.
    pub fn frame_protocol(&self) -> Option<&crate::ValidatedTargetFrameProtocolEncoding> {
        Some(self.realization.protocol())
    }

    /// The target-owned geometry the frame protocol encodes. Present exactly
    /// where [`Self::frame_protocol`] is.
    pub fn frame_layout(&self) -> Option<&crate::frame_layout::ValidatedTargetFrameLayout> {
        Some(self.realization.frame())
    }

    pub fn register_homes(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedRegisterHomes {
        self.allocation().current().homes()
    }

    pub fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.allocation().current().register_environment()
    }

    pub const fn exit_contract(&self) -> &crate::ValidatedWholeFunctionExitContract {
        self.realization.exit_contract()
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &crate::ValidatedFunctionRelativeOptimizationRealizationManifest {
        self.realization.manifest()
    }

    pub fn post_allocation_manifest(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest {
        self.allocation().current().post_allocation_manifest()
    }
}

impl FunctionFragmentReplayInputs {
    pub fn shared_selected_plan(
        &self,
    ) -> std::sync::Arc<selected_instructions::SelectedInstructionPlan> {
        self.allocation()
            .current()
            .selected()
            .shared_selected_plan()
    }

    pub fn target_input_owner(
        &self,
    ) -> &std::sync::Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>
    {
        self.allocation().current().target_input_owner()
    }

    pub fn encoding(
        &self,
    ) -> &post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding
    {
        self.realization.encoding()
    }
}
