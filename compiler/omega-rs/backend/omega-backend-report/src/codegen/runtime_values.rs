use crate::BackendReportInput;
use omega_assigned_target_operations::{RuntimeTextReadSource, RuntimeValueOperand};
use omega_object_file::storage_region_symbol_name;
use omega_target_operations::RuntimeValueOperandHandle;

pub(super) fn runtime_text_read_source_name(
    backend_plan: &BackendReportInput<'_>,
    source: &RuntimeTextReadSource,
) -> String {
    match source {
        RuntimeTextReadSource::HostOperation { operation_key } => {
            match backend_plan
                .assigned_target_operations
                .host_binding(*operation_key)
                .map(|binding| &binding.mechanism)
            {
                Some(omega_calling_conventions::HostBindingMechanism::Import {
                    symbol, ..
                }) => {
                    format!("import {symbol}")
                }
                Some(omega_calling_conventions::HostBindingMechanism::Syscall {
                    number,
                    number_register,
                    supervisor_call,
                    ..
                }) => {
                    format!("syscall {number} via x{number_register}/svc #{supervisor_call}")
                }
                None => {
                    format!(
                        "unresolved host operation {}.{}",
                        operation_key.capability_name(),
                        operation_key.operation_name()
                    )
                }
            }
        }
    }
}

pub(super) fn runtime_value_operand_name(
    backend_plan: &BackendReportInput<'_>,
    operand: RuntimeValueOperandHandle,
) -> String {
    match &backend_plan
        .assigned_target_operations
        .runtime_value_operand(operand)
        .expect("assigned runtime value operand should exist while reporting backend state")
        .kind
    {
        RuntimeValueOperand::Immediate(value) => value.to_string(),
        RuntimeValueOperand::Storage {
            region,
            byte_offset,
            byte_size,
        } => {
            let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
            format!("{symbol}@{byte_offset}/{}", byte_size)
        }
        RuntimeValueOperand::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => format!(
            "*frame@{pointer_byte_offset}+{field_byte_offset}/{}",
            byte_size
        ),
        RuntimeValueOperand::FrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => format!(
            "frame_indexed(descriptor@{descriptor_offset}, index@{index_offset}, elem {element_byte_size}, field +{field_byte_offset}, bytes {byte_size})"
        ),
        RuntimeValueOperand::FrameBaseIndexed {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => format!(
            "frame_base_indexed(base@{base_byte_offset}, index@{index_offset}, elem {element_byte_size}, field +{field_byte_offset}, bytes {byte_size})"
        ),
        RuntimeValueOperand::FrameFixedIndexed {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => format!(
            "frame_fixed_indexed(descriptor@{descriptor_offset}, index {element_index}, elem {element_byte_size}, field +{field_byte_offset}, bytes {byte_size})"
        ),
        RuntimeValueOperand::Binary {
            left,
            operator,
            right,
            is_float,
            byte_width,
        } => format!(
            "({} {operator:?}{}/{byte_width} {})",
            runtime_value_operand_name(backend_plan, *left),
            if *is_float { " f" } else { "" },
            runtime_value_operand_name(backend_plan, *right),
        ),
        RuntimeValueOperand::TextEquals {
            left_region,
            left_offset,
            right_region,
            right_offset,
        } => {
            let left_symbol =
                storage_region_symbol_name(*left_region, backend_plan.entry_machine_name());
            let right_symbol =
                storage_region_symbol_name(*right_region, backend_plan.entry_machine_name());
            format!("text_equals({left_symbol}@{left_offset}, {right_symbol}@{right_offset})")
        }
        RuntimeValueOperand::TextEqualsLiteral {
            place,
            literal,
            place_is_bounded_buffer,
        } => format!(
            "text_equals_literal({}, {literal:?}{})",
            runtime_value_operand_name(backend_plan, *place),
            if *place_is_bounded_buffer {
                ", carrier"
            } else {
                ""
            },
        ),
        RuntimeValueOperand::Convert {
            source,
            source_byte_size,
            target_byte_size,
            source_is_float,
            target_is_float,
            ..
        } => format!(
            "({} as {}{source_byte_size}->{}{target_byte_size})",
            runtime_value_operand_name(backend_plan, *source),
            if *source_is_float { "f" } else { "i" },
            if *target_is_float { "f" } else { "i" },
        ),
    }
}
