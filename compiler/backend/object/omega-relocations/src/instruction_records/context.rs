use super::super::data_addresses::insert_data_address_relocations;
use crate::RelocationPlanningInput;
use omega_object::RelocationPlan;
use omega_target_operations::FunctionInstructionPlan;

pub(super) struct InstructionRelocationContext<'plan, 'relocations> {
    pub input: RelocationPlanningInput<'plan>,
    pub function: &'plan FunctionInstructionPlan,
    pub selected_instruction_index: u32,
    pub selected_text_offset: usize,
    pub relocation_plan: &'relocations mut RelocationPlan,
}

impl InstructionRelocationContext<'_, '_> {
    pub(super) fn insert_data_address(&mut self, byte_offset: usize, symbol: &str) {
        insert_data_address_relocations(
            self.input,
            self.relocation_plan,
            self.function,
            self.selected_instruction_index,
            byte_offset,
            symbol,
        );
    }

    pub(super) fn insert_data_address_at_instruction_start(&mut self, symbol: &str) {
        self.insert_data_address(self.selected_text_offset, symbol);
    }

    pub(super) fn insert_data_address_at_relative_offset(
        &mut self,
        relative_offset: usize,
        symbol: &str,
    ) {
        self.insert_data_address(self.selected_text_offset + relative_offset, symbol);
    }
}
