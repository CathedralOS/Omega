use super::FunctionFragmentEmissionError;
use super::replay::FunctionFragmentReplayInputs;
use crate::exit_contract::ValidatedWholeFunctionExitContract;
use crate::function_realization::ValidatedFunctionRelativeOptimizationRealizationManifest;
use machine_code::ResolvedMachineProgram;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) struct CurrentFunctionFragmentInput {
    pub(super) program: ResolvedMachineProgram,
    pub(super) machine:
        register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan,
    pub(super) homes: selected_instructions_to_register_homes::ValidatedRegisterHomes,
    pub(super) environment: register_environment::ValidatedTargetRegisterEnvironment,
    pub(super) exit: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
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
        let ResolvedMachineProgram {
            selected,
            homes,
            effects,
            machine,
            encoding,
            layout,
            frame,
            protocol,
        } = &self.program;
        // Compare complete artifacts and admitted facts directly with replay
        // inputs. Retaining a second current carrier would clone the register
        // environment solely to discard it after this comparison.
        if selected != &replay.shared_selected_plan()
            || !matches_shared_plan(homes, replay.register_homes().plan())
            || !matches_shared_plan(effects, replay.machine().effects().plan())
            || !matches_shared_plan(machine, replay.machine().machine().plan())
            || !matches_shared_plan(encoding, replay.encoding().program())
            || !matches_shared_plan(layout, replay.layout_optimization().layout())
            || !matches_shared_plan(frame, replay.fixed_frame().frame().plan())
            || !matches_shared_plan(protocol, replay.fixed_frame().protocol().plan())
            || &self.machine != replay.machine()
            || &self.homes != replay.register_homes()
            || &self.environment != replay.register_environment()
            || &self.exit != replay.exit_contract()
            || &self.manifest != replay.function_relative_manifest()
            || &self.post_allocation_manifest != replay.post_allocation_manifest()
            || !Arc::ptr_eq(&self.target_input, replay.target_input_owner())
        {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        Ok(())
    }
}

/// Preserve Arc equality's shared-owner fast path while borrowing the replay
/// plan. Distinct allocations must still compare their complete contents.
fn matches_shared_plan<Plan: Eq>(current: &Arc<Plan>, replay: &Plan) -> bool {
    std::ptr::eq(current.as_ref(), replay) || current.as_ref() == replay
}
