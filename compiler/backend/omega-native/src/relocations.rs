use crate::abi::{HostBinding, HostBindingMechanism};
use crate::architecture;
use crate::instructions::{
    FunctionInstructionPlan, InstructionOperand, InstructionOperandKind, SelectedInstructionKind,
};
use crate::object::machine_storage_symbol_name;
use crate::plan::NativePlan;
use crate::state_guards::{StateGuardLowering, StateGuardOperator};
use crate::target::Architecture;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
pub use omega_object::{RelocationKind, RelocationPlan, RelocationRecord};

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

fn collect_data_address_relocations(
    native_plan: &NativePlan,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
    operands: omega_core::arena::HandleSpan<crate::instructions::InstructionOperand>,
    selected_text_offset: usize,
    relocation_plan: &mut RelocationPlan,
) {
    let Some(operands) = native_plan.instructions.operands.span(operands) else {
        return;
    };

    let mut operand_text_offset = selected_text_offset;

    for operand in operands {
        match &operand.kind {
            InstructionOperandKind::DataAddress { symbol } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    operand_text_offset,
                    symbol,
                );
            }
            InstructionOperandKind::RuntimeMachineStringPointer { .. }
            | InstructionOperandKind::RuntimeMachineStringLength { .. } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    operand_text_offset,
                    &machine_storage_symbol_name(&native_plan.entry_machine),
                );
            }
            InstructionOperandKind::ImmediateInteger(_) | InstructionOperandKind::ByteLength(_) => {
            }
        }

        operand_text_offset +=
            architecture::operand_width(native_plan.target.architecture, operand);
    }
}

fn insert_data_address_relocations(
    architecture: Architecture,
    relocation_plan: &mut RelocationPlan,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
    operand_text_offset: usize,
    symbol: &str,
) {
    match architecture {
        Architecture::Aarch64 => {
            relocation_plan.records.insert(RelocationRecord {
                function_symbol: function.symbol.clone(),
                selected_instruction_index,
                text_offset: operand_text_offset,
                byte_width: 4,
                symbol: symbol.to_owned(),
                kind: RelocationKind::Aarch64Page21,
            });
            relocation_plan.records.insert(RelocationRecord {
                function_symbol: function.symbol.clone(),
                selected_instruction_index,
                text_offset: operand_text_offset + 4,
                byte_width: 4,
                symbol: symbol.to_owned(),
                kind: RelocationKind::Aarch64PageOffset12,
            });
        }
        Architecture::X86_64 => {
            relocation_plan.records.insert(RelocationRecord {
                function_symbol: function.symbol.clone(),
                selected_instruction_index,
                text_offset: operand_text_offset,
                byte_width: 8,
                symbol: symbol.to_owned(),
                kind: RelocationKind::X86_64Absolute64,
            });
        }
    }
}

fn selected_instruction_text_offset(
    native_plan: &NativePlan,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
) -> Result<usize, Diagnostic> {
    let Some(machine_function) = native_plan
        .machine_code
        .functions
        .iter()
        .find(|(_, machine_function)| machine_function.symbol == function.symbol)
        .map(|(_, machine_function)| machine_function)
    else {
        return Err(Diagnostic::error(format!(
            "cannot plan relocations for `{}`: missing machine-code function",
            function.symbol
        )));
    };

    let Some(machine_instructions) = native_plan
        .machine_code
        .instructions
        .span(machine_function.instructions)
    else {
        return Err(Diagnostic::error(format!(
            "cannot plan relocations for `{}`: invalid machine instruction span",
            function.symbol
        )));
    };

    machine_instructions
        .iter()
        .find(|instruction| instruction.selected_instruction_index == selected_instruction_index)
        .map(|instruction| instruction.offset)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "cannot plan relocation for `{}` selected instruction #{}: missing machine-code instruction",
                function.symbol, selected_instruction_index
            ))
        })
}

fn external_call_relocation_offset(
    architecture: Architecture,
    selected_text_offset: usize,
    operands: &[InstructionOperand],
) -> usize {
    let operand_bytes = operands
        .iter()
        .map(|operand| crate::architecture::operand_width(architecture, operand))
        .sum::<usize>();

    selected_text_offset
        + operand_bytes
        + match architecture {
            Architecture::Aarch64 => 0,
            Architecture::X86_64 => 1,
        }
}

fn string_descriptor_machine_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

fn runtime_storage_copy_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

fn runtime_storage_compare_right_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

fn runtime_text_stored_suffix_source_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

fn runtime_text_stored_suffix_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 52,
        Architecture::X86_64 => 16,
    }
}

fn runtime_text_stored_place_source_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 28,
        Architecture::X86_64 => 8,
    }
}

fn runtime_text_stored_place_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

fn runtime_text_literal_append_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

fn runtime_text_buffer_materialize_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

fn runtime_text_line_read_target_address_offset(
    architecture: Architecture,
    syscall_number: u32,
) -> usize {
    crate::architecture::runtime_text_line_read_target_address_offset(architecture, syscall_number)
}

fn external_call_relocation_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 4,
        Architecture::X86_64 => 4,
    }
}

fn external_call_relocation_kind(architecture: Architecture) -> RelocationKind {
    match architecture {
        Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
        Architecture::X86_64 => RelocationKind::X86_64Relative32,
    }
}

fn find_host_binding<'plan>(
    native_plan: &'plan NativePlan,
    capability: &str,
    operation: &str,
) -> Option<&'plan HostBinding> {
    native_plan
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.capability == capability && binding.operation == operation)
        .map(|(_, binding)| binding)
}
