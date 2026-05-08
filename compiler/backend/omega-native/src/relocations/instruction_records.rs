use super::data_addresses::{collect_data_address_relocations, insert_data_address_relocations};
use super::lookups::find_host_binding;
use super::offsets::{
    external_call_relocation_kind, external_call_relocation_offset, external_call_relocation_width,
    runtime_storage_compare_right_address_offset, runtime_storage_copy_target_address_offset,
    runtime_text_buffer_materialize_target_address_offset,
    runtime_text_line_read_target_address_offset,
    runtime_text_literal_append_target_address_offset,
    runtime_text_stored_place_source_address_offset,
    runtime_text_stored_place_target_address_offset,
    runtime_text_stored_suffix_source_address_offset,
    runtime_text_stored_suffix_target_address_offset, string_descriptor_machine_address_offset,
};
use crate::abi::HostBindingMechanism;
use crate::instructions::{FunctionInstructionPlan, SelectedInstruction, SelectedInstructionKind};
use crate::object::machine_storage_symbol_name;
use crate::plan::NativePlan;
use crate::state_guards::{StateGuardLowering, StateGuardOperator};
use omega_object::{RelocationPlan, RelocationRecord};

struct InstructionRelocationContext<'plan, 'relocations> {
    native_plan: &'plan NativePlan,
    function: &'plan FunctionInstructionPlan,
    selected_instruction_index: u32,
    selected_text_offset: usize,
    relocation_plan: &'relocations mut RelocationPlan,
}

impl InstructionRelocationContext<'_, '_> {
    fn insert_data_address(&mut self, byte_offset: usize, symbol: &str) {
        insert_data_address_relocations(
            self.native_plan.target.architecture,
            self.relocation_plan,
            self.function,
            self.selected_instruction_index,
            byte_offset,
            symbol,
        );
    }

    fn insert_data_address_at_instruction_start(&mut self, symbol: &str) {
        self.insert_data_address(self.selected_text_offset, symbol);
    }

    fn insert_data_address_at_relative_offset(&mut self, relative_offset: usize, symbol: &str) {
        self.insert_data_address(self.selected_text_offset + relative_offset, symbol);
    }
}

pub(super) fn collect_instruction_relocations(
    native_plan: &NativePlan,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
    selected_text_offset: usize,
    instruction: &SelectedInstruction,
    relocation_plan: &mut RelocationPlan,
) {
    let mut context = InstructionRelocationContext {
        native_plan,
        function,
        selected_instruction_index,
        selected_text_offset,
        relocation_plan,
    };

    match &instruction.kind {
        SelectedInstructionKind::HostOperation {
            capability,
            operation,
            operands,
        } => {
            collect_data_address_relocations(
                native_plan,
                function,
                selected_instruction_index,
                *operands,
                selected_text_offset,
                context.relocation_plan,
            );

            let Some(binding) = find_host_binding(native_plan, capability, operation) else {
                return;
            };

            let HostBindingMechanism::Import { symbol, .. } = &binding.mechanism else {
                return;
            };

            context.relocation_plan.records.insert(RelocationRecord {
                function_symbol: function.symbol.clone(),
                selected_instruction_index,
                text_offset: external_call_relocation_offset(
                    native_plan.target.architecture,
                    selected_text_offset,
                    native_plan
                        .instructions
                        .operands
                        .span(*operands)
                        .unwrap_or(&[]),
                ),
                byte_width: external_call_relocation_width(native_plan.target.architecture),
                symbol: symbol.clone(),
                kind: external_call_relocation_kind(native_plan.target.architecture),
            });
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator: StateGuardOperator::Equal | StateGuardOperator::NotEqual,
            has_storage: true,
            ..
        } => {
            context.insert_data_address_at_instruction_start(&machine_storage_symbol_name(
                &native_plan.entry_machine,
            ));
        }
        SelectedInstructionKind::CompareRuntimeTextLiteral { buffer_symbol, .. } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer_symbol,
            source_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(8, source_symbol);
        }
        SelectedInstructionKind::CompareRuntimeStorage {
            left_symbol,
            right_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(left_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_compare_right_address_offset(native_plan.target.architecture),
                right_symbol,
            );
        }
        SelectedInstructionKind::CompareRuntimeStorageValue { symbol, .. } => {
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer_symbol, .. } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment { buffer_symbol, .. } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_symbol,
            source_symbol,
            target_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_source_address_offset(native_plan.target.architecture),
                source_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_target_address_offset(native_plan.target.architecture),
                target_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            buffer_symbol,
            source_symbol,
            target_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_target_address_offset(native_plan.target.architecture),
                target_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_source_address_offset(native_plan.target.architecture),
                source_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            buffer_symbol,
            target_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_literal_append_target_address_offset(native_plan.target.architecture),
                target_symbol,
            );
        }
        SelectedInstructionKind::MaterializeRuntimeTextBuffer {
            buffer_symbol,
            target_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_buffer_materialize_target_address_offset(
                    native_plan.target.architecture,
                ),
                target_symbol,
            );
        }
        SelectedInstructionKind::WriteRuntimeMachineInteger { .. } => {
            context.insert_data_address_at_instruction_start(&machine_storage_symbol_name(
                &native_plan.entry_machine,
            ));
        }
        SelectedInstructionKind::WriteRuntimeMachineString { data_symbol, .. } => {
            context.insert_data_address_at_instruction_start(data_symbol);
            context.insert_data_address_at_relative_offset(
                string_descriptor_machine_address_offset(native_plan.target.architecture),
                &machine_storage_symbol_name(&native_plan.entry_machine),
            );
        }
        SelectedInstructionKind::ReadRuntimeTextLine {
            buffer_symbol,
            target_symbol,
            syscall_number,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_line_read_target_address_offset(
                    native_plan.target.architecture,
                    *syscall_number,
                ),
                target_symbol,
            );
        }
        SelectedInstructionKind::CopyRuntimeStorage {
            source_symbol,
            target_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(source_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_target_address_offset(native_plan.target.architecture),
                target_symbol,
            );
        }
        _ => {}
    }
}
