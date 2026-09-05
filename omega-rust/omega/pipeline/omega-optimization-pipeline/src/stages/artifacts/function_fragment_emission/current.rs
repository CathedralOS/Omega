use super::replay::FunctionFragmentReplayInputs;
use super::{FunctionFragmentEmissionError, FunctionFragmentEmissionSourceKind};
use omega_machine_code::ResolvedMachineProgram;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) struct CurrentFunctionFragmentInput {
    pub(super) program: ResolvedMachineProgram,
    pub(super) machine: crate::StagedOptimizedPostAllocationMachinePlan,
    pub(super) layout: crate::StagedOptimizedResolvedSelectedFormLayout,
    pub(super) homes: omega_regalloc::ValidatedRegisterHomes,
    pub(super) environment: crate::ValidatedTargetRegisterEnvironment,
    pub(super) encoding: crate::StagedOptimizedSelectedFormEncoding,
    pub(super) frame_protocol: Option<crate::ValidatedTargetFrameProtocolEncoding>,
    pub(super) exit: crate::ValidatedWholeFunctionExitContract,
    pub(super) manifest: crate::ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) post_allocation_manifest:
        omega_regalloc::ValidatedPostAllocationOptimizationManifest,
    pub(super) target_input: Arc<crate::ValidatedOptimizedTargetOperations>,
    pub(super) source_kind: FunctionFragmentEmissionSourceKind,
}

impl CurrentFunctionFragmentInput {
    pub(super) fn retain(replay: &FunctionFragmentReplayInputs) -> Self {
        let machine = replay.machine().clone();
        let layout = replay.resolved_layout().clone();
        let homes = replay.register_homes().clone();
        Self {
            program: ResolvedMachineProgram {
                selected: replay.shared_selected_plan(),
                homes: homes.shared_plan(),
                effects: machine.effects().shared_plan(),
                machine: machine.machine().shared_plan(),
                layout: layout.shared_program(),
            },
            machine,
            layout,
            homes,
            environment: replay.register_environment().clone(),
            encoding: replay.encoding().clone(),
            frame_protocol: replay
                .fixed_frame_realization()
                .map(|value| value.protocol().clone()),
            exit: replay.exit_contract().clone(),
            manifest: replay.function_relative_manifest().clone(),
            post_allocation_manifest: replay.post_allocation_manifest().clone(),
            target_input: Arc::clone(replay.target_input_owner()),
            source_kind: replay.source_kind(),
        }
    }

    pub(super) fn validate_against(
        &self,
        replay: &FunctionFragmentReplayInputs,
    ) -> Result<(), FunctionFragmentEmissionError> {
        let expected = Self::retain(replay);
        // Compare complete artifacts and admitted facts, not just rehashable IDs.
        if self.program != expected.program
            || self.machine != expected.machine
            || self.layout != expected.layout
            || self.homes != expected.homes
            || self.environment != expected.environment
            || self.encoding != expected.encoding
            || self.frame_protocol != expected.frame_protocol
            || self.exit != expected.exit
            || self.manifest != expected.manifest
            || self.post_allocation_manifest != expected.post_allocation_manifest
            || !Arc::ptr_eq(&self.target_input, &expected.target_input)
            || self.source_kind != expected.source_kind
        {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        Ok(())
    }
}
