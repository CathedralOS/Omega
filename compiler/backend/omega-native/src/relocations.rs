use crate::abi::HostBindingMechanism;
use crate::instructions::{FunctionInstructionPlan, SelectedInstructionKind};
use crate::object::machine_storage_symbol_name;
use crate::plan::NativePlan;
use crate::state_guards::{StateGuardLowering, StateGuardOperator};
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
pub use omega_object::{RelocationKind, RelocationPlan, RelocationRecord};

mod data_addresses;
mod lookups;
mod offsets;

use data_addresses::{collect_data_address_relocations, insert_data_address_relocations};
use lookups::{find_host_binding, selected_instruction_text_offset};
use offsets::{
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

pub fn build_relocation_plan(native_plan: &NativePlan) -> Result<RelocationPlan, Diagnostic> {
    let mut relocation_plan = RelocationPlan {
        target: native_plan.target,
        records: Arena::new(),
    };

    for (_, function) in native_plan.instructions.functions.iter() {
        collect_function_relocations(native_plan, function, &mut relocation_plan)?;
    }

    Ok(relocation_plan)
}

fn collect_function_relocations(
    native_plan: &NativePlan,
    function: &FunctionInstructionPlan,
    relocation_plan: &mut RelocationPlan,
) -> Result<(), Diagnostic> {
    let Some(instructions) = native_plan
        .instructions
        .instructions
        .span(function.instructions)
    else {
        return Ok(());
    };

    for (offset, instruction) in instructions.iter().enumerate() {
        let selected_instruction_index = function
            .instructions
            .start()
            .arena_index()
            .checked_add(u32::try_from(offset).expect("instruction offset overflow"))
            .expect("instruction index overflow");

        let selected_text_offset =
            selected_instruction_text_offset(native_plan, function, selected_instruction_index)?;

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
                    relocation_plan,
                );

                let Some(binding) = find_host_binding(native_plan, capability, operation) else {
                    continue;
                };

                let HostBindingMechanism::Import { symbol, .. } = &binding.mechanism else {
                    continue;
                };

                relocation_plan.records.insert(RelocationRecord {
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
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    &machine_storage_symbol_name(&native_plan.entry_machine),
                );
            }
            SelectedInstructionKind::CompareRuntimeTextLiteral { buffer_symbol, .. } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    buffer_symbol,
                );
            }
            SelectedInstructionKind::CompareRuntimeTextStorage {
                buffer_symbol,
                source_symbol,
                ..
            } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    buffer_symbol,
                );
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset + 8,
                    source_symbol,
                );
            }
            SelectedInstructionKind::CompareRuntimeStorage {
                left_symbol,
                right_symbol,
                ..
            } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    left_symbol,
                );
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset
                        + runtime_storage_compare_right_address_offset(
                            native_plan.target.architecture,
                        ),
                    right_symbol,
                );
            }
            SelectedInstructionKind::CompareRuntimeStorageValue { symbol, .. } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    symbol,
                );
            }
            SelectedInstructionKind::WriteRuntimeTextLiteral { buffer_symbol, .. } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    buffer_symbol,
                );
            }
            SelectedInstructionKind::WriteRuntimeTextLiteralSegment { buffer_symbol, .. } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    buffer_symbol,
                );
            }
            SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
                buffer_symbol,
                source_symbol,
                target_symbol,
                ..
            } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    buffer_symbol,
                );
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset
                        + runtime_text_stored_suffix_source_address_offset(
                            native_plan.target.architecture,
                        ),
                    source_symbol,
                );
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset
                        + runtime_text_stored_suffix_target_address_offset(
                            native_plan.target.architecture,
                        ),
                    target_symbol,
                );
            }
            SelectedInstructionKind::AppendRuntimeTextStoredPlace {
                buffer_symbol,
                source_symbol,
                target_symbol,
                ..
            } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    buffer_symbol,
                );
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset
                        + runtime_text_stored_place_target_address_offset(
                            native_plan.target.architecture,
                        ),
                    target_symbol,
                );
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset
                        + runtime_text_stored_place_source_address_offset(
                            native_plan.target.architecture,
                        ),
                    source_symbol,
                );
            }
            SelectedInstructionKind::AppendRuntimeTextLiteral {
                buffer_symbol,
                target_symbol,
                ..
            } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    buffer_symbol,
                );
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset
                        + runtime_text_literal_append_target_address_offset(
                            native_plan.target.architecture,
                        ),
                    target_symbol,
                );
            }
            SelectedInstructionKind::MaterializeRuntimeTextBuffer {
                buffer_symbol,
                target_symbol,
                ..
            } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    buffer_symbol,
                );
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset
                        + runtime_text_buffer_materialize_target_address_offset(
                            native_plan.target.architecture,
                        ),
                    target_symbol,
                );
            }
            SelectedInstructionKind::WriteRuntimeMachineInteger { .. } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    &machine_storage_symbol_name(&native_plan.entry_machine),
                );
            }
            SelectedInstructionKind::WriteRuntimeMachineString { data_symbol, .. } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    data_symbol,
                );
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset
                        + string_descriptor_machine_address_offset(native_plan.target.architecture),
                    &machine_storage_symbol_name(&native_plan.entry_machine),
                );
            }
            SelectedInstructionKind::ReadRuntimeTextLine {
                buffer_symbol,
                target_symbol,
                syscall_number,
                ..
            } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    buffer_symbol,
                );
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset
                        + runtime_text_line_read_target_address_offset(
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
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset,
                    source_symbol,
                );
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    selected_text_offset
                        + runtime_storage_copy_target_address_offset(
                            native_plan.target.architecture,
                        ),
                    target_symbol,
                );
            }
            _ => {}
        }
    }

    Ok(())
}
