use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBinding, resolve_runtime_alias_expression, strip_mutable_expression,
};
use crate::selection::host_operations::runtime_text_input_buffer_data_for_text_place;
use crate::selection::storage_places::resolve_runtime_storage_place;
use omega_control_flow::StateKey;
use omega_runtime_text::RuntimeTextBuilderSegmentKind;
use omega_target_program::{RuntimeStorageRegion, SelectedInstructionKind, TargetDataObjectHandle};
use omega_typed_program::expression::{Expression, ExpressionTable};

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn runtime_text_builder_write_emit(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    runtime_text_builder_write_with_resolver_emit(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        &|expression| {
            resolve_runtime_alias_expression(expression, source_key, aliases, alias_expressions)
        },
        emit,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn runtime_text_builder_write_with_resolver_emit(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolve_expression: &dyn Fn(&Expression) -> Expression,
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
    let Some((buffer, _)) = runtime_text_input_buffer_data_for_text_place(input, &resolved_target)
    else {
        return false;
    };
    let Some(target_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &resolved_target,
    ) else {
        return false;
    };
    if target_place.byte_count != input.target.pointer_size * 2 {
        return false;
    }

    if let [prefix, suffix] = segments
        && prefix.kind == RuntimeTextBuilderSegmentKind::StaticText
        && suffix.kind == RuntimeTextBuilderSegmentKind::StoredPlace
    {
        return prefixed_stored_place_write(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            buffer,
            target_place.region,
            target_place.byte_offset,
            prefix,
            suffix,
            resolve_expression,
            emit,
        );
    }

    let mut emitted = false;
    for segment in segments {
        match segment.kind {
            RuntimeTextBuilderSegmentKind::StoredPlace => {
                let segment_expression = input.runtime_text.expressions.to_tree(segment.expression);
                let source = resolve_expression(&segment_expression);
                let Some(source_place) = resolve_runtime_storage_place(
                    input,
                    dispatch_index,
                    source_key,
                    source_machine,
                    source_state,
                    &source,
                ) else {
                    return false;
                };
                if source_place.byte_count != input.target.pointer_size * 2 {
                    return false;
                }
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
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::StaticText => {
                let Some(literal) = input
                    .runtime_text
                    .expressions
                    .string_literal(segment.expression)
                    .map(str::to_owned)
                else {
                    return false;
                };
                emit(SelectedInstructionKind::AppendRuntimeTextLiteral {
                    buffer,
                    target_region: target_place.region,
                    target_offset: target_place.byte_offset,
                    literal,
                });
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::OtherExpression => return false,
        }
    }

    emitted
}

#[allow(clippy::too_many_arguments)]
fn prefixed_stored_place_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    buffer: TargetDataObjectHandle,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    prefix: &omega_runtime_text::RuntimeTextBuilderSegment,
    suffix: &omega_runtime_text::RuntimeTextBuilderSegment,
    resolve_expression: &dyn Fn(&Expression) -> Expression,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    let Some(prefix) = input
        .runtime_text
        .expressions
        .string_literal(prefix.expression)
        .map(str::to_owned)
    else {
        return false;
    };
    let suffix_expression = input.runtime_text.expressions.to_tree(suffix.expression);
    let source = resolve_expression(&suffix_expression);
    let Some(source_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &source,
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
