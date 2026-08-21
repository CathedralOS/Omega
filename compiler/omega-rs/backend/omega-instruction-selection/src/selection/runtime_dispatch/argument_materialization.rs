use super::operation_aliases::{
    integer_landing_for_type_reference, stamp_anonymous_integer_landing_on_value_spine,
};
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
    enum_variant_value_in_table, resolve_fixed_array_length_in_table,
    resolve_runtime_call_argument_call_result_place,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
    resolve_runtime_transition_argument_call_result_place,
    resolve_runtime_transition_argument_call_result_place_by_rank,
};
use omega_abstract_operations::{RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction};
use omega_control_flow::{StateKey, StateParameterFlow};
use omega_layout::{DataShape, ENUM_TAG_BYTES};
use omega_state_calls::{StateCallLowering, StateCallRole};
use psi_arena::Arena;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_checked_trees::statement::StatementNode;

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_dispatch_argument_materialization(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    source_dispatch_index: u32,
    statement_index: usize,
    target_dispatch_index: u32,
    arguments: psi_arena::HandleSpan<ExpressionHandle>,
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
        let mut ranges: Vec<((usize, usize), Vec<(usize, usize)>)> = Vec::new();
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
            if slot.byte_size == 0 && slot.is_static_boundary_capability {
                continue;
            }
            let target = (slot.byte_offset, slot.byte_offset + slot.byte_size);
            let sources = argument_source_frame_ranges(
                input,
                source_key,
                statement_index,
                source_dispatch_index,
                argument,
                aliases,
                alias_expressions,
            );
            ranges.push((target, sources));
        }
        ranges.iter().enumerate().any(|(i, (target, _))| {
            ranges.iter().enumerate().any(|(j, (_, sources))| {
                i != j
                    && sources
                        .iter()
                        .any(|source| ranges_overlap(*target, *source))
            })
        })
    };
    let mut scratch_cursor = input.runtime_storage.frame_scratch_base;
    let mut staged_copies: Vec<(usize, usize, usize)> = Vec::new();
    // Rank of the next MACHINE-value-call argument (consumed left to right):
    // the Nth such argument reads the Nth transition-argument call record's
    // result slot. Builtin call arguments (`.unwrap()`) have no record and do
    // not consume a rank.
    let mut transition_call_argument_rank = 0usize;

    for (parameter_index, parameter) in input
        .control_flow
        .state_parameters(target_state)
        .iter()
        .enumerate()
    {
        let Some(argument) = target_arguments.get(parameter_index).copied() else {
            break;
        };
        if std::env::var_os("OMEGA_DEBUG_SUBSLICE").is_some()
            && let Some(slot) = runtime_parameter_slot(input, target_dispatch_index, parameter)
        {
            eprintln!(
                "materialize param `{}` -> target dispatch {} offset {} size {}",
                parameter.name, target_dispatch_index, slot.byte_offset, slot.byte_size,
            );
        }
        let Some(slot) = runtime_parameter_slot(input, target_dispatch_index, parameter) else {
            continue;
        };
        // Statically selected boundary capabilities are represented entirely by
        // their type/provider identity. They have no runtime payload to stage or
        // copy, and emitting a zero-byte place copy would manufacture meaningless
        // base relocations (including an empty source symbol for a free call).
        if slot.byte_size == 0 && slot.is_static_boundary_capability {
            continue;
        }
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
        // Keep the authored call identity before alias resolution substitutes
        // an inline callee terminal (for example, a string literal arm) into
        // the argument expression. The substituted value is useful for static
        // materialization, but the call identity is what pairs this parameter
        // with its CallArgument result slot and enforces nested-call sequencing.
        let argument_call = match resolved_argument_expressions
            .expression(resolved_argument.expression)
            .clone()
        {
            ExpressionNode::Call(call) => Some(call),
            _ => match resolved_argument_expressions
                .expression(copied_argument)
                .clone()
            {
                ExpressionNode::Call(call) => Some(call),
                _ => None,
            },
        };
        let argument_source_key = resolved_argument.source_key;
        let argument = resolve_prior_local_initializers_in_table(
            input,
            argument_source_key,
            statement_index,
            &mut resolved_argument_expressions,
            resolved_argument.expression,
        );
        // A state parameter is a first typed landing site. Prior-local folding
        // can reconstruct its argument from anonymous literal syntax, so stamp
        // the same-typed binary value spine from the declared parameter before
        // operator selection. This is parameter-contract metadata, not a write-
        // destination fallback.
        let argument = integer_landing_for_type_reference(input, parameter.type_reference)
            .map(|landing| {
                stamp_anonymous_integer_landing_on_value_spine(
                    &mut resolved_argument_expressions,
                    argument,
                    landing,
                )
            })
            .unwrap_or(argument);
        let expressions = &resolved_argument_expressions;

        // A NO-PAYLOAD case variant used as a value (`AlarmEvent::Trigger`) is a
        // bare `Name` path, not a `StructLiteral`, so it never reached the
        // case-construction branch below -- it fell through to the place-copy
        // path and read an uninitialized/zero slot. That only "worked" when the
        // variant's ordinal happened to be 0 (the ZII tag). Construct it here:
        // write the variant's tag ordinal into the parameter's tag word. (Payload
        // variants arrive as `StructLiteral` and are handled below; a `Name` that
        // resolves to a variant is necessarily the no-payload form.)
        if let Some(tag_value) = enum_variant_value_in_table(&input.layouts, expressions, argument)
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                    RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    tag_value,
                    ENUM_TAG_BYTES,
                ),
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

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
        // NOTE: a bare STRING / byte-slice LITERAL argument (`forward("hello")`) is
        // handled INSIDE emit_runtime_frame_slot_slice_descriptor_write_in_table above
        // (the shared seam), so it no longer needs a strategy here.
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

        // Pair each machine-value-call argument with ITS OWN result slot
        // (by rank, verified against the callee name): the unranked resolver
        // below finds the statement's FIRST slot, so two value-call arguments
        // in one transition both read call 1's result. A rank whose record
        // does not name this argument's callee (a builtin `.unwrap()` between
        // machine calls) falls through to the pre-existing chain unchanged.
        let ranked_place = if let Some(call) = argument_call.as_ref() {
            resolve_runtime_transition_argument_call_result_place_by_rank(
                input,
                source_dispatch_index,
                argument_source_key,
                statement_index,
                transition_call_argument_rank,
            )
            .filter(|_| {
                input
                    .state_calls
                    .transition_argument_call_by_rank(
                        argument_source_key,
                        statement_index,
                        transition_call_argument_rank,
                    )
                    .and_then(|state_call| input.control_flow.state_by_key(state_call.target_key))
                    .is_some_and(|target_state| target_state.name == call.target)
            })
        } else {
            None
        };
        if ranked_place.is_some() {
            transition_call_argument_rank += 1;
        }
        if std::env::var_os("OMEGA_DEBUG_CALL_RESULT").is_some() && argument_call.is_some() {
            eprintln!(
                "call-result READ: src m{} s{} stmt {} dispatch {} ranked {:?}",
                argument_source_key.machine.arena_index(),
                argument_source_key.state.arena_index(),
                statement_index,
                source_dispatch_index,
                ranked_place.as_ref().map(|place| (
                    place.region,
                    place.byte_offset,
                    place.byte_count
                )),
            );
        }
        if let Some(call) = argument_call.as_ref()
            && let Some(place) = ranked_place
                .or_else(|| {
                    resolve_runtime_transition_argument_call_result_place(
                        input,
                        source_dispatch_index,
                        argument_source_key,
                        statement_index,
                    )
                })
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
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_direct(
                    place.region,
                    place.byte_offset,
                    RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    slot.byte_size,
                ),
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
                kind: crate::selection::runtime_dispatch::copy_places_from_pointee(
                    pointee.pointer_byte_offset,
                    pointee.field_byte_offset,
                    RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    slot.byte_size,
                ),
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
                kind: crate::selection::runtime_dispatch::copy_places_from_fixed_indexed(
                    indexed_source.descriptor_offset,
                    indexed_source.element_index,
                    indexed_source.element_byte_size,
                    indexed_source.field_byte_offset,
                    RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    slot.byte_size,
                ),
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
                kind: crate::selection::runtime_dispatch::copy_places_direct(
                    place.region,
                    place.byte_offset,
                    RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    slot.byte_size,
                ),
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        // An INDEXED slice element the place-based copies above did not catch:
        // `s[i]` / `decoded.bytes[i]` where the collection is a slice DESCRIPTOR
        // (the element lives behind its data pointer, so `resolve_runtime_storage
        // _place_in_table` refuses it above) and the index is a const, an elided
        // const local, or a runtime frame value. Resolve it as a VALUE OPERAND --
        // the same machinery a `let b = s[i]` local-copy uses (which is correct)
        // -- and write it to the parameter slot. The motivating shape is consuming
        // a decoded `&[u8]` via `transition i < s.len { true -> handle(s[i]) }`,
        // where `s[i]` is a transition argument; without this it falls through
        // every strategy and the parameter slot keeps its uninitialized bytes.
        if matches!(expressions.expression(argument), ExpressionNode::Indexed(_))
            && let Some(kind) = select_runtime_frame_slot_value_write_in_table_with_source_anchor(
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
            )
        {
            selected_instructions.push(SelectedInstruction {
                kind,
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
                kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                    RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    value,
                    slot.byte_size,
                ),
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        // Float literal state argument (`transition { _ -> state(3.14) }`):
        // write the IEEE-754 bit pattern directly into the parameter slot. The
        // `static_runtime_argument_value` path above handles integers and booleans
        // but skips Float nodes, so f64/f32 literals would fall through to
        // `select_runtime_frame_slot_value_write_in_table_with_source_anchor`
        // which also does not have a direct float-literal path (it reaches the
        // binary-write family which requires a Binary or Call expression). The
        // bit pattern is stable at compile time, so WriteRuntimeStorageInteger
        // with the reinterpreted bits is the correct and simplest lowering.
        if let ExpressionNode::Float(float_literal) = expressions.expression(argument) {
            let bits = float_literal.landed_f64().to_bits();
            let value = if slot.byte_size == 4 {
                // f32 slot: narrow the f64 bit pattern to f32 bits
                float_literal.f32_bits() as i64
            } else {
                // f64 slot (8 bytes): use the full f64 bit pattern
                bits as i64
            };
            if matches!(slot.byte_size, 4 | 8) {
                selected_instructions.push(SelectedInstruction {
                    kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                        RuntimeStorageRegion::RuntimeFrame,
                        slot.byte_offset,
                        value,
                        slot.byte_size,
                    ),
                    source_key,
                    source_statement: statement_index,
                });
            }
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
                        kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                            RuntimeStorageRegion::RuntimeFrame,
                            slot.byte_offset,
                            tag_value,
                            ENUM_TAG_BYTES,
                        ),
                        source_key,
                        source_statement: statement_index,
                    });
                }

                // Write each payload field. Field offsets in VariantLayout are
                // ABSOLUTE within the enum value (0 = start of the tag), so the
                // frame address of a field is slot.byte_offset + field.offset.
                let variant_fields: Vec<(psi_checked_trees::name::Identifier, usize, usize)> = {
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
                    let Some((_, field_offset, field_size)) = variant_fields
                        .iter()
                        .find(|(name, _, _)| *name == field.name)
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
                                kind:
                                    crate::selection::runtime_dispatch::write_place_integer_direct(
                                        RuntimeStorageRegion::RuntimeFrame,
                                        frame_offset,
                                        int_val,
                                        *field_size,
                                    ),
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
                    if let Some(kind) =
                        select_runtime_frame_slot_value_write_in_table_with_source_anchor(
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
                        )
                    {
                        selected_instructions.push(SelectedInstruction {
                            kind,
                            source_key,
                            source_statement: statement_index,
                        });
                    }
                }
                continue;
            }

            // A PLAIN RECORD literal argument (`Exit { destination: d, weight: w }`,
            // no case): same field-wise delivery, no tag. This arm was MISSING --
            // the record shape fell through to the scalar writer, which plans
            // nothing for an aggregate, so the callee's param slot stayed ZII
            // (pending/calls/struct_literal_transition_arg_native_divergence).
            let record_fields: Vec<(psi_checked_trees::name::Identifier, usize, usize)> = {
                let type_name = &struct_literal.type_name;
                input
                    .layouts
                    .data_layouts
                    .iter()
                    .find(|(_, dl)| dl.name == *type_name)
                    .and_then(|(_, dl)| {
                        if let DataShape::Record { fields } = &dl.shape {
                            Some(
                                input
                                    .layouts
                                    .fields
                                    .span_or_empty(*fields)
                                    .iter()
                                    .map(|f| (f.name.clone(), f.offset, f.layout.size))
                                    .collect(),
                            )
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default()
            };
            if !record_fields.is_empty() {
                for offset in 0..struct_literal.fields.count() {
                    let field = expressions
                        .struct_field_at_offset(struct_literal.fields, offset)
                        .clone();
                    let Some((_, field_offset, field_size)) = record_fields
                        .iter()
                        .find(|(name, _, _)| *name == field.name)
                    else {
                        continue;
                    };
                    let frame_offset = slot.byte_offset + field_offset;
                    if let Some(int_val) =
                        static_runtime_argument_value(expressions.expression(field.value))
                    {
                        if matches!(field_size, 1 | 2 | 4 | 8) {
                            selected_instructions.push(SelectedInstruction {
                                kind:
                                    crate::selection::runtime_dispatch::write_place_integer_direct(
                                        RuntimeStorageRegion::RuntimeFrame,
                                        frame_offset,
                                        int_val,
                                        *field_size,
                                    ),
                                source_key,
                                source_statement: statement_index,
                            });
                        }
                        continue;
                    }
                    let mut field_slot = slot.clone();
                    field_slot.byte_offset = frame_offset;
                    field_slot.byte_size = *field_size;
                    if let Some(kind) =
                        select_runtime_frame_slot_value_write_in_table_with_source_anchor(
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
                        )
                    {
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
            kind: crate::selection::runtime_dispatch::copy_places_direct(
                RuntimeStorageRegion::RuntimeFrame,
                scratch_offset,
                RuntimeStorageRegion::RuntimeFrame,
                target_offset,
                byte_count,
            ),
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
    call: &psi_checked_trees::expression::TableCallExpression,
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
/// EVERY frame range the argument expression READS: the whole argument when it
/// is a place, and otherwise each place/descriptor leaf under binary/cast
/// operands. The overlap detector must see ALL reads: a recursive accumulator
/// arm (`self.sum(n - 1, acc + n)`) reads `n` inside the SECOND argument while
/// the FIRST argument's write targets `n`'s slot -- an unstaged in-place write
/// sequence decrements `n` before `acc + n` reads it (the parallel-assignment
/// hazard; sum(5,0) natively computed 10 instead of 15).
fn argument_source_frame_ranges(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    source_dispatch_index: u32,
    raw_argument: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
) -> Vec<(usize, usize)> {
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
    let mut sources = Vec::new();
    collect_argument_source_frame_ranges(
        input,
        source_dispatch_index,
        argument_source_key,
        &scratch,
        argument,
        &mut sources,
    );
    sources
}

/// Leaf collector for [`argument_source_frame_ranges`]: a resolvable FRAME
/// place contributes its range; a constant-index slice element (`s[0]`)
/// contributes its DESCRIPTOR slot's range (its read goes through the
/// descriptor's data pointer, so a transition that also RETARGETS that
/// descriptor -- `self.accumulate(items[1..], items[0].value)` -- must stage:
/// an unstaged in-place descriptor update would shrink the window BEFORE the
/// element read, fetching the next window's head). Binary/cast/mutable nodes
/// recurse into their operands.
fn collect_argument_source_frame_ranges(
    input: &InstructionSelectionInput<'_>,
    source_dispatch_index: u32,
    argument_source_key: StateKey,
    scratch: &ExpressionTable,
    argument: ExpressionHandle,
    sources: &mut Vec<(usize, usize)>,
) {
    if let Some(place) = resolve_runtime_storage_place_in_table(
        input,
        source_dispatch_index,
        argument_source_key,
        scratch,
        argument,
    ) {
        if place.region == RuntimeStorageRegion::RuntimeFrame {
            sources.push((place.byte_offset, place.byte_offset + place.byte_count));
        }
        return;
    }
    if let Some(indexed) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        source_dispatch_index,
        argument_source_key,
        scratch,
        argument,
    ) {
        sources.push((
            indexed.descriptor_offset,
            indexed.descriptor_offset + input.runtime_abi.pointer_size,
        ));
        return;
    }
    match scratch.expression(argument) {
        ExpressionNode::Binary(binary) => {
            collect_argument_source_frame_ranges(
                input,
                source_dispatch_index,
                argument_source_key,
                scratch,
                binary.left,
                sources,
            );
            collect_argument_source_frame_ranges(
                input,
                source_dispatch_index,
                argument_source_key,
                scratch,
                binary.right,
                sources,
            );
        }
        ExpressionNode::Cast(cast) => collect_argument_source_frame_ranges(
            input,
            source_dispatch_index,
            argument_source_key,
            scratch,
            cast.value,
            sources,
        ),
        ExpressionNode::Mutable(inner) => collect_argument_source_frame_ranges(
            input,
            source_dispatch_index,
            argument_source_key,
            scratch,
            *inner,
            sources,
        ),
        _ => {}
    }
}

fn ranges_overlap(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

pub(super) fn static_runtime_argument_value(expression: &ExpressionNode) -> Option<i64> {
    match expression {
        ExpressionNode::Integer(value) => value.value_i64(),
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
        kind: crate::selection::runtime_dispatch::write_place_address_direct(
            source_place.region,
            source_place.byte_offset,
            slot.byte_offset,
        ),
        source_key,
        source_statement: statement_index,
    });
    let descriptor = input.runtime_abi.slice_descriptor();
    selected_instructions.push(SelectedInstruction {
        kind: crate::selection::runtime_dispatch::write_place_integer_direct(
            RuntimeStorageRegion::RuntimeFrame,
            slot.byte_offset + descriptor.len_offset(),
            length as i64,
            descriptor.len_size(),
        ),
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
        kind: crate::selection::runtime_dispatch::copy_places_direct(
            RuntimeStorageRegion::RuntimeFrame,
            source_place.byte_offset,
            RuntimeStorageRegion::RuntimeFrame,
            scratch_offset,
            source_place.byte_count,
        ),
        source_key,
        source_statement: statement_index,
    });
    selected_instructions.push(SelectedInstruction {
        kind: crate::selection::runtime_dispatch::write_place_address_direct(
            RuntimeStorageRegion::RuntimeFrame,
            scratch_offset,
            slot.byte_offset,
        ),
        source_key,
        source_statement: statement_index,
    });
    let descriptor = input.runtime_abi.slice_descriptor();
    selected_instructions.push(SelectedInstruction {
        kind: crate::selection::runtime_dispatch::write_place_integer_direct(
            RuntimeStorageRegion::RuntimeFrame,
            slot.byte_offset + descriptor.len_offset(),
            length as i64,
            descriptor.len_size(),
        ),
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
        ExpressionNode::ArrayLiteral(_) | ExpressionNode::StructLiteral(_) => true,
        // A judged RECAST initializer (`let d = &self.map_buf[k] as &Descriptor`)
        // owns a slot the lowering fills at the declaration point (an element
        // ADDRESS for a wide referee, the content snapshot for a narrow one).
        // Folding a member arg like `d.physical_start` back into a
        // member-on-cast expression makes it unresolvable and the argument is
        // silently dropped; keep the local a place so the pointee/flat slot
        // path resolves it.
        ExpressionNode::Cast(cast) if cast.form.is_recast() => true,
        // A result-producing call ANYWHERE in the initializer (the whole call, or
        // one nested inside a binary/cast like `let r = base + f(6) * 3`) owns a
        // computed frame slot. Folding such a local back into its initializer and
        // re-materializing it in the TARGET state would re-evaluate the call --
        // whose result scratch lives in the SOURCE state's frame and is not
        // reproducible there -- so the argument silently fails to land. Keep the
        // local as a place so its slot is COPIED instead. (`as_slice`/`as_mut_slice`
        // views must still fold so the slice materialization sees the receiver.)
        _ => expression_contains_result_call(&input.program.expression_table, initial_value),
    }
}

/// Whether an expression tree contains a result-producing call (any call except
/// the `as_slice`/`as_mut_slice` view builders). Used to decide a let-binding
/// must not be inline-folded during argument resolution.
fn expression_contains_result_call(table: &ExpressionTable, expression: ExpressionHandle) -> bool {
    match table.expression(expression) {
        ExpressionNode::Call(call) => !matches!(
            call.target.as_str(),
            "as_slice" | "as_mut_slice" | "as_view" | "bytes"
        ),
        ExpressionNode::Binary(binary) => {
            expression_contains_result_call(table, binary.left)
                || expression_contains_result_call(table, binary.right)
        }
        ExpressionNode::Cast(cast) => expression_contains_result_call(table, cast.value),
        ExpressionNode::Unary(unary) => expression_contains_result_call(table, unary.operand),
        ExpressionNode::Mutable(inner) => expression_contains_result_call(table, *inner),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_result_call(table, indexed.collection)
                || expression_contains_result_call(table, indexed.index)
        }
        ExpressionNode::Member(member) => expression_contains_result_call(table, member.receiver),
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
            // A `let`-bound local whose initializer is a plain place read
            // (e.g. `let slot: i32 = self.s.count`) gets its OWN `LocalStorage`
            // frame slot that is written with the field's value at the declaration
            // point. Folding the Name back to its initializer expression bypasses
            // that slot and re-reads the source field directly -- if the field was
            // mutated between the declaration and the transition arm (e.g.
            // `self.s.count = self.s.count + 1`), the fold reads the
            // post-mutation value instead of the captured pre-mutation value.
            //
            // Block the fold (keep the local as a place so its captured slot is
            // COPIED) when the local has a `LocalStorage` slot AND either:
            // 1. The initializer is a PURE PLACE read (`let slot = self.s.count`):
            //    the slot holds the live field value captured at declaration, and
            //    re-folding would re-read the field -- wrong if it was mutated since.
            // 2. The initializer READS A FIELD that is REASSIGNED after this local's
            //    declaration (`let new_sp = self.vm.sp + 1; self.vm.sp = new_sp; ...
            //    try_push1(new_sp)`): re-folding re-evaluates `self.vm.sp + 1` AFTER
            //    `self.vm.sp` was overwritten, so a deeper substate's guard reads a
            //    doubly-incremented value and branches wrong (the stack_vm push
            //    miscompile, task #17). The slot captured the pre-mutation value.
            // A binary initializer whose fields are NOT reassigned after the
            // declaration still FOLDS: its frame-slot write may use a stale static
            // value for field operands, so re-evaluating at transition time with the
            // current (unchanged) field values is the reliable path.
            let block_fold =
                local_initializer_is_pure_place(initial_value, &input.program.expression_table)
                    || initializer_reads_field_reassigned_after_decl(
                        input,
                        source_key,
                        statement_index,
                        expressions,
                        expression,
                        initial_value,
                    );
            if block_fold
                && let Some((local_symbol, local_name)) =
                    local_root_identity(expressions, expression)
            {
                let has_local_storage_slot =
                    input.runtime_storage.frame_slots.iter().any(|(_, slot)| {
                        state_key_matches_statement_source(slot.source_key, source_key)
                            && matches!(
                                slot.kind,
                                omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                            )
                            && ((slot.symbol.is_valid()
                                && local_symbol.is_valid()
                                && slot.symbol == local_symbol)
                                || slot.name == local_name)
                    });
                if has_local_storage_slot {
                    return expression;
                }
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
                    psi_checked_trees::expression::TableIndexedExpression { collection, index },
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
                    psi_checked_trees::expression::TableRangeExpression {
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
                    psi_checked_trees::expression::TableCallExpression {
                        receiver,
                        target_symbol: call.target_symbol,
                        target: call.target,
                        machine_arguments: call.machine_arguments,
                        quotient_operation: call.quotient_operation,
                        arguments,
                        evidence_arguments: call.evidence_arguments,
                        operational_acknowledgement: call.operational_acknowledgement,
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
                    psi_checked_trees::expression::TableMemberExpression {
                        receiver,
                        member_symbol: member.member_symbol,
                        member: member.member,
                        case_variant: member.case_variant,
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
        ExpressionNode::Binary(binary) => {
            // Same as the cast case below: a binary operand may be a prior let-local
            // (`let s = x + 100`). Fold both sides so an inner local resolves rather
            // than dangling into the target frame (mirrors the leaf path's Binary arm;
            // the inner Name arm keeps its capture/block-fold protection, so a
            // reassigned-field operand still copies its slot).
            let left = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                binary.left,
            );
            let right = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                binary.right,
            );
            if left == binary.left && right == binary.right {
                expression
            } else {
                expressions.insert(ExpressionNode::Binary(
                    psi_checked_trees::expression::TableBinaryExpression {
                        left,
                        operator: binary.operator,
                        right,
                    },
                ))
            }
        }
        ExpressionNode::Unary(unary) => {
            // Same root as the Cast/Binary arms: a unary operand may be a prior
            // let-local (`let nb = !b`). Fold the operand so the inner local resolves
            // rather than dangling into the target frame.
            let operand = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                unary.operand,
            );
            if operand == unary.operand {
                expression
            } else {
                expressions.insert(ExpressionNode::Unary(
                    psi_checked_trees::expression::TableUnaryExpression {
                        operator: unary.operator,
                        operand,
                    },
                ))
            }
        }
        ExpressionNode::Cast(cast) => {
            // A cast's inner value may be a prior let-local (`let bw = b8 as i32`).
            // When the whole `let` is folded into a forwarded argument, the inner
            // local must be resolved too -- otherwise the cast is re-materialized in
            // the TARGET state where the source local has no slot and reads 0. Recurse
            // into the cast value (mirrors the leaf path's `resolve_leaf_caller_local_
            // initializer_names` Cast arm); the inner Name arm still applies its
            // capture/block-fold protection.
            let value = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                cast.value,
            );
            if value == cast.value {
                expression
            } else {
                expressions.insert(ExpressionNode::Cast(
                    psi_checked_trees::expression::TableCastExpression {
                        value,
                        target_type: cast.target_type,
                        target_label: cast.target_label,
                        domain: cast.domain,
                        semantic_domain: cast.semantic_domain,
                        semantic_domain_arguments: cast.semantic_domain_arguments,
                        semantic_domain_symbol: cast.semantic_domain_symbol,
                        semantic_domain_id: cast.semantic_domain_id,
                        form: cast.form,
                    },
                ))
            }
        }
        _ => expression,
    }
}

fn local_root_identity(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<(
    psi_symbols::SymbolHandle,
    psi_checked_trees::name::Identifier,
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

/// The symbol chain of a place expression (`self.vm.sp` -> [self, vm, sp]), used
/// to compare a field READ in a local's initializer against an assignment TARGET.
fn place_symbol_signature(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<Vec<psi_symbols::SymbolHandle>> {
    match expressions.expression(expression) {
        ExpressionNode::Name(path) => Some(vec![path.head_symbol]),
        ExpressionNode::Member(member) => {
            let mut signature = place_symbol_signature(expressions, member.receiver)?;
            signature.push(member.member_symbol);
            Some(signature)
        }
        ExpressionNode::Indexed(indexed) => place_symbol_signature(expressions, indexed.collection),
        ExpressionNode::Mutable(inner) => place_symbol_signature(expressions, *inner),
        _ => None,
    }
}

/// Collect the place-read signatures within an initializer expression (e.g. the
/// `self.vm.sp` operand of `self.vm.sp + 1`).
fn collect_read_place_signatures(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    out: &mut Vec<Vec<psi_symbols::SymbolHandle>>,
) {
    if let Some(signature) = place_symbol_signature(expressions, expression) {
        out.push(signature);
        return;
    }
    match expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            collect_read_place_signatures(expressions, binary.left, out);
            collect_read_place_signatures(expressions, binary.right, out);
        }
        ExpressionNode::Cast(cast) => collect_read_place_signatures(expressions, cast.value, out),
        ExpressionNode::Unary(unary) => {
            collect_read_place_signatures(expressions, unary.operand, out)
        }
        ExpressionNode::Mutable(inner) => collect_read_place_signatures(expressions, *inner, out),
        _ => {}
    }
}

/// True when the local's initializer reads a field that is REASSIGNED in a
/// statement AFTER the local's declaration and before the current (transition)
/// statement. Then re-folding the initializer at the transition reads the
/// post-mutation field, while the local's captured slot holds the correct
/// pre-mutation value -- so the caller must keep the place and copy the slot.
fn initializer_reads_field_reassigned_after_decl(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    initial_value: ExpressionHandle,
) -> bool {
    let Some((local_symbol, local_name)) = local_root_identity(expressions, expression) else {
        return false;
    };
    let Some(machine) = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)
    else {
        return false;
    };
    let Some(state) = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)
    else {
        return false;
    };
    let statements = input
        .program
        .statement_table
        .statements(state.statement_nodes);
    let upper = statement_index.min(statements.len());
    let Some(decl_index) = statements[..upper].iter().position(|statement| {
        matches!(statement,
            StatementNode::LocalData(local_data)
                if (local_symbol.is_valid() && local_data.symbol == local_symbol)
                    || local_data.name == local_name)
    }) else {
        return false;
    };

    let mut read_signatures: Vec<Vec<psi_symbols::SymbolHandle>> = Vec::new();
    collect_read_place_signatures(
        &input.program.expression_table,
        initial_value,
        &mut read_signatures,
    );
    if read_signatures.is_empty() {
        return false;
    }

    statements[decl_index + 1..upper].iter().any(|statement| {
        let StatementNode::Assignment(assignment) = statement else {
            return false;
        };
        place_symbol_signature(&input.program.expression_table, assignment.target)
            .is_some_and(|target| read_signatures.iter().any(|read| *read == target))
    })
}

/// True when `initial_value` is a "pure place" expression -- a Name, Member
/// chain, or indexed read, with no arithmetic or calls. Such an initializer is
/// always written to the `LocalStorage` frame slot with the live field value at
/// declaration time, so the slot reliably holds the captured value. Binary or
/// call initializers may be written with stale static values for field operands
/// (a known limitation of the static-value tracking layer), so for those the
/// caller should NOT block the fold: it re-evaluates at transition time instead.
fn local_initializer_is_pure_place(
    initial_value: ExpressionHandle,
    expressions: &psi_checked_trees::expression::ExpressionTable,
) -> bool {
    match expressions.expression(initial_value) {
        ExpressionNode::Name(_) => true,
        ExpressionNode::Member(member) => {
            local_initializer_is_pure_place(member.receiver, expressions)
        }
        ExpressionNode::Indexed(indexed) => {
            local_initializer_is_pure_place(indexed.collection, expressions)
                && local_initializer_is_pure_place(indexed.index, expressions)
        }
        ExpressionNode::Mutable(inner) => local_initializer_is_pure_place(*inner, expressions),
        // Integers/booleans are compile-time constants; perfectly safe to
        // capture (they never become stale). Float literals likewise.
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_) => true,
        // Binary expressions, calls, casts, arrays, and struct literals may
        // involve stale static values for machine-field operands.
        _ => false,
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
