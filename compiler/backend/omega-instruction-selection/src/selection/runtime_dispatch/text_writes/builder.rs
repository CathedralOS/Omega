use super::string_literal_data_handle;
use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, resolve_runtime_alias_binding_handle,
    strip_mutable_expression,
};
use crate::selection::host_operations::runtime_text_input_buffer_data_for_text_place;
use crate::selection::host_operations::runtime_text_input_buffer_data_for_text_place_in_table;
use crate::selection::storage_places::{
    RuntimeFrameIndexedTarget, RuntimePointeeTarget, RuntimeStoragePlace,
    resolve_runtime_frame_indexed_target, resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset, resolve_runtime_pointee_slot_offset_in_table,
    resolve_runtime_storage_place, resolve_runtime_storage_place_in_table,
};
use omega_checked_trees::expression::{Expression, ExpressionHandle, ExpressionTable};
use omega_control_flow::StateKey;
use omega_runtime_text::RuntimeTextBuilderSegmentKind;
use omega_abstract_operations::{
    RuntimeStorageRegion, SelectedInstructionKind, TargetDataObjectHandle, TargetDataObjectKind,
};
use std::sync::Arc;

const UNSUPPORTED_RUNTIME_TEXT_SEGMENT: &str = "<expr>";

#[derive(Debug, Clone)]
struct RuntimeTextTarget {
    place: Option<RuntimeStoragePlace>,
    indexed: Option<RuntimeFrameIndexedTarget>,
    pointee: Option<RuntimePointeeTarget>,
}

impl RuntimeTextTarget {
    fn new(
        input: &InstructionSelectionInput<'_>,
        place: Option<RuntimeStoragePlace>,
        indexed: Option<RuntimeFrameIndexedTarget>,
        pointee: Option<RuntimePointeeTarget>,
    ) -> Option<Self> {
        if place.is_none() && pointee.is_none() && indexed.is_none() {
            return None;
        }
        if place
            .as_ref()
            .is_some_and(|place| place.byte_count != input.target.pointer_size * 2)
        {
            return None;
        }
        if pointee
            .as_ref()
            .is_some_and(|target| target.pointee_byte_size != input.target.pointer_size * 2)
        {
            return None;
        }
        if indexed
            .as_ref()
            .is_some_and(|target| target.byte_count != input.target.pointer_size * 2)
        {
            return None;
        }

        Some(Self {
            place,
            indexed,
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
    let target_pointee =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, &resolved_target);
    if target_place.is_none() && target_pointee.is_none() && target_indexed.is_none() {
        return false;
    }
    if target_place
        .as_ref()
        .is_some_and(|place| place.byte_count != input.target.pointer_size * 2)
    {
        return false;
    }
    if target_pointee
        .as_ref()
        .is_some_and(|target| target.pointee_byte_size != input.target.pointer_size * 2)
    {
        return false;
    }
    if target_indexed
        .as_ref()
        .is_some_and(|target| target.byte_count != input.target.pointer_size * 2)
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
                if source_place.byte_count != input.target.pointer_size * 2 {
                    return false;
                }
                if let Some(target_place) = target.place.as_ref() {
                    if source_place.region == target_place.region
                        && source_place.byte_offset == target_place.byte_offset
                    {
                        emit(SelectedInstructionKind::MaterializeRuntimeTextBuffer {
                            buffer,
                            target_region: target_place.region,
                            target_offset: target_place.byte_offset,
                        });
                        emitted = true;
                        continue;
                    }
                    emit(SelectedInstructionKind::AppendRuntimeTextStoredPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target_region: target_place.region,
                        target_offset: target_place.byte_offset,
                    });
                } else if let Some(target) = target.pointee.as_ref() {
                    emit(
                        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
                            buffer,
                            source_region: source_place.region,
                            source_offset: source_place.byte_offset,
                            pointer_byte_offset: target.pointer_byte_offset,
                            field_byte_offset: target.field_byte_offset,
                        },
                    );
                } else if let Some(target) = target.indexed.as_ref() {
                    emit(SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        descriptor_offset: target.descriptor_offset,
                        index_offset: target.index_offset,
                        element_byte_size: target.element_byte_size,
                        field_byte_offset: target.field_byte_offset,
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
                    buffer,
                    target.place.as_ref(),
                    target.pointee.as_ref(),
                    target.indexed.as_ref(),
                    literal,
                    emit,
                ) {
                    return false;
                }
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::OtherExpression => {
                if !append_runtime_text_literal_to_target(
                    buffer,
                    target.place.as_ref(),
                    target.pointee.as_ref(),
                    target.indexed.as_ref(),
                    Arc::from(UNSUPPORTED_RUNTIME_TEXT_SEGMENT),
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
    let target_pointee =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, &resolved_target);
    let Some(target) = RuntimeTextTarget::new(input, target_place, target_indexed, target_pointee)
    else {
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
    let target_pointee = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        target_expressions,
        resolved_target,
    );
    let Some(target) = RuntimeTextTarget::new(input, target_place, target_indexed, target_pointee)
    else {
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
                if source_place.byte_count != input.target.pointer_size * 2 {
                    return false;
                }
                if let Some(target_place) = target.place.as_ref() {
                    if source_place.region == target_place.region
                        && source_place.byte_offset == target_place.byte_offset
                    {
                        emit(SelectedInstructionKind::MaterializeRuntimeTextBuffer {
                            buffer,
                            target_region: target_place.region,
                            target_offset: target_place.byte_offset,
                        });
                        emitted = true;
                        continue;
                    }
                    emit(SelectedInstructionKind::AppendRuntimeTextStoredPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target_region: target_place.region,
                        target_offset: target_place.byte_offset,
                    });
                } else if let Some(target) = target.pointee.as_ref() {
                    emit(
                        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
                            buffer,
                            source_region: source_place.region,
                            source_offset: source_place.byte_offset,
                            pointer_byte_offset: target.pointer_byte_offset,
                            field_byte_offset: target.field_byte_offset,
                        },
                    );
                } else if let Some(target) = target.indexed.as_ref() {
                    emit(SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        descriptor_offset: target.descriptor_offset,
                        index_offset: target.index_offset,
                        element_byte_size: target.element_byte_size,
                        field_byte_offset: target.field_byte_offset,
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
                    buffer,
                    target.place.as_ref(),
                    target.pointee.as_ref(),
                    target.indexed.as_ref(),
                    literal,
                    emit,
                ) {
                    return false;
                }
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::OtherExpression => {
                if !append_runtime_text_literal_to_target(
                    buffer,
                    target.place.as_ref(),
                    target.pointee.as_ref(),
                    target.indexed.as_ref(),
                    Arc::from(UNSUPPORTED_RUNTIME_TEXT_SEGMENT),
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
    buffer: TargetDataObjectHandle,
    target_place: Option<&RuntimeStoragePlace>,
    target_pointee: Option<&RuntimePointeeTarget>,
    target_indexed: Option<&RuntimeFrameIndexedTarget>,
    literal: Arc<str>,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    if let Some(target_place) = target_place {
        emit(SelectedInstructionKind::AppendRuntimeTextLiteral {
            buffer,
            target_region: target_place.region,
            target_offset: target_place.byte_offset,
            literal,
        });
    } else if let Some(target) = target_pointee {
        emit(
            SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee {
                buffer,
                pointer_byte_offset: target.pointer_byte_offset,
                field_byte_offset: target.field_byte_offset,
                literal,
            },
        );
    } else if let Some(target) = target_indexed {
        emit(
            SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
                buffer,
                descriptor_offset: target.descriptor_offset,
                index_offset: target.index_offset,
                element_byte_size: target.element_byte_size,
                field_byte_offset: target.field_byte_offset,
                literal,
            },
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
        emit(SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset: target_place.byte_offset,
            data,
            byte_length: literal.len(),
        });
    } else if let Some(target) = target.pointee.as_ref() {
        emit(SelectedInstructionKind::WriteRuntimePointeeString {
            pointer_byte_offset: target.pointer_byte_offset,
            field_byte_offset: target.field_byte_offset,
            data,
            byte_length: literal.len(),
        });
    } else if let Some(target) = target.indexed.as_ref() {
        emit(SelectedInstructionKind::WriteRuntimeFrameIndexedString {
            descriptor_offset: target.descriptor_offset,
            index_offset: target.index_offset,
            element_byte_size: target.element_byte_size,
            field_byte_offset: target.field_byte_offset,
            data,
            byte_length: literal.len(),
        });
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
    if source_place.byte_count != input.target.pointer_size * 2 {
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
    if source_place.byte_count != input.target.pointer_size * 2 {
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
