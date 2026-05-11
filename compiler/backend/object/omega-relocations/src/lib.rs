use omega_calling_conventions::HostAbiPlan;
use omega_core::arena::{Arena, Handle};
use omega_core::diagnostics::Diagnostic;
use omega_machine_bytes::EncodedMachinePlan;
use omega_object::{ObjectPlan, RelocationPlan};
use omega_target::NativeTarget;
use omega_target_operations::{FunctionInstructionPlan, InstructionPlan, TargetDataPlan};

mod data_addresses;
mod instruction_records;
mod lookups;
mod offsets;

use instruction_records::collect_instruction_relocations;
use lookups::selected_instruction_text_offset;

#[derive(Debug, Clone, Copy)]
pub struct RelocationPlanningInput<'plan> {
    pub target: NativeTarget,
    pub instructions: &'plan InstructionPlan,
    pub encoded_machine: &'plan EncodedMachinePlan,
    pub data: &'plan TargetDataPlan,
    pub object: &'plan ObjectPlan,
    pub host_abi: &'plan HostAbiPlan,
    pub entry_machine_name: &'plan str,
}

pub fn build_relocation_plan(
    input: RelocationPlanningInput<'_>,
) -> Result<RelocationPlan, Diagnostic> {
    let mut relocation_plan = RelocationPlan {
        target: input.target,
        records: Arena::new(),
    };

    for (function_handle, function) in input.instructions.functions.iter() {
        collect_function_relocations(input, function_handle, function, &mut relocation_plan)?;
    }

    Ok(relocation_plan)
}

fn collect_function_relocations(
    input: RelocationPlanningInput<'_>,
    function_handle: Handle<FunctionInstructionPlan>,
    function: &FunctionInstructionPlan,
    relocation_plan: &mut RelocationPlan,
) -> Result<(), Diagnostic> {
    let Some(instructions) = input.instructions.instructions.span(function.instructions) else {
        return Ok(());
    };

    for (offset, instruction) in instructions.iter().enumerate() {
        let selected_instruction_index = function
            .instructions
            .start()
            .arena_index()
            .checked_add(u32::try_from(offset).expect("instruction offset overflow"))
            .expect("instruction index overflow");

        let selected_text_offset = selected_instruction_text_offset(
            input,
            function_handle,
            function,
            selected_instruction_index,
        )?;

        collect_instruction_relocations(
            input,
            function,
            selected_instruction_index,
            selected_text_offset,
            instruction,
            relocation_plan,
        );
    }

    Ok(())
}
