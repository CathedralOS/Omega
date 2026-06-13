use super::writes::{
    emit_runtime_frame_slot_slice_descriptor_write_in_table,
    select_runtime_frame_slot_value_write_in_table_with_source_anchor,
};
use super::{guards::static_guard_conjunct_summary_in_table, state_key_matches_statement_source};
use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, resolve_runtime_alias_binding_handle,
};
use crate::selection::instruction_sink::SelectedInstructionSink;
use crate::selection::storage_places::{
    resolve_fixed_array_length_in_table, resolve_runtime_call_argument_call_result_place,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
    resolve_runtime_transition_argument_call_result_place,
};
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::statement::StatementNode;
use omega_control_flow::{StateKey, StateParameterFlow};
use omega_core::arena::Arena;
use omega_layout::{DataShape, ENUM_TAG_BYTES};
use omega_state_calls::{StateCallLowering, StateCallRole};

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_dispatch_argument_materialization(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    source_dispatch_index: u32,
    statement_index: usize,
    target_dispatch_index: u32,
    arguments: omega_core::arena::HandleSpan<ExpressionHandle>,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(target_key) = dispatch_key_for_index(input, target_dispatch_index) else {
        return;
    };
    let Some(target_state) = input.control_flow.state_by_key(target_key) else {
        return;
    };
    let expressions = &input.control_flow.expressions;
    let target_arguments = expressions.expression_handles(arguments);
    let static_values =
        super::writes::RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    let mut resolved_argument_expressions = ExpressionTable::with_expression_capacity(
        target_arguments
            .len()
            .saturating_add(alias_expressions.expression_count())
            .saturating_add(4),
    );

    // Decide whether to STAGE the arguments through a scratch region. A same-call
    // -context transition's source and target frames OVERLAP (states in one context
    // share a frame region), so writing one parameter slot can clobber another
    // argument's still-unread source -- a slice/scalar copy cycle. When any target
    // slot overlaps another argument's frame source, copy every argument to scratch
    // first (source -> scratch, all reads safe) then scratch -> target (all writes
    // safe), instead of writing targets directly.
    let stage = input.runtime_storage.frame_scratch_base != 0 && {
        let mut ranges: Vec<((usize, usize), Option<(usize, usize)>)> = Vec::new();
        for (parameter_index, parameter) in input
            .control_flow
            .state_parameters(target_state)
            .iter()
            .enumerate()
        {
            let Some(argument) = target_arguments.get(parameter_index).copied() else {
                break;
            };
            let Some(slot) = runtime_parameter_slot(input, target_dispatch_index, parameter) else {
                continue;
            };
            let target = (slot.byte_offset, slot.byte_offset + slot.byte_size);
            let source = argument_source_frame_range(
                input,
                source_key,
                statement_index,
                source_dispatch_index,
                argument,
                aliases,
                alias_expressions,
            );
            ranges.push((target, source));
        }
        ranges.iter().enumerate().any(|(i, (target, _))| {
            ranges.iter().enumerate().any(|(j, (_, source))| {
                i != j && source.is_some_and(|source| ranges_overlap(*target, source))
            })
        })
    };
    let mut scratch_cursor = input.runtime_storage.frame_scratch_base;
    let mut staged_copies: Vec<(usize, usize, usize)> = Vec::new();

    for (parameter_index, parameter) in input
        .control_flow
        .state_parameters(target_state)
        .iter()
        .enumerate()
    {
        let Some(argument) = target_arguments.get(parameter_index).copied() else {
            break;
        };
        let Some(slot) = runtime_parameter_slot(input, target_dispatch_index, parameter) else {
            continue;
        };
        let source_anchor_byte_offset = slot.byte_offset;
        let real_target_range = (slot.byte_offset, slot.byte_offset + slot.byte_size);
        // When staging, redirect this argument's write to a scratch slot (a clone of
        // the real slot at a packed scratch offset) and remember the scratch->target
        // copy to emit after every source has been read.
        let scratch_slot;
        let slot = if stage {
            scratch_cursor = scratch_cursor.next_multiple_of(slot.alignment.max(1));
            let scratch_offset = scratch_cursor;
            scratch_cursor += slot.byte_size;
            staged_copies.push((scratch_offset, slot.byte_offset, slot.byte_size));
            scratch_slot = {
                let mut redirected = slot.clone();
                redirected.byte_offset = scratch_offset;
                redirected
            };
            &scratch_slot
        } else {
            slot
        };

        resolved_argument_expressions.clear();
        let copied_aliases = RuntimeAliasBuffer::copy_from_bindings(
            alias_expressions,
            aliases,
            &mut resolved_argument_expressions,
        );
        let copied_argument = resolved_argument_expressions.copy_from(expressions, argument);
        let resolved_argument = resolve_runtime_alias_binding_handle(
            copied_argument,
            source_key,
            copied_aliases.bindings(),
            &mut resolved_argument_expressions,
        );
        let argument_source_key = resolved_argument.source_key;
        let argument = resolve_prior_local_initializers_in_table(
            input,
            argument_source_key,
            statement_index,
            &mut resolved_argument_expressions,
            resolved_argument.expression,
        );
        let expressions = &resolved_argument_expressions;

        if emit_runtime_detached_frame_slice_argument_materialization(
            input,
            argument_source_key,
            statement_index,
            source_dispatch_index,
            expressions,
            argument,
            slot,
            real_target_range,
            &mut scratch_cursor,
            selected_instructions,
        ) {
            continue;
        }

        // Slice-descriptor argument (an `as_slice()` view or a subslice of a
        // runtime-length slice, including the self-recursive `decreases … Length`
        // shape where source slot == target slot): one seam covers every
        // descriptor-construction strategy.
        if emit_runtime_frame_slot_slice_descriptor_write_in_table(
            input,
            source_dispatch_index,
            argument_source_key,
            statement_index,
            expressions,
            slot,
            argument,
            runtime_value_operands,
            selected_instructions,
        ) {
            continue;
        }

        if matches!(expressions.expression(argument), ExpressionNode::Call(_))
            && let Some(place) = resolve_runtime_transition_argument_call_result_place(
                input,
                source_dispatch_index,
                argument_source_key,
                statement_index,
            )
            .or_else(|| {
                resolve_runtime_call_argument_call_result_place(
                    input,
                    source_dispatch_index,
                    argument_source_key,
                    statement_index,
                )
            })
        {
            if place.byte_count != slot.byte_size {
                continue;
            }
            if let ExpressionNode::Call(call) = expressions.expression(argument) {
                materialize_static_inline_branching_call_argument_result(
                    input,
                    source_key,
                    source_dispatch_index,
                    statement_index,
                    call,
                    &static_values,
                    runtime_value_operands,
                    selected_instructions,
                );
            }
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorage {
                    source_region: place.region,
                    source_offset: place.byte_offset,
                    target_region: RuntimeStorageRegion::RuntimeFrame,
                    target_offset: slot.byte_offset,
                    byte_count: slot.byte_size,
                },
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        if emit_runtime_fixed_array_slice_argument_materialization(
            input,
            argument_source_key,
            statement_index,
            source_dispatch_index,
            expressions,
            argument,
            slot,
            selected_instructions,
        ) {
            continue;
        }

        if let Some(initial_value) = source_local_initial_value(
            input,
            argument_source_key,
            statement_index,
            expressions,
            argument,
        ) && emit_runtime_fixed_array_slice_argument_materialization(
            input,
            argument_source_key,
            statement_index,
            source_dispatch_index,
            &input.program.expression_table,
            initial_value,
            slot,
            selected_instructions,
        ) {
            continue;
        }

        if let Some(pointee) = resolve_runtime_pointee_slot_offset_in_table(
            input,
            source_dispatch_index,
            argument_source_key,
            expressions,
            argument,
        ) && pointee.pointee_byte_size == slot.byte_size
            && pointee.pointee_byte_size > 0
            && !matches!(expressions.expression(argument), ExpressionNode::Mutable(_))
            // Only DEREF into a VALUE parameter slot. When the target parameter is
            // itself a reference (`&mut T`), forwarding another reference argument
            // (e.g. an `out_room: &mut Room` param passed on to a sub-state's
            // `out_room: &mut Room`) must copy the POINTER VALUE, not the pointee --
            // otherwise the 8-byte referent is written into the pointer slot (a
            // coincidental size match when the referent is pointer-sized) and the
            // bogus "pointer" faults on the next deref. Fall through to the storage
            // -place copy below, which copies the pointer value.
            && slot.type_descriptor.reference_referee().is_none()
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimePointeeToRuntimeFrame {
                    pointer_byte_offset: pointee.pointer_byte_offset,
                    field_byte_offset: pointee.field_byte_offset,
                    target_offset: slot.byte_offset,
                    byte_count: slot.byte_size,
                },
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        // A constant-index slice element (`items[0].value` where `items` is a
        // frame-slot slice DESCRIPTOR, the self-recursive threaded-scalar shape
        // `self.accumulate(items[1..], items[0].value)`): the element lives behind
        // the descriptor's data pointer, so this is an indexed copy THROUGH the
        // descriptor — never a plain place copy, which would read the descriptor
        // slot's own bytes (the data pointer) as the value. The resolver returns
        // None for inline fixed arrays; those keep their direct-place paths.
        if let Some(indexed_source) = resolve_runtime_frame_fixed_indexed_target_in_table(
            input,
            source_dispatch_index,
            argument_source_key,
            expressions,
            argument,
        ) && indexed_source.byte_count == slot.byte_size
            && indexed_source.byte_count > 0
            && !matches!(expressions.expression(argument), ExpressionNode::Mutable(_))
            && slot.type_descriptor.reference_referee().is_none()
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
                    descriptor_offset: indexed_source.descriptor_offset,
                    element_index: indexed_source.element_index,
                    element_byte_size: indexed_source.element_byte_size,
                    field_byte_offset: indexed_source.field_byte_offset,
                    target_offset: slot.byte_offset,
                    byte_count: slot.byte_size,
                },
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        if let Some(place) = resolve_runtime_storage_place_in_table(
            input,
            source_dispatch_index,
            argument_source_key,
            expressions,
            argument,
        ) && place.byte_count == slot.byte_size
            && !matches!(expressions.expression(argument), ExpressionNode::Mutable(_))
        {
            // Same-sized place: copy the value into the parameter slot. A
            // size MISMATCH (e.g. a 16-byte String place feeding an 8-byte
            // `&mut String` reference parameter) is NOT a copy -- it falls
            // through to the address-write strategy below, which stores the
            // referent's address into the pointer slot.
            //
            // A `&mut x` argument (a `Mutable` expression) is ALSO never a value
            // copy, even when the referent happens to be pointer-sized (e.g.
            // `&mut SomeEightByteStruct`): it must store the referent's ADDRESS, so
            // it likewise falls through to the address-write strategy below.
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorage {
                    source_region: place.region,
                    source_offset: place.byte_offset,
                    target_region: RuntimeStorageRegion::RuntimeFrame,
                    target_offset: slot.byte_offset,
                    byte_count: slot.byte_size,
                },
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        if let Some(initial_value) = source_local_initial_value(
            input,
            argument_source_key,
            statement_index,
            expressions,
            argument,
        ) && let Some(kind) = select_runtime_frame_slot_value_write_in_table_with_source_anchor(
            input,
            source_dispatch_index,
            argument_source_key,
            statement_index,
            &input.program.expression_table,
            slot,
            initial_value,
            &static_values,
            runtime_value_operands,
            source_anchor_byte_offset,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        if let Some(value) = static_runtime_argument_value(expressions.expression(argument)) {
            if !matches!(slot.byte_size, 1 | 2 | 4 | 8) {
                continue;
            }

            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                    target_region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: slot.byte_offset,
                    byte_size: slot.byte_size,
                    value,
                },
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        // A case-bearing struct literal argument (`Event::Insert { cents: 50 }`):
        // write the enum tag and each payload field directly into the parameter
        // slot. This path fires for InlineBranching calls where the argument is a
        // StructLiteral expression (not yet in a local that resolve_prior_local_
        // initializers_in_table would have folded away -- those are blocked by
        // initial_value_blocks_inline_fold returning true, so they arrive here as
        // a Name whose frame slot already holds the pre-populated aggregate).
        // Note: if the argument arrived here as a Name (folded or blocked), it was
        // already handled by the CopyRuntimeStorage place path above.  This
        // branch covers the case where the StructLiteral IS the unfolded expression.
        if let ExpressionNode::StructLiteral(struct_literal) =
            expressions.expression(argument).clone()
        {
            // Write the case tag (i32 at offset 0 within the enum slot).
            if let Some(case_name) = &struct_literal.case_name {
                let tag: Option<i64> = {
                    let type_name = &struct_literal.type_name;
                    input
                        .layouts
                        .data_layouts
                        .iter()
                        .find(|(_, dl)| dl.name == *type_name)
                        .and_then(|(_, dl)| {
                            if let DataShape::Enum { variants, .. } = &dl.shape {
                                input
                                    .layouts
                                    .variants
                                    .span_or_empty(*variants)
                                    .iter()
                                    .position(|v| v.name == *case_name)
                                    .and_then(|i| i64::try_from(i).ok())
                            } else {
                                None
                            }
                        })
                };
                if let Some(tag_value) = tag {
                    selected_instructions.push(SelectedInstruction {
                        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                            target_region: RuntimeStorageRegion::RuntimeFrame,
                            byte_offset: slot.byte_offset,
                            byte_size: ENUM_TAG_BYTES,
                            value: tag_value,
                        },
                        source_key,
                        source_statement: statement_index,
                    });
                }

                // Write each payload field. Field offsets in VariantLayout are
                // ABSOLUTE within the enum value (0 = start of the tag), so the
                // frame address of a field is slot.byte_offset + field.offset.
                let variant_fields: Vec<(omega_checked_trees::name::Identifier, usize, usize)> = {
                    let type_name = &struct_literal.type_name;
                    input
                        .layouts
                        .data_layouts
                        .iter()
                        .find(|(_, dl)| dl.name == *type_name)
                        .and_then(|(_, dl)| {
                            if let DataShape::Enum { variants, .. } = &dl.shape {
                                input
                                    .layouts
                                    .variants
                                    .span_or_empty(*variants)
                                    .iter()
                                    .find(|v| v.name == *case_name)
                                    .map(|variant| {
                                        input
                                            .layouts
                                            .fields
                                            .span_or_empty(variant.fields)
                                            .iter()
                                            .map(|f| (f.name.clone(), f.offset, f.layout.size))
                                            .collect()
                                    })
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default()
                };

                for offset in 0..struct_literal.fields.count() {
                    let field = expressions
                        .struct_field_at_offset(struct_literal.fields, offset)
                        .clone();
                    // Find the matching layout entry for this field name.
                    let Some((_, field_offset, field_size)) =
                        variant_fields.iter().find(|(name, _, _)| *name == field.name)
                    else {
                        continue;
                    };
                    let frame_offset = slot.byte_offset + field_offset;
                    // Fast path: integer/bool literal.
                    if let Some(int_val) =
                        static_runtime_argument_value(expressions.expression(field.value))
                    {
                        if matches!(field_size, 1 | 2 | 4 | 8) {
                            selected_instructions.push(SelectedInstruction {
                                kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                                    target_region: RuntimeStorageRegion::RuntimeFrame,
                                    byte_offset: frame_offset,
                                    byte_size: *field_size,
                                    value: int_val,
                                },
                                source_key,
                                source_statement: statement_index,
                            });
                        }
                        continue;
                    }
                    // General path: synthesize a temporary slot at the field's
                    // frame position and delegate to the standard scalar writer.
                    let mut field_slot = slot.clone();
                    field_slot.byte_offset = frame_offset;
                    field_slot.byte_size = *field_size;
                    if let Some(kind) = select_runtime_frame_slot_value_write_in_table_with_source_anchor(
                        input,
                        source_dispatch_index,
                        argument_source_key,
                        statement_index,
                        expressions,
                        &field_slot,
                        field.value,
                        &static_values,
                        runtime_value_operands,
                        frame_offset,
                    ) {
                        selected_instructions.push(SelectedInstruction {
                            kind,
                            source_key,
                            source_statement: statement_index,
                        });
                    }
                }
                continue;
            }
        }

        if let Some(kind) = select_runtime_frame_slot_value_write_in_table_with_source_anchor(
            input,
            source_dispatch_index,
            argument_source_key,
            statement_index,
            expressions,
            slot,
            argument,
            &static_values,
            runtime_value_operands,
            source_anchor_byte_offset,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: statement_index,
            });
        }
    }

    // Phase B of staging: every argument is now in scratch (all sources were read
    // in phase A). Copy each from scratch into its real parameter slot. Scratch is
    // disjoint from all real slots, so these copies cannot clobber one another.
    for (scratch_offset, target_offset, byte_count) in staged_copies {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyRuntimeStorage {
                source_region: RuntimeStorageRegion::RuntimeFrame,
                source_offset: scratch_offset,
                target_region: RuntimeStorageRegion::RuntimeFrame,
                target_offset,
                byte_count,
            },
            source_key,
            source_statement: statement_index,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn materialize_static_inline_branching_call_argument_result(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    dispatch_index: u32,
    statement_index: usize,
    call: &omega_checked_trees::expression::TableCallExpression,
    static_values: &super::writes::RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some(state_call) = input.state_calls.calls.iter().find_map(|(_, state_call)| {
        let (_, target_state) = input
            .control_flow
            .state_names_by_key_cloned(state_call.target_key);
        let target_matches = state_call.target_key.state == call.target_symbol
            || target_state.as_str() == &*call.target;
        (state_call.role == StateCallRole::CallArgument
            && state_call.lowering == StateCallLowering::InlineBranching
            && state_key_matches_statement_source(state_call.source_key, source_key)
            && state_call.statement_index == statement_index
            && target_matches)
            .then_some(state_call)
    }) else {
        return false;
    };

    let Some(slot) = input.runtime_storage.call_result_slot_by_ordinal(
        dispatch_index,
        state_call.source_key,
        state_call.statement_index,
        state_call.role,
        state_call.call_ordinal,
    ) else {
        return false;
    };

    let siblings = input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice()
        .iter()
        .filter(|expansion| {
            expansion.dispatch_index == dispatch_index
                && state_key_matches_statement_source(expansion.source_key, state_call.source_key)
                && expansion.statement_index == state_call.statement_index
                && expansion.role == state_call.role
                && expansion.call_ordinal == state_call.call_ordinal
        })
        .collect::<Vec<_>>();

    let selected = siblings
        .iter()
        .copied()
        .find(|expansion| {
            let summary = static_guard_conjunct_summary_in_table(
                input,
                &input.runtime_branching_calls.expressions,
                expansion.resolved_guard,
            );
            expansion.target_value.is_valid()
                && !expansion.is_default_target
                && summary.has_true
                && !summary.has_false
        })
        .or_else(|| {
            siblings.iter().copied().find(|expansion| {
                if !expansion.target_value.is_valid() || !expansion.is_default_target {
                    return false;
                }
                let summary = static_guard_conjunct_summary_in_table(
                    input,
                    &input.runtime_branching_calls.expressions,
                    expansion.resolved_guard,
                );
                !summary.has_false
                    && siblings
                        .iter()
                        .filter(|sibling| {
                            sibling.target_value.is_valid() && !sibling.is_default_target
                        })
                        .all(|sibling| {
                            static_guard_conjunct_summary_in_table(
                                input,
                                &input.runtime_branching_calls.expressions,
                                sibling.resolved_guard,
                            )
                            .has_false
                        })
            })
        });
    let Some(expansion) = selected else {
        return false;
    };

    let Some(kind) = select_runtime_frame_slot_value_write_in_table_with_source_anchor(
        input,
        dispatch_index,
        expansion.branch_key,
        expansion.target_statement_index,
        &input.runtime_branching_calls.expressions,
        slot,
        expansion.target_value,
        static_values,
        runtime_value_operands,
        slot.byte_offset,
    ) else {
        return false;
    };

    selected_instructions.push(SelectedInstruction {
        kind,
        source_key: expansion.branch_key,
        source_statement: expansion.target_statement_index,
    });
    true
}

/// The frame byte range an argument READS from, if it resolves to a runtime-frame
/// place (so a parameter-slot write could clobber it). `None` for immediates and
/// non-frame (machine-owned/static) sources, which cannot be clobbered.
#[allow(clippy::too_many_arguments)]
fn argument_source_frame_range(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    source_dispatch_index: u32,
    raw_argument: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
) -> Option<(usize, usize)> {
    let mut scratch = ExpressionTable::with_expression_capacity(
        alias_expressions.expression_count().saturating_add(4),
    );
    let copied_aliases =
        RuntimeAliasBuffer::copy_from_bindings(alias_expressions, aliases, &mut scratch);
    let copied_argument = scratch.copy_from(&input.control_flow.expressions, raw_argument);
    let resolved = resolve_runtime_alias_binding_handle(
        copied_argument,
        source_key,
        copied_aliases.bindings(),
        &mut scratch,
    );
    let argument_source_key = resolved.source_key;
    let argument = resolve_prior_local_initializers_in_table(
        input,
        argument_source_key,
        statement_index,
        &mut scratch,
        resolved.expression,
    );
    if let Some(place) = resolve_runtime_storage_place_in_table(
        input,
        source_dispatch_index,
        argument_source_key,
        &scratch,
        argument,
    ) {
        return (place.region == RuntimeStorageRegion::RuntimeFrame)
            .then_some((place.byte_offset, place.byte_offset + place.byte_count));
    }

    // A constant-index slice element (`items[0].value`) has no static frame
    // place, but its read still goes THROUGH the descriptor slot's data pointer.
    // Report the descriptor slot as the frame source so a transition that also
    // RETARGETS that descriptor (`self.accumulate(items[1..], items[0].value)`)
    // sees the overlap and stages: an unstaged in-place descriptor update would
    // shrink the window BEFORE the element read, fetching the next window's head.
    let indexed = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        source_dispatch_index,
        argument_source_key,
        &scratch,
        argument,
    )?;
    Some((
        indexed.descriptor_offset,
        indexed.descriptor_offset + input.runtime_abi.pointer_size,
    ))
}

fn ranges_overlap(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

pub(super) fn static_runtime_argument_value(expression: &ExpressionNode) -> Option<i64> {
    match expression {
        ExpressionNode::Integer(value) => Some(*value),
        ExpressionNode::Boolean(value) => Some(i64::from(*value)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_runtime_fixed_array_slice_argument_materialization(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    expressions: &ExpressionTable,
    argument: ExpressionHandle,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if slot.byte_size != input.runtime_abi.slice_descriptor_size() {
        return false;
    }

    let ExpressionNode::Call(call) = expressions.expression(argument) else {
        return false;
    };
    if !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || (call.target.as_str() != "as_slice" && call.target.as_str() != "as_mut_slice")
    {
        return false;
    }

    let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        call.receiver,
    ) else {
        return false;
    };
    let Some(length) = resolve_fixed_array_length_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        call.receiver,
    ) else {
        return false;
    };

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            target_offset: slot.byte_offset,
        },
        source_key,
        source_statement: statement_index,
    });
    let descriptor = input.runtime_abi.slice_descriptor();
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset + descriptor.len_offset(),
            byte_size: descriptor.len_size(),
            value: length as i64,
        },
        source_key,
        source_statement: statement_index,
    });
    true
}

#[allow(clippy::too_many_arguments)]
fn emit_runtime_detached_frame_slice_argument_materialization(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    expressions: &ExpressionTable,
    argument: ExpressionHandle,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    real_target_range: (usize, usize),
    scratch_cursor: &mut usize,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if input.runtime_storage.frame_scratch_base == 0
        || slot.byte_size != input.runtime_abi.slice_descriptor_size()
    {
        return false;
    }

    let ExpressionNode::Call(call) = expressions.expression(argument) else {
        return false;
    };
    if !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || (call.target.as_str() != "as_slice" && call.target.as_str() != "as_mut_slice")
    {
        return false;
    }

    let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        call.receiver,
    ) else {
        return false;
    };
    if source_place.region != RuntimeStorageRegion::RuntimeFrame {
        return false;
    }

    let source_range = (
        source_place.byte_offset,
        source_place.byte_offset + source_place.byte_count,
    );
    if !ranges_overlap(real_target_range, source_range) {
        return false;
    }

    let Some(length) = resolve_fixed_array_length_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        call.receiver,
    ) else {
        return false;
    };

    *scratch_cursor = scratch_cursor.next_multiple_of(slot.alignment.max(1));
    let scratch_offset = *scratch_cursor;
    *scratch_cursor += source_place.byte_count;

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::CopyRuntimeStorage {
            source_region: RuntimeStorageRegion::RuntimeFrame,
            source_offset: source_place.byte_offset,
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset: scratch_offset,
            byte_count: source_place.byte_count,
        },
        source_key,
        source_statement: statement_index,
    });
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region: RuntimeStorageRegion::RuntimeFrame,
            source_offset: scratch_offset,
            target_offset: slot.byte_offset,
        },
        source_key,
        source_statement: statement_index,
    });
    let descriptor = input.runtime_abi.slice_descriptor();
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset + descriptor.len_offset(),
            byte_size: descriptor.len_size(),
            value: length as i64,
        },
        source_key,
        source_statement: statement_index,
    });
    true
}

/// Whether a local with this `initial_value` must NOT be folded back into its
/// initializer during argument resolution -- because it owns a frame slot whose
/// address an arg may take. True for a real result-producing call (a call-result
/// slot; but NOT `as_slice`/`as_mut_slice` views, which must stay folded so the
/// slice materialization sees the receiver) and for aggregate literals (array /
/// struct, which are materialized into a frame slot).
fn initial_value_blocks_inline_fold(
    input: &InstructionSelectionInput<'_>,
    initial_value: ExpressionHandle,
) -> bool {
    match input.program.expression_table.expression(initial_value) {
        ExpressionNode::Call(call) => !matches!(call.target.as_str(), "as_slice" | "as_mut_slice"),
        ExpressionNode::ArrayLiteral(_) | ExpressionNode::StructLiteral(_) => true,
        _ => false,
    }
}

fn source_local_initial_value(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    argument: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let (local_symbol, local_name) = local_root_identity(expressions, argument)?;
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)?;

    input
        .program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local_data)
                if local_data.initial_value.is_valid()
                    && ((local_symbol.is_valid() && local_data.symbol == local_symbol)
                        || local_data.name == local_name) =>
            {
                Some(local_data.initial_value)
            }
            _ => None,
        })
}

fn resolve_prior_local_initializers_in_table(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expressions: &mut ExpressionTable,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    match expressions.expression(expression).clone() {
        ExpressionNode::Name(_) => {
            let Some(initial_value) = source_local_initial_value(
                input,
                source_key,
                statement_index,
                expressions,
                expression,
            ) else {
                return expression;
            };
            // A local that has its OWN frame slot must NOT be folded back into its
            // initializer: the fold destroys the place, so an arg like
            // `&mut room.event` or `&mut rooms[0]` -- whose root is such a local --
            // resolves to no place and the callee's parameter slot is left null.
            // Keep it as a Name so it resolves to its slot. This covers:
            //   - result-producing calls (`room = self.room_mut(..)`), which have a
            //     call-result slot (but NOT `as_slice`/`as_mut_slice` views, which
            //     must stay folded so the slice materialization sees the receiver);
            //   - aggregate literals (`rooms = [..]`, `r = Foo{..}`), which are
            //     stored in a frame slot (the state-values binding layer likewise
            //     refuses to inline these).
            if initial_value_blocks_inline_fold(input, initial_value) {
                return expression;
            }
            let copied = expressions.copy_from(&input.program.expression_table, initial_value);
            resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                copied,
            )
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                indexed.collection,
            );
            let index = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                indexed.index,
            );
            if collection == indexed.collection && index == indexed.index {
                expression
            } else {
                expressions.insert(ExpressionNode::Indexed(
                    omega_checked_trees::expression::TableIndexedExpression { collection, index },
                ))
            }
        }
        ExpressionNode::Range(range) => {
            let start = if range.start.is_valid() {
                resolve_prior_local_initializers_in_table(
                    input,
                    source_key,
                    statement_index,
                    expressions,
                    range.start,
                )
            } else {
                range.start
            };
            let end = if range.end.is_valid() {
                resolve_prior_local_initializers_in_table(
                    input,
                    source_key,
                    statement_index,
                    expressions,
                    range.end,
                )
            } else {
                range.end
            };
            if start == range.start && end == range.end {
                expression
            } else {
                expressions.insert(ExpressionNode::Range(
                    omega_checked_trees::expression::TableRangeExpression {
                        start,
                        end,
                        end_inclusive: range.end_inclusive,
                    },
                ))
            }
        }
        ExpressionNode::Call(call) => {
            let receiver = if call.receiver.is_valid() {
                resolve_prior_local_initializers_in_table(
                    input,
                    source_key,
                    statement_index,
                    expressions,
                    call.receiver,
                )
            } else {
                call.receiver
            };
            let arguments = expressions.reserve_expression_handles(call.arguments.count());
            let mut changed = receiver != call.receiver;
            for offset in 0..call.arguments.count() {
                let argument = expressions.expression_handle_at_offset(call.arguments, offset);
                let resolved_argument = resolve_prior_local_initializers_in_table(
                    input,
                    source_key,
                    statement_index,
                    expressions,
                    argument,
                );
                changed |= resolved_argument != argument;
                expressions.set_expression_handle_at_offset(arguments, offset, resolved_argument);
            }
            if !changed {
                expression
            } else {
                expressions.insert(ExpressionNode::Call(
                    omega_checked_trees::expression::TableCallExpression {
                        receiver,
                        target_symbol: call.target_symbol,
                        target: call.target,
                        arguments,
                    },
                ))
            }
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                member.receiver,
            );
            if receiver == member.receiver {
                expression
            } else {
                expressions.insert(ExpressionNode::Member(
                    omega_checked_trees::expression::TableMemberExpression {
                        receiver,
                        member_symbol: member.member_symbol,
                        member: member.member,
                    },
                ))
            }
        }
        ExpressionNode::Mutable(inner) => {
            let resolved = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                inner,
            );
            if resolved == inner {
                expression
            } else {
                expressions.insert(ExpressionNode::Mutable(resolved))
            }
        }
        _ => expression,
    }
}

fn local_root_identity(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<(
    omega_core::symbols::SymbolHandle,
    omega_checked_trees::name::Identifier,
)> {
    match expressions.expression(expression) {
        ExpressionNode::Name(path) => Some((
            path.head_symbol,
            expressions.name_path_members(path.members).first()?.clone(),
        )),
        ExpressionNode::Member(member) => local_root_identity(expressions, member.receiver),
        ExpressionNode::Indexed(indexed) => local_root_identity(expressions, indexed.collection),
        ExpressionNode::Mutable(inner) => local_root_identity(expressions, *inner),
        _ => None,
    }
}

fn dispatch_key_for_index(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
) -> Option<StateKey> {
    input
        .runtime_dispatch_loop
        .cases
        .iter()
        .find_map(|(_, case)| (case.dispatch_index == dispatch_index).then_some(case.key))
}

fn runtime_parameter_slot<'a>(
    input: &'a InstructionSelectionInput<'a>,
    target_dispatch_index: u32,
    parameter: &StateParameterFlow,
) -> Option<&'a omega_runtime_storage::RuntimeFrameSlot> {
    input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == target_dispatch_index && slot.symbol == parameter.symbol)
                .then_some(slot)
        })
}
