use super::FunctionFragmentEmissionError;
use super::replay::FunctionFragmentReplayInputs;
use machine_code::ResolvedMachineProgram;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) struct CurrentFunctionFragmentInput {
    pub(super) program: ResolvedMachineProgram,
    pub(super) machine:
        register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan,
    pub(super) homes: selected_instructions_to_register_homes::ValidatedRegisterHomes,
    pub(super) environment: register_environment::ValidatedTargetRegisterEnvironment,
    pub(super) exit: crate::ValidatedWholeFunctionExitContract,
    pub(super) manifest: crate::ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) post_allocation_manifest:
        selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest,
    pub(super) target_input:
        Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>,
}

impl CurrentFunctionFragmentInput {
    pub(super) fn retain(replay: &FunctionFragmentReplayInputs) -> Self {
        let machine = replay.machine().clone();
        let homes = replay.register_homes().clone();
        Self {
            program: ResolvedMachineProgram {
                selected: replay.shared_selected_plan(),
                homes: homes.shared_plan(),
                effects: machine.effects().shared_plan(),
                machine: machine.machine().shared_plan(),
                encoding: replay.encoding().shared_program(),
                layout: replay.layout_optimization().shared_layout(),
                frame: replay.fixed_frame().frame().shared_plan(),
                protocol: replay.fixed_frame().protocol().shared_plan(),
            },
            machine,
            homes,
            environment: replay.register_environment().clone(),
            exit: replay.exit_contract().clone(),
            manifest: replay.function_relative_manifest().clone(),
            post_allocation_manifest: replay.post_allocation_manifest().clone(),
            target_input: Arc::clone(replay.target_input_owner()),
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
            || self.homes != expected.homes
            || self.environment != expected.environment
            || self.exit != expected.exit
            || self.manifest != expected.manifest
            || self.post_allocation_manifest != expected.post_allocation_manifest
            || !Arc::ptr_eq(&self.target_input, &expected.target_input)
        {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        Ok(())
    }
}
