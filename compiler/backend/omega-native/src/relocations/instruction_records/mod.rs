mod context;
mod host_operation;
mod runtime_text;

use super::data_addresses::collect_data_address_relocations;
use super::offsets::{
    runtime_storage_compare_right_address_offset, runtime_storage_copy_target_address_offset,
    string_descriptor_machine_address_offset,
};
use crate::instructions::{FunctionInstructionPlan, SelectedInstruction, SelectedInstructionKind};
use crate::object::machine_storage_symbol_name;
use crate::plan::NativePlan;
use crate::state_guards::{StateGuardLowering, StateGuardOperator};
use context::InstructionRelocationContext;
use omega_object::RelocationPlan;

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
        SelectedInstructionKind::HostOperation { operands, .. } => {
            collect_data_address_relocations(
                native_plan,
                function,
                selected_instruction_index,
                *operands,
                selected_text_offset,
                context.relocation_plan,
            );
            host_operation::collect_host_operation_relocation(&mut context, &instruction.kind);
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator: StateGuardOperator::Equal | StateGuardOperator::NotEqual,
            has_storage: true,
            ..
        } => {
            context.insert_data_address_at_instruction_start(&machine_storage_symbol_name(
                native_plan.entry_machine_name(),
            ));
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
        SelectedInstructionKind::WriteRuntimeMachineInteger { .. } => {
            context.insert_data_address_at_instruction_start(&machine_storage_symbol_name(
                native_plan.entry_machine_name(),
            ));
        }
        SelectedInstructionKind::WriteRuntimeMachineString { data_symbol, .. } => {
            context.insert_data_address_at_instruction_start(data_symbol);
            context.insert_data_address_at_relative_offset(
                string_descriptor_machine_address_offset(native_plan.target.architecture),
                &machine_storage_symbol_name(native_plan.entry_machine_name()),
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
        _ => runtime_text::collect_runtime_text_relocations(&mut context, &instruction.kind),
    }
}
