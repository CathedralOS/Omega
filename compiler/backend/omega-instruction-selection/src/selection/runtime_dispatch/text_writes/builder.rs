use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBinding, resolve_runtime_alias_expression, strip_mutable_expression,
};
use crate::selection::host_operations::runtime_text_input_buffer_data_for_text_place;
use crate::selection::storage_places::resolve_runtime_storage_place;
use omega_control_flow::StateKey;
use omega_runtime_text::RuntimeTextBuilderSegmentKind;
use omega_target_program::{RuntimeStorageRegion, SelectedInstructionKind, TargetDataObjectHandle};
use omega_typed_program::expression::Expression;

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn runtime_text_builder_write(
    native_plan: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    aliases: &[RuntimeAliasBinding],
) -> Option<Vec<SelectedInstructionKind>> {
    runtime_text_builder_write_with_resolver(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        &|expression| resolve_runtime_alias_expression(expression, source_key, aliases),
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn runtime_text_builder_write_with_resolver(
    native_plan: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolve_expression: &dyn Fn(&Expression) -> Expression,
) -> Option<Vec<SelectedInstructionKind>> {
    let builder = native_plan
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| {
            builder.source_key == source_key && builder.statement_index == statement_index
        })
        .map(|(_, builder)| builder)?;
    let segments = native_plan
        .runtime_text
        .builder_segments
        .span(builder.segments)?;
    let resolved_target = strip_mutable_expression(resolved_target.clone());
    let (buffer, _) = runtime_text_input_buffer_data_for_text_place(native_plan, &resolved_target)?;
    let target_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &resolved_target,
    )?;
    if target_place.byte_count != native_plan.target.pointer_size * 2 {
        return None;
    }

    if let [prefix, suffix] = segments
        && prefix.kind == RuntimeTextBuilderSegmentKind::StaticText
        && suffix.kind == RuntimeTextBuilderSegmentKind::StoredPlace
    {
        return prefixed_stored_place_write(
            native_plan,
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
        );
    }

    let mut instructions = Vec::new();
    for segment in segments {
        match segment.kind {
            RuntimeTextBuilderSegmentKind::StoredPlace => {
                let source = resolve_expression(&segment.expression);
                let source_place = resolve_runtime_storage_place(
                    native_plan,
                    dispatch_index,
                    source_key,
                    source_machine,
                    source_state,
                    &source,
                )?;
                if source_place.byte_count != native_plan.target.pointer_size * 2 {
                    return None;
                }
                if source_place.region == target_place.region
                    && source_place.byte_offset == target_place.byte_offset
                {
                    instructions.push(SelectedInstructionKind::MaterializeRuntimeTextBuffer {
                        buffer,
                        target_region: target_place.region,
                        target_offset: target_place.byte_offset,
                    });
                    continue;
                }
                instructions.push(SelectedInstructionKind::AppendRuntimeTextStoredPlace {
                    buffer,
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    target_region: target_place.region,
                    target_offset: target_place.byte_offset,
                });
            }
            RuntimeTextBuilderSegmentKind::StaticText => {
                let Expression::String(literal) = &segment.expression else {
                    return None;
                };
                instructions.push(SelectedInstructionKind::AppendRuntimeTextLiteral {
                    buffer,
                    target_region: target_place.region,
                    target_offset: target_place.byte_offset,
                    literal: literal.clone(),
                });
            }
            RuntimeTextBuilderSegmentKind::OtherExpression => return None,
        }
    }

    (!instructions.is_empty()).then_some(instructions)
}

#[allow(clippy::too_many_arguments)]
fn prefixed_stored_place_write(
    native_plan: &InstructionSelectionInput<'_>,
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
) -> Option<Vec<SelectedInstructionKind>> {
    let Expression::String(prefix) = &prefix.expression else {
        return None;
    };
    let source = resolve_expression(&suffix.expression);
    let source_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &source,
    )?;
    if source_place.byte_count != native_plan.target.pointer_size * 2 {
        return None;
    }

    Some(vec![
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            buffer,
            byte_offset: 0,
            literal: prefix.clone(),
        },
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer,
            buffer_offset: prefix.len(),
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            target_region,
            target_offset,
            length_delta: prefix.len(),
        },
    ])
}
