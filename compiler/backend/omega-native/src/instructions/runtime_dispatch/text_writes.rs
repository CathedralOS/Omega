use crate::control_flow::StateKey;
use crate::data::NativeDataObject;
use crate::plan::NativePlan;
use crate::runtime_text::RuntimeTextBuilderSegmentKind;
use omega_typed_program::expression::Expression;

use super::super::bindings::{
    RuntimeAliasBinding, resolve_runtime_alias_expression, strip_mutable_expression,
};
use super::super::host_operations::runtime_text_input_buffer_for_text_place;
use super::super::model::{SelectedInstruction, SelectedInstructionKind};
use super::super::storage_places::{resolve_machine_owned_place, resolve_runtime_storage_place};

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_string_descriptor_write(
    native_plan: &NativePlan,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    value: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        resolved_target,
    ) else {
        return;
    };
    if byte_size != native_plan.target.pointer_size * 2 {
        return;
    }
    let Some(data_object) =
        string_literal_data_object(native_plan, source_key, statement_index, value)
    else {
        return;
    };

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            data_symbol: data_object.symbol.clone(),
            byte_length: value.len(),
        },
        source_machine: source_machine.to_owned().into(),
        source_state: source_state.to_owned().into(),
        source_statement: statement_index,
    });
}

pub(super) fn runtime_text_builder_write(
    native_plan: &NativePlan,
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
pub(super) fn runtime_text_builder_write_with_resolver(
    native_plan: &NativePlan,
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
    let buffer = runtime_text_input_buffer_for_text_place(native_plan, &resolved_target)?;
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
        return Some(vec![
            SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
                buffer_symbol: buffer.symbol.clone(),
                byte_offset: 0,
                literal: prefix.clone(),
            },
            SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
                buffer_symbol: buffer.symbol.clone(),
                buffer_offset: prefix.len(),
                source_symbol: source_place.symbol,
                source_offset: source_place.byte_offset,
                target_symbol: target_place.symbol,
                target_offset: target_place.byte_offset,
                length_delta: prefix.len(),
            },
        ]);
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
                if source_place.symbol == target_place.symbol
                    && source_place.byte_offset == target_place.byte_offset
                {
                    instructions.push(SelectedInstructionKind::MaterializeRuntimeTextBuffer {
                        buffer_symbol: buffer.symbol.clone(),
                        target_symbol: target_place.symbol.clone(),
                        target_offset: target_place.byte_offset,
                    });
                    continue;
                }
                instructions.push(SelectedInstructionKind::AppendRuntimeTextStoredPlace {
                    buffer_symbol: buffer.symbol.clone(),
                    source_symbol: source_place.symbol,
                    source_offset: source_place.byte_offset,
                    target_symbol: target_place.symbol.clone(),
                    target_offset: target_place.byte_offset,
                });
            }
            RuntimeTextBuilderSegmentKind::StaticText => {
                let Expression::String(literal) = &segment.expression else {
                    return None;
                };
                instructions.push(SelectedInstructionKind::AppendRuntimeTextLiteral {
                    buffer_symbol: buffer.symbol.clone(),
                    target_symbol: target_place.symbol.clone(),
                    target_offset: target_place.byte_offset,
                    literal: literal.clone(),
                });
            }
            RuntimeTextBuilderSegmentKind::OtherExpression => return None,
        }
    }

    (!instructions.is_empty()).then_some(instructions)
}

fn string_literal_data_object<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
    value: &str,
) -> Option<&'plan NativeDataObject> {
    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == source_key
                && data_object.source_statement == statement_index
                && native_plan
                    .data
                    .bytes
                    .span(data_object.bytes)
                    .is_some_and(|bytes| {
                        bytes == value.as_bytes() || (value.is_empty() && bytes == [0])
                    })
        })
        .map(|(_, data_object)| data_object)
}
