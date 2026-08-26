use super::super::data_address_records::insert_data_address_relocations;
use crate::RelocationPlanningInput;
use omega_object_file::{
    ObjectSymbolHandle, RelocationPlan, machine_storage_symbol_name, object_symbol_handle_by_name,
    storage_region_symbol_name,
};
use omega_target::Architecture;
use omega_target_operations::{RuntimeStorageRegion, TargetDataObjectHandle};

pub(super) struct InstructionRelocationContext<'plan, 'relocations> {
    pub input: RelocationPlanningInput<'plan>,
    pub function_symbol_handle: ObjectSymbolHandle,
    pub selected_instruction_index: u32,
    pub selected_text_offset: usize,
    pub selected_text_width: usize,
    pub relocation_plan: &'relocations mut RelocationPlan,
}

impl InstructionRelocationContext<'_, '_> {
    pub(super) fn object_symbol_handle(&self, symbol_name: &str) -> ObjectSymbolHandle {
        object_symbol_handle_by_name(&self.input.object, symbol_name)
    }

    pub(super) fn data_object_symbol_handle(
        &self,
        data: TargetDataObjectHandle,
    ) -> ObjectSymbolHandle {
        self.object_symbol_handle(self.input.data.objects.get(data).symbol.as_ref())
    }

    pub(super) fn data_object_byte_count(&self, data: TargetDataObjectHandle) -> usize {
        self.input.data.objects.get(data).bytes.len()
    }

    pub(super) fn storage_region_symbol_handle(
        &self,
        region: RuntimeStorageRegion,
    ) -> ObjectSymbolHandle {
        let symbol_name = storage_region_symbol_name(region, self.input.entry_machine_name);
        self.object_symbol_handle(&symbol_name)
    }

    pub(super) fn machine_storage_symbol_handle(&self) -> ObjectSymbolHandle {
        let symbol_name = machine_storage_symbol_name(self.input.entry_machine_name);
        self.object_symbol_handle(&symbol_name)
    }

    pub(super) fn runtime_frame_symbol_handle(&self) -> ObjectSymbolHandle {
        self.storage_region_symbol_handle(RuntimeStorageRegion::RuntimeFrame)
    }

    pub(super) fn insert_data_address(&mut self, byte_offset: usize, symbol: ObjectSymbolHandle) {
        if self.selected_text_width == 0 {
            return;
        }

        let byte_offset = match self.input.target.architecture {
            Architecture::Aarch64 => byte_offset,
            Architecture::X86_64 => byte_offset + 2,
        };
        insert_data_address_relocations(
            self.input,
            self.relocation_plan,
            self.function_symbol_handle,
            self.selected_instruction_index,
            byte_offset,
            symbol,
        );
    }

    pub(super) fn insert_data_address_at_instruction_start(&mut self, symbol: ObjectSymbolHandle) {
        self.insert_data_address(self.selected_text_offset, symbol);
    }

    pub(super) fn insert_data_address_at_relative_offset(
        &mut self,
        relative_offset: usize,
        symbol: ObjectSymbolHandle,
    ) {
        self.insert_data_address(self.selected_text_offset + relative_offset, symbol);
    }
}
