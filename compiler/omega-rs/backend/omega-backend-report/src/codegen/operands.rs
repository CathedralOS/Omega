use crate::BackendReportInput;
use omega_object_file::storage_region_symbol_name;
use omega_target_operations::{InstructionOperand, InstructionOperandKind};

pub(super) fn selected_instruction_operands_name(
    backend_plan: &BackendReportInput<'_>,
    operands: omega_core::arena::HandleSpan<InstructionOperand>,
) -> String {
    let Some(operands) = backend_plan.target_operations.code.operands.span(operands) else {
        return "invalid operands".to_owned();
    };

    operands
        .iter()
        .map(|operand| match &operand.kind {
            InstructionOperandKind::DataAddress { data } => {
                let symbol = backend_plan.data.objects.get(*data).symbol.as_ref();
                format!("addr {symbol}")
            }
            InstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!("string ptr {symbol}@{byte_offset}")
            }
            InstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!("string len {symbol}@{byte_offset}")
            }
            InstructionOperandKind::RuntimePointeeStringPointer {
                region,
                byte_offset,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!("pointee string ptr *{symbol}@{byte_offset}")
            }
            InstructionOperandKind::RuntimePointeeStringLength {
                region,
                byte_offset,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!("pointee string len *{symbol}@{byte_offset}")
            }
            InstructionOperandKind::RuntimeScalarInteger {
                region,
                byte_offset,
                byte_count,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!("scalar i{} {symbol}@{byte_offset}", byte_count * 8)
            }
            InstructionOperandKind::ImmediateInteger(value) => value.to_string(),
            InstructionOperandKind::ByteLength(value) => format!("len {value}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
