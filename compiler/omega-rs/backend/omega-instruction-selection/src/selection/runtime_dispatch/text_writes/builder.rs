use super::string_literal_data_handle;
use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, resolve_runtime_alias_binding_handle,
    strip_mutable_expression,
};
use crate::selection::host_operations::runtime_text_input_buffer_data_for_text_place;
use crate::selection::host_operations::runtime_text_input_buffer_data_for_text_place_in_table;
use crate::selection::storage_places::{
    RuntimeFrameBaseDoubleIndexedTarget, RuntimeFrameBaseIndexedTarget, RuntimeFrameIndexedTarget,
    RuntimeMachineDoubleIndexedTarget, RuntimeMachineIndexedTarget, RuntimePointeeTarget,
    RuntimeStoragePlace, resolve_runtime_frame_base_double_indexed_source,
    resolve_runtime_frame_base_double_indexed_source_in_table,
    resolve_runtime_frame_base_indexed_target, resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_indexed_target, resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_machine_double_indexed_source,
    resolve_runtime_machine_double_indexed_source_in_table,
    resolve_runtime_machine_indexed_target_in_table, resolve_runtime_pointee_slot_offset,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place,
    resolve_runtime_storage_place_in_table,
};
use omega_abstract_operations::{
    RuntimeStorageRegion, SelectedInstructionKind, TargetDataObjectHandle, TargetDataObjectKind,
};
use omega_control_flow::StateKey;
use omega_runtime_text::RuntimeTextBuilderSegmentKind;
use psi_checked_trees::expression::{Expression, ExpressionHandle, ExpressionTable};
use std::sync::Arc;

const UNSUPPORTED_RUNTIME_TEXT_SEGMENT: &str = "<expr>";

#[derive(Debug, Clone)]
struct RuntimeTextTarget {
    place: Option<RuntimeStoragePlace>,
    indexed: Option<RuntimeFrameIndexedTarget>,
    frame_base_indexed: Option<RuntimeFrameBaseIndexedTarget>,
    frame_base_double_indexed: Option<RuntimeFrameBaseDoubleIndexedTarget>,
    machine_indexed: Option<RuntimeMachineIndexedTarget>,
    machine_double_indexed: Option<RuntimeMachineDoubleIndexedTarget>,
    pointee: Option<RuntimePointeeTarget>,
}

impl RuntimeTextTarget {
    fn new(
        input: &InstructionSelectionInput<'_>,
        place: Option<RuntimeStoragePlace>,
        indexed: Option<RuntimeFrameIndexedTarget>,
        frame_base_indexed: Option<RuntimeFrameBaseIndexedTarget>,
        frame_base_double_indexed: Option<RuntimeFrameBaseDoubleIndexedTarget>,
        machine_indexed: Option<RuntimeMachineIndexedTarget>,
        machine_double_indexed: Option<RuntimeMachineDoubleIndexedTarget>,
        pointee: Option<RuntimePointeeTarget>,
    ) -> Option<Self> {
        if place.is_none()
            && pointee.is_none()
            && indexed.is_none()
            && frame_base_indexed.is_none()
            && frame_base_double_indexed.is_none()
            && machine_indexed.is_none()
            && machine_double_indexed.is_none()
        {
            return None;
        }
        if place
            .as_ref()
            .is_some_and(|place| place.byte_count != input.runtime_abi.string_descriptor_size())
        {
            return None;
        }
        if pointee.as_ref().is_some_and(|target| {
            target.pointee_byte_size != input.runtime_abi.string_descriptor_size()
        }) {
            return None;
        }
        if indexed
            .as_ref()
            .is_some_and(|target| target.byte_count != input.runtime_abi.string_descriptor_size())
        {
            return None;
        }
        if frame_base_indexed
            .as_ref()
            .is_some_and(|target| target.byte_count != input.runtime_abi.string_descriptor_size())
        {
            return None;
        }
        if frame_base_double_indexed
            .as_ref()
            .is_some_and(|target| target.byte_count != input.runtime_abi.string_descriptor_size())
        {
            return None;
        }
        if machine_indexed
            .as_ref()
            .is_some_and(|target| target.byte_count != input.runtime_abi.string_descriptor_size())
        {
            return None;
        }
        if machine_double_indexed
            .as_ref()
            .is_some_and(|target| target.byte_count != input.runtime_abi.string_descriptor_size())
        {
            return None;
        }

        Some(Self {
            place,
            indexed,
            frame_base_indexed,
            frame_base_double_indexed,
            machine_indexed,
            machine_double_indexed,
            pointee,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn runtime_text_builder_write_with_scratch_emit(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    resolved_segment_expressions: &mut ExpressionTable,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    if aliases.is_empty()
        && runtime_text_builder_write_without_aliases_emit(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            resolved_target,
            emit,
        )
    {
        return true;
    }

    resolved_segment_expressions.clear();
    let copied_aliases = RuntimeAliasBuffer::copy_from_bindings(
        alias_expressions,
        aliases,
        resolved_segment_expressions,
    );
    runtime_text_builder_write_with_handle_resolver_emit(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        resolved_segment_expressions,
        &|expressions, expression| {
            resolve_runtime_alias_binding_handle(
                expression,
                source_key,
                copied_aliases.bindings(),
                expressions,
            )
            .expression
        },
        emit,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn runtime_text_builder_write_without_aliases_emit(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    let Some(builder) = input
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| {
            builder.source_key == source_key && builder.statement_index == statement_index
        })
        .map(|(_, builder)| builder)
    else {
        return false;
    };
    let Some(segments) = input.runtime_text.builder_segments.span(builder.segments) else {
        return false;
    };
    let resolved_target = strip_mutable_expression(resolved_target.clone());
    let buffer =
        runtime_text_input_buffer_for_builder(input, builder.source_key, builder.statement_index)
            .unwrap_or_else(|| {
                runtime_text_input_buffer_data_for_text_place(input, &resolved_target)
            });
    if !buffer.is_valid() {
        return false;
    };
    let mut target_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &resolved_target,
    );
    let target_indexed =
        resolve_runtime_frame_indexed_target(input, dispatch_index, source_key, &resolved_target);
    let target_frame_base_indexed = resolve_runtime_frame_base_indexed_target(
        input,
        dispatch_index,
        source_key,
        &resolved_target,
    );
    let target_frame_base_double_indexed = resolve_runtime_frame_base_double_indexed_source(
        input,
        dispatch_index,
        source_key,
        &resolved_target,
    );
    let mut machine_indexed_expressions = ExpressionTable::default();
    let machine_indexed_target = machine_indexed_expressions.insert_tree(&resolved_target);
    let target_machine_indexed = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        &machine_indexed_expressions,
        machine_indexed_target,
    );
    let target_machine_double_indexed = resolve_runtime_machine_double_indexed_source(
        input,
        dispatch_index,
        source_key,
        &resolved_target,
    );
    let target_pointee =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, &resolved_target);
    // A `&mut String` alias parameter resolves both as a storage place (its
    // pointer slot -- pointer-sized, not string-sized) and as a string pointee
    // (the descriptor it points at). The text write targets the pointee, so drop
    // the pointer-sized place; otherwise the string-size guard below rejects the
    // whole write because the alias slot is not a string descriptor.
    let string_descriptor_size = input.runtime_abi.string_descriptor_size();
    if target_place
        .as_ref()
        .is_some_and(|place| place.byte_count != string_descriptor_size)
        && target_pointee
            .as_ref()
            .is_some_and(|pointee| pointee.pointee_byte_size == string_descriptor_size)
    {
        target_place = None;
    }
    if target_place.is_none()
        && target_pointee.is_none()
        && target_indexed.is_none()
        && target_frame_base_indexed.is_none()
        && target_frame_base_double_indexed.is_none()
        && target_machine_indexed.is_none()
        && target_machine_double_indexed.is_none()
    {
        return false;
    }
    if target_place
        .as_ref()
        .is_some_and(|place| place.byte_count != input.runtime_abi.string_descriptor_size())
    {
        return false;
    }
    if target_pointee.as_ref().is_some_and(|target| {
        target.pointee_byte_size != input.runtime_abi.string_descriptor_size()
    }) {
        return false;
    }
    if target_indexed
        .as_ref()
        .is_some_and(|target| target.byte_count != input.runtime_abi.string_descriptor_size())
    {
        return false;
    }
    if target_frame_base_indexed
        .as_ref()
        .is_some_and(|target| target.byte_count != input.runtime_abi.string_descriptor_size())
    {
        return false;
    }
    if target_frame_base_double_indexed
        .as_ref()
        .is_some_and(|target| target.byte_count != input.runtime_abi.string_descriptor_size())
    {
        return false;
    }
    if target_machine_indexed
        .as_ref()
        .is_some_and(|target| target.byte_count != input.runtime_abi.string_descriptor_size())
    {
        return false;
    }
    if target_machine_double_indexed
        .as_ref()
        .is_some_and(|target| target.byte_count != input.runtime_abi.string_descriptor_size())
    {
        return false;
    }

    if let [prefix, suffix] = segments
        && prefix.kind == RuntimeTextBuilderSegmentKind::StaticText
        && suffix.kind == RuntimeTextBuilderSegmentKind::StoredPlace
        && let Some(target_place) = target_place.as_ref()
    {
        return prefixed_stored_place_write_without_aliases(
            input,
            dispatch_index,
            source_key,
            buffer,
            target_place.region,
            target_place.byte_offset,
            prefix,
            suffix,
            emit,
        );
    }

    let Some(target) = RuntimeTextTarget::new(
        input,
        target_place.clone(),
        target_indexed.clone(),
        target_frame_base_indexed,
        target_frame_base_double_indexed,
        target_machine_indexed,
        target_machine_double_indexed,
        target_pointee,
    ) else {
        return false;
    };

    let initialized_first_segment = initialize_runtime_text_target_with_first_literal_segment(
        input,
        source_key,
        statement_index,
        buffer,
        &target,
        segments,
        emit,
    );

    let mut emitted = initialized_first_segment;
    for (segment_index, segment) in segments.iter().enumerate() {
        if initialized_first_segment && segment_index == 0 {
            continue;
        }
        match segment.kind {
            RuntimeTextBuilderSegmentKind::StoredPlace => {
                let Some(source_place) = resolve_runtime_storage_place_in_table(
                    input,
                    dispatch_index,
                    source_key,
                    &input.runtime_text.expressions,
                    segment.expression,
                ) else {
                    return false;
                };
                if source_place.byte_count != input.runtime_abi.string_descriptor_size() {
                    return false;
                }
                if let Some(target_place) = target.place.as_ref() {
                    if source_place.region == target_place.region
                        && source_place.byte_offset == target_place.byte_offset
                    {
                        emit(SelectedInstructionKind::MaterializeTextBufferToPlace {
                            buffer,
                            target: crate::selection::runtime_dispatch::text_place_direct(
                                target_place.region,
                                target_place.byte_offset,
                            ),
                        });
                        emitted = true;
                        continue;
                    }
                    emit(SelectedInstructionKind::AppendTextStoredToPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target: crate::selection::runtime_dispatch::text_place_direct(
                            target_place.region,
                            target_place.byte_offset,
                        ),
                    });
                } else if let Some(target) = target.pointee.as_ref() {
                    emit(SelectedInstructionKind::AppendTextStoredToPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target: crate::selection::runtime_dispatch::text_place_pointee(
                            target.pointer_byte_offset,
                            target.field_byte_offset,
                        ),
                    });
                } else if let Some(target) = target.indexed.as_ref() {
                    emit(SelectedInstructionKind::AppendTextStoredToPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target: crate::selection::runtime_dispatch::text_place_frame_indexed(
                            target.descriptor_offset,
                            target.index_region,
                            target.index_offset,
                            target.index_byte_size,
                            target.element_byte_size,
                            target.field_byte_offset,
                        ),
                    });
                } else if let Some(target) = target.frame_base_indexed.as_ref() {
                    emit(SelectedInstructionKind::AppendTextStoredToPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target: crate::selection::runtime_dispatch::frame_base_indexed_place(
                            target.base_byte_offset,
                            target.index_offset,
                            target.index_byte_size,
                            target.element_byte_size,
                            target.field_byte_offset,
                        ),
                    });
                } else if let Some(target) = target.frame_base_double_indexed.as_ref() {
                    emit(SelectedInstructionKind::AppendTextStoredToPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target: crate::selection::runtime_dispatch::double_indexed_place(
                            RuntimeStorageRegion::RuntimeFrame,
                            target.base_byte_offset,
                            RuntimeStorageRegion::RuntimeFrame,
                            target.outer_index_offset,
                            target.outer_index_byte_size,
                            target.outer_stride,
                            RuntimeStorageRegion::RuntimeFrame,
                            target.inner_index_offset,
                            target.inner_index_byte_size,
                            target.inner_stride,
                            target.field_byte_offset,
                        ),
                    });
                } else {
                    return false;
                }
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::StaticText => {
                let Some(literal) = input
                    .runtime_text
                    .expressions
                    .string_literal_value(segment.expression)
                else {
                    return false;
                };
                if !append_runtime_text_literal_to_target(
                    input,
                    source_key,
                    statement_index,
                    buffer,
                    target.place.as_ref(),
                    target.pointee.as_ref(),
                    target.indexed.as_ref(),
                    target.frame_base_indexed.as_ref(),
                    target.frame_base_double_indexed.as_ref(),
                    target.machine_indexed.as_ref(),
                    target.machine_double_indexed.as_ref(),
                    literal,
                    emit,
                ) {
                    return false;
                }
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::OtherExpression => {
                if !append_runtime_text_literal_to_target(
                    input,
                    source_key,
                    statement_index,
                    buffer,
                    target.place.as_ref(),
                    target.pointee.as_ref(),
                    target.indexed.as_ref(),
                    target.frame_base_indexed.as_ref(),
                    target.frame_base_double_indexed.as_ref(),
                    target.machine_indexed.as_ref(),
                    target.machine_double_indexed.as_ref(),
                    Arc::from(UNSUPPORTED_RUNTIME_TEXT_SEGMENT.as_bytes()),
                    emit,
                ) {
                    return false;
                }
                emitted = true;
            }
        }
    }

    emitted
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn runtime_text_builder_write_with_handle_resolver_emit(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolved_segment_expressions: &mut ExpressionTable,
    resolve_expression: &dyn Fn(&mut ExpressionTable, ExpressionHandle) -> ExpressionHandle,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    let Some(builder) = input
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| {
            builder.source_key == source_key && builder.statement_index == statement_index
        })
        .map(|(_, builder)| builder)
    else {
        return false;
    };
    let Some(segments) = input.runtime_text.builder_segments.span(builder.segments) else {
        return false;
    };
    let resolved_target = strip_mutable_expression(resolved_target.clone());
    let buffer =
        runtime_text_input_buffer_for_builder(input, builder.source_key, builder.statement_index)
            .unwrap_or_else(|| {
                runtime_text_input_buffer_data_for_text_place(input, &resolved_target)
            });
    if !buffer.is_valid() {
        return false;
    };
    let target_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &resolved_target,
    );
    let target_indexed =
        resolve_runtime_frame_indexed_target(input, dispatch_index, source_key, &resolved_target);
    let target_frame_base_indexed = resolve_runtime_frame_base_indexed_target(
        input,
        dispatch_index,
        source_key,
        &resolved_target,
    );
    let target_frame_base_double_indexed = resolve_runtime_frame_base_double_indexed_source(
        input,
        dispatch_index,
        source_key,
        &resolved_target,
    );
    let target_pointee =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, &resolved_target);
    let mut machine_indexed_target_expressions = ExpressionTable::default();
    let machine_indexed_target_expression =
        machine_indexed_target_expressions.insert_tree(&resolved_target);
    let target_machine_indexed = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        &machine_indexed_target_expressions,
        machine_indexed_target_expression,
    );
    let target_machine_double_indexed = resolve_runtime_machine_double_indexed_source(
        input,
        dispatch_index,
        source_key,
        &resolved_target,
    );
    let Some(target) = RuntimeTextTarget::new(
        input,
        target_place,
        target_indexed,
        target_frame_base_indexed,
        target_frame_base_double_indexed,
        target_machine_indexed,
        target_machine_double_indexed,
        target_pointee,
    ) else {
        return false;
    };
    emit_runtime_text_builder_segments_with_handle_resolver(
        input,
        dispatch_index,
        source_key,
        statement_index,
        buffer,
        &target,
        segments,
        resolved_segment_expressions,
        resolve_expression,
        emit,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn runtime_text_builder_write_in_table_emit(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    builder_source_key: StateKey,
    target_source_key: StateKey,
    statement_index: usize,
    target_expressions: &ExpressionTable,
    resolved_target: ExpressionHandle,
    resolved_segment_expressions: &mut ExpressionTable,
    resolve_expression: &dyn Fn(&mut ExpressionTable, ExpressionHandle) -> ExpressionHandle,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    let Some(builder) = input
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| {
            builder.source_key == builder_source_key && builder.statement_index == statement_index
        })
        .map(|(_, builder)| builder)
    else {
        return false;
    };
    let Some(segments) = input.runtime_text.builder_segments.span(builder.segments) else {
        return false;
    };
    let buffer =
        runtime_text_input_buffer_for_builder(input, builder.source_key, builder.statement_index)
            .unwrap_or_else(|| {
                runtime_text_input_buffer_data_for_text_place_in_table(
                    input,
                    target_expressions,
                    resolved_target,
                )
            });
    if !buffer.is_valid() {
        return false;
    };
    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        target_expressions,
        resolved_target,
    );
    let target_indexed = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        target_expressions,
        resolved_target,
    );
    let target_frame_base_indexed = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        target_expressions,
        resolved_target,
    );
    let target_frame_base_double_indexed =
        resolve_runtime_frame_base_double_indexed_source_in_table(
            input,
            dispatch_index,
            target_source_key,
            target_expressions,
            resolved_target,
        );
    let target_machine_indexed = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        target_expressions,
        resolved_target,
    );
    let target_machine_double_indexed = resolve_runtime_machine_double_indexed_source_in_table(
        input,
        dispatch_index,
        target_source_key,
        target_expressions,
        resolved_target,
    );
    let target_pointee = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        target_expressions,
        resolved_target,
    );
    let Some(target) = RuntimeTextTarget::new(
        input,
        target_place,
        target_indexed,
        target_frame_base_indexed,
        target_frame_base_double_indexed,
        target_machine_indexed,
        target_machine_double_indexed,
        target_pointee,
    ) else {
        return false;
    };
    emit_runtime_text_builder_segments_with_handle_resolver(
        input,
        dispatch_index,
        builder_source_key,
        statement_index,
        buffer,
        &target,
        segments,
        resolved_segment_expressions,
        resolve_expression,
        emit,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_runtime_text_builder_segments_with_handle_resolver(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    buffer: TargetDataObjectHandle,
    target: &RuntimeTextTarget,
    segments: &[omega_runtime_text::RuntimeTextBuilderSegment],
    resolved_segment_expressions: &mut ExpressionTable,
    resolve_expression: &dyn Fn(&mut ExpressionTable, ExpressionHandle) -> ExpressionHandle,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    if let [prefix, suffix] = segments
        && prefix.kind == RuntimeTextBuilderSegmentKind::StaticText
        && suffix.kind == RuntimeTextBuilderSegmentKind::StoredPlace
        && let Some(target_place) = target.place.as_ref()
    {
        return prefixed_stored_place_write_with_handle_resolver(
            input,
            dispatch_index,
            source_key,
            buffer,
            target_place.region,
            target_place.byte_offset,
            prefix,
            suffix,
            resolved_segment_expressions,
            resolve_expression,
            emit,
        );
    }

    let initialized_first_segment = initialize_runtime_text_target_with_first_literal_segment(
        input,
        source_key,
        statement_index,
        buffer,
        target,
        segments,
        emit,
    );

    let mut emitted = initialized_first_segment;
    for (segment_index, segment) in segments.iter().enumerate() {
        if initialized_first_segment && segment_index == 0 {
            continue;
        }
        match segment.kind {
            RuntimeTextBuilderSegmentKind::StoredPlace => {
                let segment_expression = resolved_segment_expressions
                    .copy_from(&input.runtime_text.expressions, segment.expression);
                let source = resolve_expression(resolved_segment_expressions, segment_expression);
                let Some(source_place) = resolve_runtime_storage_place_in_table(
                    input,
                    dispatch_index,
                    source_key,
                    resolved_segment_expressions,
                    source,
                ) else {
                    return false;
                };
                if source_place.byte_count != input.runtime_abi.string_descriptor_size() {
                    return false;
                }
                if let Some(target_place) = target.place.as_ref() {
                    if source_place.region == target_place.region
                        && source_place.byte_offset == target_place.byte_offset
                    {
                        emit(SelectedInstructionKind::MaterializeTextBufferToPlace {
                            buffer,
                            target: crate::selection::runtime_dispatch::text_place_direct(
                                target_place.region,
                                target_place.byte_offset,
                            ),
                        });
                        emitted = true;
                        continue;
                    }
                    emit(SelectedInstructionKind::AppendTextStoredToPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target: crate::selection::runtime_dispatch::text_place_direct(
                            target_place.region,
                            target_place.byte_offset,
                        ),
                    });
                } else if let Some(target) = target.pointee.as_ref() {
                    emit(SelectedInstructionKind::AppendTextStoredToPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target: crate::selection::runtime_dispatch::text_place_pointee(
                            target.pointer_byte_offset,
                            target.field_byte_offset,
                        ),
                    });
                } else if let Some(target) = target.indexed.as_ref() {
                    emit(SelectedInstructionKind::AppendTextStoredToPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target: crate::selection::runtime_dispatch::text_place_frame_indexed(
                            target.descriptor_offset,
                            target.index_region,
                            target.index_offset,
                            target.index_byte_size,
                            target.element_byte_size,
                            target.field_byte_offset,
                        ),
                    });
                } else if let Some(target) = target.frame_base_indexed.as_ref() {
                    emit(SelectedInstructionKind::AppendTextStoredToPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target: crate::selection::runtime_dispatch::frame_base_indexed_place(
                            target.base_byte_offset,
                            target.index_offset,
                            target.index_byte_size,
                            target.element_byte_size,
                            target.field_byte_offset,
                        ),
                    });
                } else if let Some(target) = target.frame_base_double_indexed.as_ref() {
                    emit(SelectedInstructionKind::AppendTextStoredToPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target: crate::selection::runtime_dispatch::double_indexed_place(
                            RuntimeStorageRegion::RuntimeFrame,
                            target.base_byte_offset,
                            RuntimeStorageRegion::RuntimeFrame,
                            target.outer_index_offset,
                            target.outer_index_byte_size,
                            target.outer_stride,
                            RuntimeStorageRegion::RuntimeFrame,
                            target.inner_index_offset,
                            target.inner_index_byte_size,
                            target.inner_stride,
                            target.field_byte_offset,
                        ),
                    });
                } else {
                    return false;
                }
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::StaticText => {
                let Some(literal) = input
                    .runtime_text
                    .expressions
                    .string_literal_value(segment.expression)
                else {
                    return false;
                };
                if !append_runtime_text_literal_to_target(
                    input,
                    source_key,
                    statement_index,
                    buffer,
                    target.place.as_ref(),
                    target.pointee.as_ref(),
                    target.indexed.as_ref(),
                    target.frame_base_indexed.as_ref(),
                    target.frame_base_double_indexed.as_ref(),
                    target.machine_indexed.as_ref(),
                    target.machine_double_indexed.as_ref(),
                    literal,
                    emit,
                ) {
                    return false;
                }
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::OtherExpression => {
                if !append_runtime_text_literal_to_target(
                    input,
                    source_key,
                    statement_index,
                    buffer,
                    target.place.as_ref(),
                    target.pointee.as_ref(),
                    target.indexed.as_ref(),
                    target.frame_base_indexed.as_ref(),
                    target.frame_base_double_indexed.as_ref(),
                    target.machine_indexed.as_ref(),
                    target.machine_double_indexed.as_ref(),
                    Arc::from(UNSUPPORTED_RUNTIME_TEXT_SEGMENT.as_bytes()),
                    emit,
                ) {
                    return false;
                }
                emitted = true;
            }
        }
    }

    emitted
}

fn append_runtime_text_literal_to_target(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    buffer: TargetDataObjectHandle,
    target_place: Option<&RuntimeStoragePlace>,
    target_pointee: Option<&RuntimePointeeTarget>,
    target_indexed: Option<&RuntimeFrameIndexedTarget>,
    target_frame_base_indexed: Option<&RuntimeFrameBaseIndexedTarget>,
    target_frame_base_double_indexed: Option<&RuntimeFrameBaseDoubleIndexedTarget>,
    target_machine_indexed: Option<&RuntimeMachineIndexedTarget>,
    target_machine_double_indexed: Option<&RuntimeMachineDoubleIndexedTarget>,
    literal: Arc<[u8]>,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    if let Some(target_place) = target_place {
        emit(SelectedInstructionKind::AppendTextLiteralToPlace {
            buffer,
            target: crate::selection::runtime_dispatch::text_place_direct(
                target_place.region,
                target_place.byte_offset,
            ),
            literal,
        });
    } else if let Some(target) = target_pointee {
        emit(SelectedInstructionKind::AppendTextLiteralToPlace {
            buffer,
            target: crate::selection::runtime_dispatch::text_place_pointee(
                target.pointer_byte_offset,
                target.field_byte_offset,
            ),
            literal,
        });
    } else if let Some(target) = target_indexed {
        emit(SelectedInstructionKind::AppendTextLiteralToPlace {
            buffer,
            target: crate::selection::runtime_dispatch::text_place_frame_indexed(
                target.descriptor_offset,
                target.index_region,
                target.index_offset,
                target.index_byte_size,
                target.element_byte_size,
                target.field_byte_offset,
            ),
            literal,
        });
    } else if let Some(target) = target_frame_base_indexed {
        emit(SelectedInstructionKind::AppendTextLiteralToPlace {
            buffer,
            target: crate::selection::runtime_dispatch::frame_base_indexed_place(
                target.base_byte_offset,
                target.index_offset,
                target.index_byte_size,
                target.element_byte_size,
                target.field_byte_offset,
            ),
            literal,
        });
    } else if let Some(target) = target_frame_base_double_indexed {
        emit(SelectedInstructionKind::AppendTextLiteralToPlace {
            buffer,
            target: crate::selection::runtime_dispatch::double_indexed_place(
                RuntimeStorageRegion::RuntimeFrame,
                target.base_byte_offset,
                RuntimeStorageRegion::RuntimeFrame,
                target.outer_index_offset,
                target.outer_index_byte_size,
                target.outer_stride,
                RuntimeStorageRegion::RuntimeFrame,
                target.inner_index_offset,
                target.inner_index_byte_size,
                target.inner_stride,
                target.field_byte_offset,
            ),
            literal,
        });
    } else if let Some(target) = target_machine_indexed {
        let data = string_literal_data_handle(input, source_key, statement_index, &literal);
        if !data.is_valid() {
            return false;
        }
        emit(
            crate::selection::runtime_dispatch::write_place_string_machine_indexed(
                target.base_byte_offset,
                target.index_region,
                target.index_offset,
                target.index_byte_size,
                target.element_byte_size,
                target.field_byte_offset,
                data,
                literal.len(),
            ),
        );
    } else if let Some(target) = target_machine_double_indexed {
        let data = string_literal_data_handle(input, source_key, statement_index, &literal);
        if !data.is_valid() {
            return false;
        }
        emit(
            crate::selection::runtime_dispatch::write_place_string_machine_double_indexed(
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
                data,
                literal.len(),
            ),
        );
    } else {
        return false;
    }

    true
}

fn initialize_runtime_text_target_with_first_literal_segment(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    buffer: TargetDataObjectHandle,
    target: &RuntimeTextTarget,
    segments: &[omega_runtime_text::RuntimeTextBuilderSegment],
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    let Some(first) = segments.first() else {
        return false;
    };
    if first.kind != RuntimeTextBuilderSegmentKind::StaticText {
        return false;
    }
    let Some(literal) = input
        .runtime_text
        .expressions
        .string_literal_value(first.expression)
    else {
        return false;
    };
    let data = string_literal_data_handle(input, source_key, statement_index, &literal);
    if !data.is_valid() {
        return false;
    }

    emit(SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
        buffer,
        byte_offset: 0,
        literal: literal.clone(),
    });

    if let Some(target_place) = target.place.as_ref() {
        if target_place.region != RuntimeStorageRegion::Machine {
            return false;
        }
        emit(
            crate::selection::runtime_dispatch::write_place_string_direct(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                target_place.byte_offset,
                data,
                literal.len(),
            ),
        );
    } else if let Some(target) = target.pointee.as_ref() {
        emit(
            crate::selection::runtime_dispatch::write_place_string_pointee(
                target.pointer_byte_offset,
                target.field_byte_offset,
                data,
                literal.len(),
            ),
        );
    } else if let Some(target) = target.indexed.as_ref() {
        emit(
            crate::selection::runtime_dispatch::write_place_string_frame_indexed(
                target.descriptor_offset,
                target.index_region,
                target.index_offset,
                target.index_byte_size,
                target.element_byte_size,
                target.field_byte_offset,
                data,
                literal.len(),
            ),
        );
    } else if let Some(target) = target.frame_base_indexed.as_ref() {
        emit(
            crate::selection::runtime_dispatch::write_place_string_frame_base_indexed(
                target.base_byte_offset,
                target.index_offset,
                target.index_byte_size,
                target.element_byte_size,
                target.field_byte_offset,
                data,
                literal.len(),
            ),
        );
    } else if let Some(target) = target.frame_base_double_indexed.as_ref() {
        emit(
            crate::selection::runtime_dispatch::write_place_string_frame_base_double_indexed(
                target.base_byte_offset,
                target.outer_index_offset,
                target.outer_index_byte_size,
                target.outer_stride,
                target.inner_index_offset,
                target.inner_index_byte_size,
                target.inner_stride,
                target.field_byte_offset,
                data,
                literal.len(),
            ),
        );
    } else if let Some(target) = target.machine_indexed.as_ref() {
        emit(
            crate::selection::runtime_dispatch::write_place_string_machine_indexed(
                target.base_byte_offset,
                target.index_region,
                target.index_offset,
                target.index_byte_size,
                target.element_byte_size,
                target.field_byte_offset,
                data,
                literal.len(),
            ),
        );
    } else if let Some(target) = target.machine_double_indexed.as_ref() {
        emit(
            crate::selection::runtime_dispatch::write_place_string_machine_double_indexed(
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
                data,
                literal.len(),
            ),
        );
    } else {
        return false;
    }

    true
}

fn runtime_text_input_buffer_for_builder(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<TargetDataObjectHandle> {
    input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.kind == TargetDataObjectKind::RuntimeTextBuffer
                && data_object.source_key == source_key
                && data_object.source_statement == statement_index
        })
        .map(|(handle, _)| handle)
}

#[allow(clippy::too_many_arguments)]
fn prefixed_stored_place_write_with_handle_resolver(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    buffer: TargetDataObjectHandle,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    prefix: &omega_runtime_text::RuntimeTextBuilderSegment,
    suffix: &omega_runtime_text::RuntimeTextBuilderSegment,
    resolved_segment_expressions: &mut ExpressionTable,
    resolve_expression: &dyn Fn(&mut ExpressionTable, ExpressionHandle) -> ExpressionHandle,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    let Some(prefix) = input
        .runtime_text
        .expressions
        .string_literal_value(prefix.expression)
    else {
        return false;
    };
    let suffix_expression =
        resolved_segment_expressions.copy_from(&input.runtime_text.expressions, suffix.expression);
    let source = resolve_expression(resolved_segment_expressions, suffix_expression);
    let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        resolved_segment_expressions,
        source,
    ) else {
        return false;
    };
    if source_place.byte_count != input.runtime_abi.string_descriptor_size() {
        return false;
    }

    emit(SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
        buffer,
        byte_offset: 0,
        literal: prefix.clone(),
    });
    emit(SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
        buffer,
        buffer_offset: prefix.len(),
        source_region: source_place.region,
        source_offset: source_place.byte_offset,
        target_region,
        target_offset,
        length_delta: prefix.len(),
    });
    true
}

#[allow(clippy::too_many_arguments)]
fn prefixed_stored_place_write_without_aliases(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    buffer: TargetDataObjectHandle,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    prefix: &omega_runtime_text::RuntimeTextBuilderSegment,
    suffix: &omega_runtime_text::RuntimeTextBuilderSegment,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    let Some(prefix) = input
        .runtime_text
        .expressions
        .string_literal_value(prefix.expression)
    else {
        return false;
    };
    let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        &input.runtime_text.expressions,
        suffix.expression,
    ) else {
        return false;
    };
    if source_place.byte_count != input.runtime_abi.string_descriptor_size() {
        return false;
    }

    emit(SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
        buffer,
        byte_offset: 0,
        literal: prefix.clone(),
    });
    emit(SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
        buffer,
        buffer_offset: prefix.len(),
        source_region: source_place.region,
        source_offset: source_place.byte_offset,
        target_region,
        target_offset,
        length_delta: prefix.len(),
    });
    true
}
