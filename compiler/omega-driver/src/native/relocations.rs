use crate::diagnostics::Diagnostic;
use crate::native::abi::{HostBinding, HostBindingMechanism};
use crate::native::architecture;
use crate::native::instructions::{
    FunctionInstructionPlan, InstructionOperand, InstructionOperandKind, SelectedInstructionKind,
};
use crate::native::object::machine_storage_symbol_name;
use crate::native::plan::NativePlan;
use crate::native::state_guards::{StateGuardLowering, StateGuardOperator};
use crate::native::target::{Architecture, NativeTarget};
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationPlan {
    pub target: NativeTarget,
    pub records: Arena<RelocationRecord>,
}

impl Default for RelocationPlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            records: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationRecord {
    pub function_symbol: String,
    pub selected_instruction_index: u32,
    pub text_offset: usize,
    pub byte_width: usize,
    pub symbol: String,
    pub kind: RelocationKind,
}

impl Default for RelocationRecord {
    fn default() -> Self {
        Self {
            function_symbol: String::new(),
            selected_instruction_index: 0,
            text_offset: 0,
            byte_width: 0,
            symbol: String::new(),
            kind: RelocationKind::Aarch64Branch26,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationKind {
    Aarch64Page21,
    Aarch64PageOffset12,
    Aarch64Branch26,
    X86_64Absolute64,
    X86_64Relative32,
}

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
            _ => {}
        }
    }

    Ok(())
}

fn collect_data_address_relocations(
    native_plan: &NativePlan,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
    operands: omega_core::arena::HandleSpan<crate::native::instructions::InstructionOperand>,
    selected_text_offset: usize,
    relocation_plan: &mut RelocationPlan,
) {
    let Some(operands) = native_plan.instructions.operands.span(operands) else {
        return;
    };

    let mut operand_text_offset = selected_text_offset;

    for operand in operands {
        let InstructionOperandKind::DataAddress { symbol } = &operand.kind else {
            operand_text_offset +=
                architecture::operand_width(native_plan.target.architecture, operand);
            continue;
        };

        insert_data_address_relocations(
            native_plan.target.architecture,
            relocation_plan,
            function,
            selected_instruction_index,
            operand_text_offset,
            symbol,
        );

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
        .map(|operand| crate::native::architecture::operand_width(architecture, operand))
        .sum::<usize>();

    selected_text_offset
        + operand_bytes
        + match architecture {
            Architecture::Aarch64 => 0,
            Architecture::X86_64 => 1,
        }
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
