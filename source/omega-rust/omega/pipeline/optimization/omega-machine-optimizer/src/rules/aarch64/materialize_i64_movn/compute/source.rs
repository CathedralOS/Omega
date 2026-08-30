use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::SelectedInstructionPlan;
use omega_target::Architecture;

use crate::{
    Aarch64MovnInstructionDisposition, Aarch64MovnMaterializationBlock,
    Aarch64MovnMaterializationError, Aarch64MovnMaterializationFunction,
    Aarch64MovnMaterializationInstruction, PostAllocationMachinePlan,
};

pub(super) fn validate_roots(
    selected: &SelectedInstructionPlan,
    selected_identity: omega_selected_instructions::SelectedInstructionPlanIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: crate::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64MovnMaterializationError> {
    if selected.target.architecture != Architecture::Aarch64
        || source.target.architecture != Architecture::Aarch64
        || physical.model().architecture != Architecture::Aarch64
    {
        return Err(Aarch64MovnMaterializationError::UnsupportedTarget(
            source.target,
        ));
    }
    if source.identity != source_identity
        || source.selected != selected_identity
        || selected.target != source.target
        || source.physical_register_model != physical.identity()
        || selected.functions.len() != source.functions.len()
    {
        return Err(Aarch64MovnMaterializationError::RootMismatch);
    }
    Ok(())
}

pub(super) fn baseline_roster(
    source: &PostAllocationMachinePlan,
) -> Vec<Aarch64MovnMaterializationFunction> {
    source
        .functions
        .iter()
        .map(|function| Aarch64MovnMaterializationFunction {
            machine: function.machine,
            blocks: function
                .blocks
                .iter()
                .map(|block| Aarch64MovnMaterializationBlock {
                    block: block.block,
                    instructions: block
                        .instructions
                        .iter()
                        .map(|instruction| Aarch64MovnMaterializationInstruction {
                            instruction: instruction.instruction,
                            disposition: Aarch64MovnInstructionDisposition::RetainedV1,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}
