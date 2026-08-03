use crate::BackendReportInput;
use omega_object_file::storage_region_symbol_name;
use omega_target_operations::{InstructionOperand, InstructionOperandKind};

pub(super) fn selected_instruction_operands_name(
    backend_plan: &BackendReportInput<'_>,
    operands: psi_arena::HandleSpan<InstructionOperand>,
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
                is_bounded_buffer,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                let carrier = if *is_bounded_buffer { " carrier" } else { "" };
                format!("string ptr {symbol}@{byte_offset}{carrier}")
            }
            InstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
                is_bounded_buffer,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                let carrier = if *is_bounded_buffer { " carrier" } else { "" };
                format!("string len {symbol}@{byte_offset}{carrier}")
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
            InstructionOperandKind::RuntimeScalarFloat {
                region,
                byte_offset,
                byte_count,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!("scalar f{} {symbol}@{byte_offset}", byte_count * 8)
            }
            InstructionOperandKind::RuntimeHomogeneousFloatAggregate {
                region,
                byte_offset,
                member_byte_count,
                members,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!(
                    "hfa{}x{} {symbol}@{byte_offset}",
                    members,
                    member_byte_count * 8
                )
            }
            InstructionOperandKind::RuntimeSystemVAggregate {
                region,
                byte_offset,
                byte_count,
                alignment,
                sse_eightbytes,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!(
                    "sysv aggregate {byte_count}/{alignment} sse={sse_eightbytes:#04b} {symbol}@{byte_offset}"
                )
            }
            InstructionOperandKind::RuntimeSmallAggregate {
                region,
                byte_offset,
                byte_count,
                alignment,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!("aggregate {byte_count}/{alignment} {symbol}@{byte_offset}")
            }
            InstructionOperandKind::RuntimeLargeAggregate {
                region,
                byte_offset,
                byte_count,
                alignment,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!("indirect aggregate {byte_count}/{alignment} {symbol}@{byte_offset}")
            }
            InstructionOperandKind::RuntimeStorageAddress {
                region,
                byte_offset,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!("address &{symbol}@{byte_offset}")
            }
            InstructionOperandKind::ImmediateInteger(value) => value.to_string(),
            InstructionOperandKind::ByteLength(value) => format!("len {value}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
