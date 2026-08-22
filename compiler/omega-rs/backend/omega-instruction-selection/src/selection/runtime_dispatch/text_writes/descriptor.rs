use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use crate::selection::storage_places::{
    resolve_runtime_frame_base_double_indexed_source_in_table,
    resolve_runtime_frame_base_indexed_target_with_index_region,
    resolve_runtime_frame_indexed_target, resolve_runtime_machine_double_indexed_source,
    resolve_runtime_machine_indexed_target, resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset, resolve_runtime_storage_place,
    resolve_runtime_storage_place_is_bounded_byte_buffer,
};
use omega_abstract_operations::SelectedInstruction;
use omega_abstract_operations::TargetDataObjectHandle;
use omega_control_flow::StateKey;
use psi_checked_trees::expression::Expression;

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn resolve_bounded_buffer_target_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    expression: &Expression,
) -> Option<omega_abstract_operations::Place> {
    let direct_is_bounded = resolve_runtime_storage_place_is_bounded_byte_buffer(
        input,
        dispatch_index,
        source_key,
        expression,
    );
    if let Some(target) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, expression)
        && direct_is_bounded
    {
        return Some(crate::selection::runtime_dispatch::text_place_pointee(
            target.pointer_byte_offset,
            target.field_byte_offset,
        ));
    }
    if let Some(target) =
        resolve_runtime_frame_indexed_target(input, dispatch_index, source_key, expression)
        && target.is_bounded_byte_buffer
    {
        return Some(crate::selection::runtime_dispatch::indexed_place(
            target.descriptor_offset,
            target.index_region,
            target.index_offset,
            target.index_byte_size,
            target.element_byte_size,
            target.field_byte_offset,
        ));
    }
    if let Some(target) = resolve_runtime_frame_base_indexed_target_with_index_region(
        input,
        dispatch_index,
        source_key,
        expression,
    ) && target.is_bounded_byte_buffer
    {
        return Some(
            crate::selection::runtime_dispatch::frame_base_indexed_place_with_index_region(
                target.base_byte_offset,
                target.index_region,
                target.index_offset,
                target.index_byte_size,
                target.element_byte_size,
                target.field_byte_offset,
            ),
        );
    }
    let mut frame_double_expressions = psi_checked_trees::expression::ExpressionTable::default();
    let frame_double_handle = frame_double_expressions.insert_tree(expression);
    if let Some(target) = resolve_runtime_frame_base_double_indexed_source_in_table(
        input,
        dispatch_index,
        source_key,
        &frame_double_expressions,
        frame_double_handle,
    ) && target.is_bounded_byte_buffer
    {
        return Some(crate::selection::runtime_dispatch::double_indexed_place(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            target.base_byte_offset,
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            target.outer_index_offset,
            target.outer_index_byte_size,
            target.outer_stride,
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            target.inner_index_offset,
            target.inner_index_byte_size,
            target.inner_stride,
            target.field_byte_offset,
        ));
    }
    if let Some(target) =
        resolve_runtime_machine_double_indexed_source(input, dispatch_index, source_key, expression)
        && target.is_bounded_byte_buffer
    {
        return Some(crate::selection::runtime_dispatch::double_indexed_place(
            omega_abstract_operations::RuntimeStorageRegion::Machine,
            target.base_byte_offset,
            target.outer_index_region,
            target.outer_index_offset,
            target.outer_index_byte_size,
            target.outer_stride,
            target.inner_index_region,
            target.inner_index_offset,
            target.inner_index_byte_size,
            target.inner_stride,
            target.field_byte_offset,
        ));
    }
    if let Some(target) =
        resolve_runtime_machine_indexed_target(input, dispatch_index, source_key, expression)
        && target.is_bounded_byte_buffer
    {
        return Some(crate::selection::runtime_dispatch::machine_indexed_place(
            target.base_byte_offset,
            target.index_region,
            target.index_offset,
            target.index_byte_size,
            target.element_byte_size,
            target.field_byte_offset,
        ));
    }
    if !direct_is_bounded {
        return None;
    }
    let target = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        expression,
    )?;
    Some(omega_abstract_operations::Place::at(
        target.region,
        target.byte_offset,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_string_descriptor_write(
    input: &InstructionSelectionInput<'_>,
    literal_source_key: StateKey,
    target_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    dispatch_index: u32,
    statement_index: usize,
    resolved_target: &Expression,
    value: &[u8],
    selected_instructions: &mut SelectedInstructionSink,
) {
    if let Some(target) = resolve_runtime_frame_base_indexed_target_with_index_region(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) && target.is_bounded_byte_buffer
    {
        selected_instructions.push(SelectedInstruction {
            kind: omega_abstract_operations::SelectedInstructionKind::WritePlaceBoundedBuffer {
                target:
                    crate::selection::runtime_dispatch::frame_base_indexed_place_with_index_region(
                        target.base_byte_offset,
                        target.index_region,
                        target.index_offset,
                        target.index_byte_size,
                        target.element_byte_size,
                        target.field_byte_offset,
                    ),
                literal: std::sync::Arc::from(value),
            },
            source_key: literal_source_key,
            source_statement: statement_index,
        });
        return;
    }
    if let Some(target) = resolve_bounded_buffer_target_place(
        input,
        dispatch_index,
        target_source_key,
        source_machine,
        source_state,
        resolved_target,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: omega_abstract_operations::SelectedInstructionKind::WritePlaceBoundedBuffer {
                target,
                literal: std::sync::Arc::from(value),
            },
            source_key: literal_source_key,
            source_statement: statement_index,
        });
        return;
    }

    let data = string_literal_data_object(input, literal_source_key, statement_index, value);
    if !data.is_valid() {
        return;
    };

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
    {
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_string_frame_indexed(
                indexed_target.descriptor_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                data,
                value.len(),
            ),
            source_key: literal_source_key,
            source_statement: statement_index,
        });
        return;
    }

    let mut machine_indexed_expressions = psi_checked_trees::expression::ExpressionTable::default();
    let target_handle = machine_indexed_expressions.insert_tree(resolved_target);
    if let Some(indexed_target) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        &machine_indexed_expressions,
        target_handle,
    ) && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
    {
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_string_machine_indexed(
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                data,
                value.len(),
            ),
            source_key: literal_source_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(indexed_target) = resolve_runtime_machine_double_indexed_source(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
    {
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_string_machine_double_indexed(
                indexed_target.base_byte_offset,
                indexed_target.outer_index_region,
                indexed_target.outer_index_offset,
                indexed_target.outer_index_byte_size,
                indexed_target.outer_stride,
                indexed_target.inner_index_region,
                indexed_target.inner_index_offset,
                indexed_target.inner_index_byte_size,
                indexed_target.inner_stride,
                indexed_target.field_byte_offset,
                data,
                value.len(),
            ),
            source_key: literal_source_key,
            source_statement: statement_index,
        });
        return;
    }

    let Some(target_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        target_source_key,
        source_machine,
        source_state,
        resolved_target,
    ) else {
        return;
    };

    if target_place.byte_count != input.runtime_abi.string_descriptor_size() {
        return;
    }

    // Honor the resolved region: a frame-resident place (e.g. a `let` local's
    // String field) must be a frame write, not a machine-storage write at the
    // same numeric offset (which would alias the machine-storage region base).
    let kind = match target_place.region {
        omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame => {
            crate::selection::runtime_dispatch::write_place_string_direct(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                target_place.byte_offset,
                data,
                value.len(),
            )
        }
        omega_abstract_operations::RuntimeStorageRegion::Machine => {
            crate::selection::runtime_dispatch::write_place_string_direct(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                target_place.byte_offset,
                data,
                value.len(),
            )
        }
    };
    selected_instructions.push(SelectedInstruction {
        kind,
        source_key: literal_source_key,
        source_statement: statement_index,
    });
}

pub(in crate::selection) fn string_literal_data_handle(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    value: &[u8],
) -> TargetDataObjectHandle {
    string_literal_data_object(input, source_key, statement_index, value)
}

fn string_literal_data_object(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    value: &[u8],
) -> TargetDataObjectHandle {
    let exact = input.data.objects.iter().find(|(_, data_object)| {
        data_object.source_key == source_key
            && data_object.source_statement == statement_index
            && input
                .data
                .bytes
                .span(data_object.bytes)
                .is_some_and(|bytes| bytes == value || (value.is_empty() && bytes == [0]))
    });
    exact
        .or_else(|| {
            input.data.objects.iter().find(|(_, data_object)| {
                input
                    .data
                    .bytes
                    .span(data_object.bytes)
                    .is_some_and(|bytes| bytes == value || (value.is_empty() && bytes == [0]))
            })
        })
        .map(|(handle, _)| handle)
        .unwrap_or_else(TargetDataObjectHandle::invalid)
}
