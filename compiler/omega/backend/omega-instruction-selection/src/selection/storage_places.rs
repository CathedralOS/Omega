mod expressions;
mod machine_owned;
mod model;
mod nested_fields;
mod static_values;

pub(super) use expressions::indexed_expression_path;
pub(super) use machine_owned::{
    MachineOwnedCollectionTarget, resolve_machine_owned_bit_field_in_table,
    resolve_machine_owned_collection_in_table, resolve_machine_owned_place,
    resolve_machine_owned_place_in_table, resolve_machine_owned_self_case_tag_place_in_table,
    resolve_machine_owned_stored_integer_in_table,
};
pub(super) use model::{
    RuntimeBitFieldPlace, RuntimeFrameBaseIndexedTarget, RuntimeFrameFixedIndexedTarget,
    RuntimeFrameIndexedTarget, RuntimeStoragePlace, RuntimeStoredIntegerProjection,
    RuntimeStoredIntegerSource,
};
use omega_abstract_operations::{Place, PlaceStep, RuntimeStorageRegion};
pub(super) use static_values::{
    clamp_runtime_case_comparison_operands, clamp_runtime_case_comparison_operands_in_table,
    enum_variant_value, enum_variant_value_in_table, static_integer_value,
    static_integer_value_in_table,
};

use crate::InstructionSelectionInput;
use expressions::{StorageNamePath, StoragePathSuffix, normalized_storage_name_path_in_table};
use nested_fields::{
    NestedFieldLayoutCursor, resolve_nested_field_layout_step,
    resolve_nested_field_layout_with_pairs, resolve_nested_stored_integer_layout_step,
};
use omega_control_flow::StateKey;
use omega_layout::{FieldLayout, TypeLayout, TypeLayoutDescriptor};
use omega_state_calls::StateCallRole;
use psi_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use psi_checked_trees::name::Identifier;
use psi_checked_trees::types::PrimitiveType;
use psi_symbols::{BuiltinType, SymbolHandle};

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}

fn runtime_slice_descriptor_member_place(
    input: &InstructionSelectionInput<'_>,
    root_offset: usize,
    byte_size: usize,
    member_name: Option<&str>,
    member_count: usize,
) -> Option<RuntimeStoragePlace> {
    let descriptor = input.runtime_abi.slice_descriptor();
    if byte_size != descriptor.total_size() || member_count != 1 {
        return None;
    }

    match member_name {
        // The length VALUE is a 32-bit count (the language types `.len` as
        // `i32`-assignable without a cast); the descriptor's 8-byte len slot is
        // storage, so read the low 4-byte word. This matches the carrier `.len`
        // convention and lets `.len` narrow into an `i32` target (an 8-byte read
        // does not lower into a 4-byte field write).
        Some("len") => Some(RuntimeStoragePlace {
            region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: root_offset.checked_add(descriptor.len_offset())?,
            byte_count: 4,
        }),
        _ => None,
    }
}

/// Whether a type descriptor is a FAT slice descriptor -- a `&[T]`/`&string`
/// view or a bare slice -- the only shapes that carry a runtime `len` slot. A
/// fixed array's `.len` is a layout constant (folded elsewhere), and an
/// arbitrary 16-byte aggregate must NOT be misread as a descriptor, so this is
/// type-driven rather than size-driven. Mirrors the fat-pointer classification
/// in omega-layout's reference layout.
pub(super) fn descriptor_is_fat_slice(descriptor: &TypeLayoutDescriptor) -> bool {
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. } => descriptor_is_fat_slice(base_type),
        TypeLayoutDescriptor::Reference { referee, .. } => {
            // Unwrap any `Constrained` wrappers on the referee: a `&[u8] in Utf8`
            // is `Reference { Constrained { Slice } }`, and the domain constraint
            // does not change the fat-descriptor shape -- it must classify as a
            // fat slice exactly like `&[u8]`. (Mirrors omega-layout's reference
            // sizing, which likewise unwraps Constrained before the unsized check.)
            let mut referee = referee.as_ref();
            while let TypeLayoutDescriptor::Constrained { base_type, .. } = referee {
                referee = base_type.as_ref();
            }
            match referee {
                TypeLayoutDescriptor::Slice { .. } => true,
                TypeLayoutDescriptor::Named { name, .. } => name.as_str() == "string",
                _ => false,
            }
        }
        TypeLayoutDescriptor::Slice { .. } => true,
        TypeLayoutDescriptor::Named { name, .. } => name.as_str() == "string",
        _ => false,
    }
}

/// Resolve `<struct>.<descriptor-field>.len` (a `.len` read on a slice/`&[u8]`
/// descriptor held as a FIELD, not the root slot) to the descriptor's runtime
/// `len` slot. Walks the suffix up to -- but not including -- the trailing
/// `len`; returns `None` unless that lands on a fat slice descriptor of exactly
/// descriptor size. The root-slot-IS-descriptor case (`s.len` where `s` is a
/// `&[u8]` local) is handled by `runtime_slice_descriptor_member_place`.
fn runtime_nested_slice_descriptor_len_place(
    input: &InstructionSelectionInput<'_>,
    root_field: &FieldLayout,
    suffix: StoragePathSuffix<'_, '_>,
) -> Option<RuntimeStoragePlace> {
    let segments: Vec<_> = suffix.iter().collect();
    let (last_name, _, _, _) = segments.last()?;
    // Must END in `len` with a non-empty descriptor-field prefix.
    if last_name.as_str() != "len" || segments.len() < 2 {
        return None;
    }

    let mut cursor = NestedFieldLayoutCursor::from_root(root_field);
    for &(field_name, field_symbol, field_index, case_variant) in &segments[..segments.len() - 1] {
        cursor = resolve_nested_field_layout_step(
            &input.layouts,
            cursor,
            field_name,
            field_symbol,
            field_index,
            case_variant,
        )?;
    }

    if !descriptor_is_fat_slice(cursor.type_descriptor()) {
        return None;
    }
    let descriptor = input.runtime_abi.slice_descriptor();
    if cursor.layout().size != descriptor.total_size() {
        return None;
    }
    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset: cursor.byte_offset().checked_add(descriptor.len_offset())?,
        // The length value is a 32-bit count -- read the low 4-byte word (see the
        // root-slot `.len` resolver above).
        byte_count: 4,
    })
}

pub(super) fn resolve_runtime_storage_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    _source_machine: &str,
    _source_state: &str,
    expression: &Expression,
) -> Option<RuntimeStoragePlace> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

pub(super) fn resolve_runtime_assignment_value_call_result_place_by_ordinal(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::AssignmentValue,
        Some(call_ordinal),
    )
}

pub(super) fn resolve_runtime_call_argument_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::CallArgument,
        None,
    )
}

pub(super) fn resolve_runtime_call_argument_call_result_place_by_ordinal(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::CallArgument,
        Some(call_ordinal),
    )
}

pub(super) fn resolve_runtime_transition_guard_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::TransitionGuard,
        None,
    )
    .or_else(|| {
        input
            .runtime_storage
            .transition_guard_result_slot(dispatch_index, source_key, statement_index)
            .map(|slot| RuntimeStoragePlace {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: slot.byte_offset,
                byte_count: slot.byte_size,
            })
    })
}

pub(super) fn resolve_runtime_transition_argument_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::TransitionArgument,
        None,
    )
}

/// Per-argument variant: the Nth Call-typed transition argument reads the Nth
/// transition-argument call's result slot (by ascending call_ordinal). The
/// unranked resolver above finds the statement's FIRST slot, so with two
/// value-call arguments in one transition both parameters read call 1's
/// result.
pub(super) fn resolve_runtime_transition_argument_call_result_place_by_rank(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    call_rank: usize,
) -> Option<RuntimeStoragePlace> {
    let state_call = input.state_calls.transition_argument_call_by_rank(
        source_key,
        statement_index,
        call_rank,
    )?;
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::TransitionArgument,
        Some(state_call.call_ordinal),
    )
}

fn resolve_runtime_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: Option<usize>,
) -> Option<RuntimeStoragePlace> {
    let slot = if let Some(call_ordinal) = call_ordinal {
        input
            .runtime_storage
            .call_result_slot_by_ordinal(
                dispatch_index,
                source_key,
                statement_index,
                role,
                call_ordinal,
            )
            .or_else(|| {
                // A statement with multiple dispatched calls is split into one
                // continuation segment per call. Earlier results live in an
                // earlier segment's slot but the final expression reads them in
                // the tail segment. All those segments share the caller's call
                // context and physical frame; recover the exact ordinal only from
                // that same context, never from another clone of the source state.
                let context = input
                    .runtime_flow
                    .states
                    .iter()
                    .find(|(handle, _)| handle.arena_index() == dispatch_index)
                    .map(|(_, state)| state.context)?;
                input
                    .runtime_storage
                    .frame_slots
                    .iter()
                    .find_map(|(_, slot)| {
                        let slot_context = input
                            .runtime_flow
                            .states
                            .iter()
                            .find(|(handle, _)| handle.arena_index() == slot.dispatch_index)
                            .map(|(_, state)| state.context);
                        (slot_context == Some(context)
                            && state_key_matches_statement_source(slot.source_key, source_key)
                            && slot.statement_index == statement_index
                            && matches!(
                                slot.kind,
                                omega_runtime_storage::RuntimeFrameSlotKind::StateCallResult {
                                    role: slot_role,
                                    call_ordinal: slot_ordinal,
                                    ..
                                } if slot_role == role && slot_ordinal == call_ordinal
                            ))
                        .then_some(slot)
                    })
            })
    } else {
        input
            .runtime_storage
            .call_result_slot(dispatch_index, source_key, statement_index, role)
    }?;
    let target_key = match slot.kind {
        omega_runtime_storage::RuntimeFrameSlotKind::StateCallResult { target_key, .. } => {
            target_key
        }
        _ => return None,
    };
    let state_call = if let Some(call_ordinal) = call_ordinal {
        input
            .state_calls
            .calls_for_statement(source_key, statement_index)
            .find(|state_call| state_call.role == role && state_call.call_ordinal == call_ordinal)
    } else {
        input
            .state_calls
            .call_for_role(source_key, statement_index, role)
    }?;
    if state_call.target_key != target_key {
        return None;
    }
    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset: slot.byte_offset,
        byte_count: slot.byte_size,
    })
}

pub(super) fn resolve_runtime_storage_place_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoragePlace> {
    if resolve_runtime_bit_field_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )
    .is_some()
    {
        return None;
    }
    // A `[u8; N]` carrier's `.len` is the length word at the carrier's OWN offset
    // (its content lives at `+pointer_size`), unlike a fat-slice descriptor whose
    // `len` sits at `+pointer_size`. Resolve `<carrier>.len` by recursively
    // resolving the carrier receiver, then reading the length word -- so every
    // value-position consumer (host-call argument, mutation-write value) reads it
    // uniformly. The length is a 32-bit count (`N < 2^32`), so the 4-byte read is
    // exact and matches `i32` targets/exit codes (an 8-byte read does not lower
    // into a 4-byte field write). The slice-descriptor `.len` paths below only
    // cover fat descriptors.
    let carrier_length_receiver = match expressions.expression(expression) {
        ExpressionNode::Member(member) if member.member.as_str() == "len" => Some(member.receiver),
        _ => None,
    };
    if let Some(receiver) = carrier_length_receiver
        && resolve_runtime_storage_place_is_bounded_byte_buffer_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            receiver,
        )
        && let Some(place) = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            receiver,
        )
    {
        return Some(RuntimeStoragePlace {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_count: 4,
        });
    }

    if let Some(place) = resolve_runtime_fixed_indexed_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(place);
    }

    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.is_empty() {
        return None;
    }
    let suffix = path.suffix(1);
    if let Some(slot) =
        find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
            slot_matches_table_path(slot, &path)
        })
        .or_else(|| {
            latest_dispatch_frame_slot(input, dispatch_index, |slot| {
                slot_matches_table_path(slot, &path)
            })
        })
        .or_else(|| resymbolized_local_slot_for_path(input, dispatch_index, source_key, &path))
    {
        // A root segment that carries an element index (`items[0].value` where
        // `items` is the matched frame slot) only resolves to a static place when
        // the slot stores the array INLINE — and the inline cases were already
        // handled by the index-aware resolver above. When the slot holds a slice
        // DESCRIPTOR the element lives behind the descriptor's data pointer, so
        // no static frame place exists; the fall-through below used to DROP the
        // index and alias the descriptor slot's own bytes as the element (a
        // threaded `items[0].value` argument received the data pointer's low
        // bytes). Refuse so callers fall back to descriptor-aware indexed
        // strategies. Index 0 on an inline array keeps the legacy direct place
        // (offset arithmetic is identical with the index dropped).
        if let Some(root_index) = path.member_index(0)
            && (root_index != 0 || !runtime_frame_slot_is_inline_fixed_array_storage(input, slot))
        {
            return None;
        }

        if let Some(place) = runtime_slice_descriptor_member_place(
            input,
            slot.byte_offset,
            slot.byte_size,
            suffix.iter().next().map(|(name, _, _, _)| name.as_str()),
            suffix.iter().count(),
        ) {
            return Some(place);
        }

        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: slot.byte_offset,
            type_symbol: slot.type_symbol,
            type_name: slot.type_name.clone(),
            type_descriptor: slot.type_descriptor.clone(),
            layout: TypeLayout {
                size: slot.byte_size,
                alignment: slot.alignment,
            },
        };
        // `<struct>.<descriptor-field>.len` -- a `.len` read on a slice/`&[u8]`
        // descriptor that lives as a FIELD (so the descriptor is not the root
        // slot, which the count==1 path above handles). Walk the suffix up to
        // (but not including) the trailing `len`; if it lands on a fat slice
        // descriptor, the length is its runtime len slot. Without this the `.len`
        // step has no data layout to resolve against, the whole place silently
        // fails to resolve, and the read yields uninitialized garbage.
        if let Some(place) = runtime_nested_slice_descriptor_len_place(input, &root_field, suffix) {
            return Some(place);
        }

        let (byte_offset, layout) =
            resolve_nested_field_layout_with_pairs(&input.layouts, &root_field, suffix.iter())?;

        return Some(RuntimeStoragePlace {
            region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset,
            byte_count: layout.size,
        });
    }

    resolve_machine_owned_place_in_table(
        &input.layouts,
        input,
        dispatch_index,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        expression,
    )
    .map(|(byte_offset, byte_count)| RuntimeStoragePlace {
        region: RuntimeStorageRegion::Machine,
        byte_offset,
        byte_count,
    })
}

pub(super) fn resolve_runtime_bit_field_place_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeBitFieldPlace> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.is_empty() {
        return None;
    }
    let suffix = path.suffix(1);
    if let Some(slot) =
        find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
            slot_matches_table_path(slot, &path)
        })
        .or_else(|| {
            latest_dispatch_frame_slot(input, dispatch_index, |slot| {
                slot_matches_table_path(slot, &path)
            })
        })
    {
        if path.member_index(0).is_some() {
            return None;
        }
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: slot.byte_offset,
            type_symbol: slot.type_symbol,
            type_name: slot.type_name.clone(),
            type_descriptor: slot.type_descriptor.clone(),
            layout: TypeLayout {
                size: slot.byte_size,
                alignment: slot.alignment,
            },
        };
        let mut cursor = NestedFieldLayoutCursor::from_root(&root_field);
        for (field_name, field_symbol, field_index, case_variant) in suffix.iter() {
            cursor = resolve_nested_field_layout_step(
                &input.layouts,
                cursor,
                field_name,
                field_symbol,
                field_index,
                case_variant,
            )?;
        }
        let (containing_byte_offset, bit_field) = cursor.bit_field()?;
        return Some(RuntimeBitFieldPlace {
            region: RuntimeStorageRegion::RuntimeFrame,
            base_byte_offset: containing_byte_offset,
            value_byte_count: cursor.layout().size,
            fragments: bit_field.fragments.clone(),
        });
    }

    resolve_machine_owned_bit_field_in_table(
        &input.layouts,
        input,
        dispatch_index,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        expression,
    )
}

/// Resolve a plan-laid `IntegerAt` leaf without exposing it to ordinary place
/// consumers. The result carries both physical width/interpretation and the
/// semantic carrier width; callers must perform the stated extension.
pub(super) fn resolve_runtime_stored_integer_projection_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoredIntegerProjection> {
    if let Some(projection) = resolve_runtime_pointee_indexed_stored_integer_projection_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(projection);
    }
    if let Some(projection) = resolve_runtime_frame_indexed_stored_integer_projection_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(projection);
    }
    if let Some(projection) = resolve_runtime_frame_base_indexed_stored_integer_projection_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(projection);
    }
    if let Some(projection) = resolve_runtime_machine_indexed_stored_integer_projection_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(projection);
    }
    if let Some(projection) = resolve_runtime_pointee_stored_integer_projection_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(projection);
    }

    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.is_empty() || path.member_index(0).is_some() {
        return None;
    }
    let suffix = path.suffix(1);
    if let Some(slot) =
        find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
            slot_matches_table_path(slot, &path)
        })
        .or_else(|| {
            latest_dispatch_frame_slot(input, dispatch_index, |slot| {
                slot_matches_table_path(slot, &path)
            })
        })
    {
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: slot.byte_offset,
            type_symbol: slot.type_symbol,
            type_name: slot.type_name.clone(),
            type_descriptor: slot.type_descriptor.clone(),
            layout: TypeLayout {
                size: slot.byte_size,
                alignment: slot.alignment,
            },
        };
        let mut cursor = NestedFieldLayoutCursor::from_root(&root_field);
        for (field_name, field_symbol, field_index, case_variant) in suffix.iter() {
            cursor = resolve_nested_stored_integer_layout_step(
                &input.layouts,
                cursor,
                field_name,
                field_symbol,
                field_index,
                case_variant,
            )?;
        }
        let byte_offset = cursor.byte_offset();
        return stored_integer_projection_from_cursor(
            cursor,
            RuntimeStoredIntegerSource::Direct {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
            },
        );
    }

    resolve_machine_owned_stored_integer_in_table(
        &input.layouts,
        input,
        dispatch_index,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        expression,
    )
}

fn stored_integer_projection_from_cursor(
    cursor: NestedFieldLayoutCursor<'_>,
    source: RuntimeStoredIntegerSource,
) -> Option<RuntimeStoredIntegerProjection> {
    let stored = cursor.stored_integer()?;
    if stored.stored_width_bits == 0 || stored.stored_width_bits % 8 != 0 {
        return None;
    }
    let carrier = descriptor_primitive_type(cursor.type_descriptor())?;
    Some(RuntimeStoredIntegerProjection {
        source,
        stored_byte_count: usize::from(stored.stored_width_bits / 8),
        carrier_byte_count: cursor.layout().size,
        interpretation: stored.interpretation,
        carrier_signed: carrier.is_signed_integer(),
        write_is_total: stored.write_is_total,
    })
}

fn resolve_runtime_frame_indexed_stored_integer_projection_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoredIntegerProjection> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    if runtime_frame_slot_is_inline_fixed_array_storage(input, collection_slot) {
        return None;
    }
    let descriptor_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    if descriptor_place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }
    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let cursor = resolve_indexed_stored_integer_suffix_cursor_in_table(
        &input.layouts,
        NestedFieldLayoutCursor::from_root(&root_field),
        expressions,
        indexed.suffix_root,
        indexed.boundary,
    )?;
    let field_byte_offset = cursor.byte_offset();
    stored_integer_projection_from_cursor(
        cursor,
        RuntimeStoredIntegerSource::FrameIndexed {
            descriptor_offset: descriptor_place.byte_offset,
            index_region: index_place.region,
            index_offset: index_place.byte_offset,
            index_byte_size: index_place.byte_count,
            element_byte_size: element_layout.size,
            field_byte_offset,
        },
    )
}

fn resolve_runtime_frame_base_indexed_stored_integer_projection_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoredIntegerProjection> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let owned_element: TypeLayoutDescriptor;
    let (array_prefix_offset, element_descriptor, element_stride) =
        if let Some(element) = inline_fixed_array_element_type(&collection_slot.type_descriptor) {
            (0usize, element, None)
        } else {
            let path = normalized_storage_name_path_in_table(expressions, indexed.collection)?;
            if path.len() <= 1 {
                return None;
            }
            let root = FieldLayout {
                symbol: collection_slot.symbol,
                name: collection_slot.name.clone(),
                offset: 0,
                type_symbol: collection_slot.type_descriptor.storage_symbol(),
                type_name: "".into(),
                type_descriptor: collection_slot.type_descriptor.clone(),
                layout: TypeLayout {
                    size: collection_slot.byte_size,
                    alignment: collection_slot.alignment,
                },
            };
            let mut cursor = NestedFieldLayoutCursor::from_root(&root);
            for (field_name, field_symbol, field_index, case_variant) in path.suffix(1).iter() {
                cursor = resolve_nested_field_layout_step(
                    &input.layouts,
                    cursor,
                    field_name,
                    field_symbol,
                    field_index,
                    case_variant,
                )?;
            }
            owned_element = inline_fixed_array_element_type(cursor.type_descriptor())?.clone();
            (
                cursor.byte_offset(),
                &owned_element,
                cursor.repeated_element_stride(),
            )
        };
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    if index_place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let cursor = resolve_indexed_stored_integer_suffix_cursor_in_table(
        &input.layouts,
        NestedFieldLayoutCursor::from_root(&root_field),
        expressions,
        indexed.suffix_root,
        indexed.boundary,
    )?;
    let field_byte_offset = cursor.byte_offset();
    stored_integer_projection_from_cursor(
        cursor,
        RuntimeStoredIntegerSource::FrameBaseIndexed {
            base_byte_offset: collection_slot
                .byte_offset
                .checked_add(array_prefix_offset)?,
            index_offset: index_place.byte_offset,
            index_byte_size: index_place.byte_count,
            element_byte_size: element_stride.unwrap_or(element_layout.size),
            field_byte_offset,
        },
    )
}

fn resolve_runtime_machine_indexed_stored_integer_projection_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoredIntegerProjection> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    if runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )
    .is_some()
    {
        return None;
    }
    let collection = resolve_machine_owned_collection_with_const_prefix_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    if !matches!(
        index_place.region,
        RuntimeStorageRegion::RuntimeFrame | RuntimeStorageRegion::Machine
    ) {
        return None;
    }
    let element_descriptor = collection.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let cursor = resolve_indexed_stored_integer_suffix_cursor_in_table(
        &input.layouts,
        NestedFieldLayoutCursor::from_root(&root_field),
        expressions,
        indexed.suffix_root,
        indexed.boundary,
    )?;
    let field_byte_offset = cursor.byte_offset();
    stored_integer_projection_from_cursor(
        cursor,
        RuntimeStoredIntegerSource::MachineIndexed {
            base_byte_offset: collection.byte_offset,
            index_region: index_place.region,
            index_offset: index_place.byte_offset,
            index_byte_size: index_place.byte_count,
            element_byte_size: collection.element_stride.unwrap_or(element_layout.size),
            field_byte_offset,
        },
    )
}

/// Fold `<receiver>.len` to its STATIC element count when the receiver is (an
/// alias of) a FIXED ARRAY. A fixed array stored inline has no runtime length
/// field -- its length is a layout constant -- so a `.len` read on it (or on a
/// full `as_slice()` view of it) cannot resolve to a storage place; it folds to
/// an immediate instead.
///
/// The motivating shape is an inline-leaf VALUE-call arm guard `s.len > 0`
/// where the callee param `s` was substituted (through the branch's argument
/// bindings) to a caller local `let s = self.arr.as_slice()` that runtime
/// storage ELIDED (the local is only used as a call argument, so it has no
/// frame slot and therefore no slice descriptor to read a length from). The
/// receiver is traced through such unmaterialized local aliases back to the
/// fixed array they view.
pub(super) fn static_fixed_array_len_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<i64> {
    let expression = peel_mutable_in_table(expressions, expression);
    let ExpressionNode::Member(member) = expressions.expression(expression) else {
        return None;
    };
    if member.member.as_str() != "len" {
        return None;
    }
    let length = fixed_array_length_of_receiver_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        member.receiver,
        0,
    )?;
    i64::try_from(length).ok()
}

fn peel_mutable_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(inner) => peel_mutable_in_table(expressions, *inner),
        _ => expression,
    }
}

/// A subslice bound (`a`/`b` in `arr[a..b]`) as a literal `i64`, peeling `Mutable`.
/// `None` for a non-literal (runtime) bound.
fn subslice_bound_literal_in_table(
    expressions: &ExpressionTable,
    handle: ExpressionHandle,
) -> Option<i64> {
    if !handle.is_valid() {
        return None;
    }
    match expressions.expression(peel_mutable_in_table(expressions, handle)) {
        ExpressionNode::Integer(value) => value.value_i64(),
        _ => None,
    }
}

/// How many unmaterialized local-alias hops (`let s = t;` / `let t = self.arr
/// .as_slice();`) the fixed-array `.len` fold will trace before giving up.
const FIXED_ARRAY_ALIAS_TRACE_DEPTH_LIMIT: usize = 4;

/// The static length of the fixed array a receiver expression views, if any:
/// a machine-owned field (`self.arr`, `self.arr.as_slice()` -- path
/// normalization peels full slice views), a frame-slot field path, or an
/// unmaterialized local alias traced through its initializer.
fn fixed_array_length_of_receiver_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    receiver: ExpressionHandle,
    depth: usize,
) -> Option<usize> {
    if depth > FIXED_ARRAY_ALIAS_TRACE_DEPTH_LIMIT {
        return None;
    }

    // A literal-bounded subslice receiver `arr[a..b]` -- often the inlined value of
    // a folded `let sub = arr[a..b]` local -- has `.len` equal to the window length
    // `b - a`, a compile-time constant. No runtime descriptor slot exists for such
    // an inlined subslice, so the place resolvers miss `sub.len` and the value-write
    // resolver would drop it (a silent read-0 in a `let n = sub.len`). Fold it here
    // so the value-write and guard/operand paths agree (mirrors the operand-path
    // `fixed_array_subslice_length`). A RUNTIME-bounded subslice (`arr[i..j]`) is not
    // a constant, so a non-literal bound falls through to the resolvers below.
    if let ExpressionNode::Indexed(indexed) = expressions.expression(receiver) {
        let collection = indexed.collection;
        let index = indexed.index;
        if let ExpressionNode::Range(range) = expressions.expression(index) {
            let (range_start, range_end, end_inclusive) =
                (range.start, range.end, range.end_inclusive);
            let start = subslice_bound_literal_in_table(expressions, range_start).unwrap_or(0);
            let end = if range_end.is_valid() {
                subslice_bound_literal_in_table(expressions, range_end)?
            } else {
                i64::try_from(fixed_array_length_of_receiver_in_table(
                    input,
                    dispatch_index,
                    source_key,
                    expressions,
                    collection,
                    depth + 1,
                )?)
                .ok()?
            };
            let end = if end_inclusive {
                end.checked_add(1)?
            } else {
                end
            };
            return usize::try_from(end.checked_sub(start)?).ok();
        }
    }

    if let Some(target) = resolve_machine_owned_collection_in_table(
        &input.layouts,
        input,
        dispatch_index,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        receiver,
    ) && let Some((_, length)) = target.type_descriptor.fixed_array()
    {
        return Some(length);
    }

    let path = normalized_storage_name_path_in_table(expressions, receiver)?;
    if path.is_empty() {
        return None;
    }

    if let Some(slot) =
        find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
            slot_matches_table_path(slot, &path)
        })
    {
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: slot.byte_offset,
            type_symbol: slot.type_symbol,
            type_name: slot.type_name.clone(),
            type_descriptor: slot.type_descriptor.clone(),
            layout: TypeLayout {
                size: slot.byte_size,
                alignment: slot.alignment,
            },
        };
        let mut cursor = NestedFieldLayoutCursor::from_root(&root_field);
        for (field_name, field_symbol, field_index, case_variant) in path.suffix(1).iter() {
            cursor = resolve_nested_field_layout_step(
                &input.layouts,
                cursor,
                field_name,
                field_symbol,
                field_index,
                case_variant,
            )?;
        }
        return cursor
            .type_descriptor()
            .fixed_array()
            .map(|(_, length)| length);
    }

    // An ELIDED local (no frame slot): trace its declared initializer. Only a
    // bare single-name path can be a local.
    if path.len() != 1 {
        return None;
    }
    let initializer =
        state_local_initializer(input, source_key, path.head_symbol(), path.member(0)?)?;
    fixed_array_length_of_receiver_in_table(
        input,
        dispatch_index,
        source_key,
        &input.program.expression_table,
        initializer,
        depth + 1,
    )
}

/// Fold an ELIDED local (a single-name path with no frame slot, folded into
/// its initializer by storage planning) to its STATIC initializer value -- a
/// boolean or integer literal, possibly through `mut` wrappers or a chain of
/// such locals. The motivating shape is an inline-leaf VALUE-call arm guard
/// substituted with a caller local used only as the call argument (`let flag:
/// bool = true; self.out = self.pick(flag)` -- the arm guard reads `flag`,
/// which has no storage to compare against). Elision implies the local folds
/// into its initializer everywhere (a mutated or `&mut`-escaped local gets a
/// frame slot), so the fold is sound.
pub(super) fn static_elided_local_value_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<i64> {
    static_elided_local_value_traced(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
        0,
    )
}

fn static_elided_local_value_traced(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<i64> {
    if depth > FIXED_ARRAY_ALIAS_TRACE_DEPTH_LIMIT {
        return None;
    }
    let expression = peel_mutable_in_table(expressions, expression);
    match expressions.expression(expression) {
        ExpressionNode::Boolean(value) => return Some(i64::from(*value)),
        ExpressionNode::Integer(value) => return value.value_i64(),
        ExpressionNode::Name(_) => {}
        _ => return None,
    }

    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.len() != 1 {
        return None;
    }
    // A MATERIALIZED local has runtime storage and may be mutated after its
    // initializer; never fold it here (the place resolvers read the slot).
    if find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
        slot_matches_table_path(slot, &path)
    })
    .is_some()
    {
        return None;
    }
    let initializer =
        state_local_initializer(input, source_key, path.head_symbol(), path.member(0)?)?;
    static_elided_local_value_traced(
        input,
        dispatch_index,
        source_key,
        &input.program.expression_table,
        initializer,
        depth + 1,
    )
}

/// The declared initializer of the local named by `symbol`/`name` in
/// `source_key`'s state body, read from the checked program statements.
fn state_local_initializer(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    symbol: SymbolHandle,
    name: &Identifier,
) -> Option<ExpressionHandle> {
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
        .find_map(|statement| {
            let psi_checked_trees::statement::StatementNode::LocalData(local_data) = statement
            else {
                return None;
            };
            let matches_symbol =
                symbol.is_valid() && local_data.symbol.is_valid() && local_data.symbol == symbol;
            (matches_symbol || local_data.name == *name).then_some(local_data.initial_value)
        })
        .filter(|initializer| initializer.is_valid())
}

/// Best-effort signedness of the integer place named by `expression`. Resolves
/// the frame-slot field path to its leaf primitive type. Returns `None` when the
/// type cannot be determined here (non-frame places, non-primitive leaves), in
/// which case callers fall back to the signed form. Used to pick signed vs
/// unsigned division/shift encodings.
pub(super) fn resolve_runtime_storage_is_signed_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<bool> {
    // A value-returning CALL carries the signedness of the selected callable's
    // declared result. This matters for named numeric conversions: unlike an
    // inline `as` cast, `narrow_u64_to_u32_wrapping(x)` remains a Call node, so
    // probing only storage places loses its u32 result and silently selects the
    // signed modulo fallback for a surrounding `%`.
    if let ExpressionNode::Call(call) = expressions.expression(expression) {
        let return_type = input
            .program
            .machines()
            .iter()
            .flat_map(|machine| input.program.machine_states(machine).iter())
            .find(|state| state.symbol == call.target_symbol)
            .map(|state| state.return_type)
            .or_else(|| {
                input
                    .program
                    .machine_parameter_signature(call.target_symbol)
                    .map(|(_, signature)| signature.return_type)
            })?;
        let primitive = input.program.primitive_type_reference(return_type)?;
        return primitive
            .accepts_integer_literal()
            .then(|| primitive.is_signed_integer());
    }
    // A numeric `as` cast operand has the signedness of the cast's TARGET type
    // (`(x as u32) % k` must pick the unsigned modulo regardless of `x`'s own
    // type) -- without this, the place resolution below fails on the Cast node
    // and the caller falls back to the signed encoding.
    if let ExpressionNode::Cast(cast) = expressions.expression(expression) {
        let target = input.program.primitive_type_reference(cast.target_type)?;
        return Some(target.is_signed_integer());
    }
    // A nested BINARY operand has the signedness of its own operands (one
    // witness, left first -- mixed signedness classes are checker-rejected):
    // `(a / b) % k` must pick the unsigned modulo when `a` is u32, or a
    // high-bit dividend runs the signed idiv and yields a negative remainder.
    // (Re-applied 2026-07-10: an earlier attempt was reverted as "ineffective"
    // because the ARG-materialization path was ALSO broken then and masked
    // this fix; that path has since been repaired, leaving only the fused
    // guard-subject resolution, which this recursion completes.)
    if let ExpressionNode::Binary(binary) = expressions.expression(expression) {
        return resolve_runtime_storage_is_signed_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.left,
        )
        .or_else(|| {
            resolve_runtime_storage_is_signed_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                binary.right,
            )
        });
    }
    // A LANDED constant carries its own signedness (carrier CR3, ch5
    // two-phase law): a folded unsigned local keeps typing the operand even
    // after the place is gone. Anonymous literals stay untyped and fall
    // through to the caller's ordinary fallback chain.
    if let ExpressionNode::Integer(literal) = expressions.expression(expression) {
        if let Some(landing) = literal.landing() {
            return Some(landing.landed_type.is_signed());
        }
    }
    let descriptor = resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    descriptor_primitive_is_signed(&descriptor)
}

/// Resolve the leaf primitive type of a runtime storage target (a `data` field
/// or frame slot, possibly through nested fields). Used to pick float vs integer
/// codegen for a binary write.
pub(super) fn resolve_runtime_storage_primitive_type_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    let descriptor = resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    descriptor_primitive_type(&descriptor)
}

/// Whether a storage PLACE is a fat `{ptr, len}` slice/text descriptor -- a
/// `&[T]`/`&[u8] in Utf8` view or a bare slice. Text content-comparison leaves
/// recognize the slice descriptor directly. Type-driven (not size-driven): an arbitrary
/// 16-byte aggregate must never be misread as a text descriptor.
pub(super) fn resolve_runtime_storage_place_is_fat_slice_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )
    .is_some_and(|descriptor| descriptor_is_fat_slice(&descriptor))
}

/// Whether a storage PLACE is an owned `[u8; N]` bounded byte carrier
/// (`BoundedByteBuffer`, `{len, bytes}` inline). Unlike a fat-slice descriptor,
/// the carrier owns its bytes -- its content lives at `place + pointer_size` and
/// its length at `place + 0` -- so a content read must use carrier addressing,
/// not a `{ptr, len}` descriptor load. Resolves the leaf descriptor (peeling a
/// domain `Constrained` wrapper).
pub(super) fn resolve_runtime_storage_place_is_bounded_byte_buffer_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )
    .is_some_and(|descriptor| descriptor_is_bounded_byte_buffer(&descriptor))
}

/// Whether a storage place is a raw fixed `[u8; N]` array. Unlike a bounded
/// text carrier, this shape has no leading length word; `read_line` may use it
/// as disposable scratch and writes bytes starting at the place itself.
pub(super) fn resolve_runtime_storage_place_is_fixed_byte_array_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )
    .is_some_and(|descriptor| descriptor_is_fixed_byte_array(&descriptor))
}

/// Whether a storage PLACE is an owned `[u8; N]` bounded byte carrier
/// (`BoundedByteBuffer`, `{len, bytes}` inline). Unlike a fat-slice descriptor,
/// the carrier owns its bytes; a literal write into it must store `len` + copy
/// the content inline, not stamp a `{ptr,len}` descriptor. Resolves the target
/// expression's leaf descriptor (peeling a domain `Constrained` wrapper).
pub(super) fn resolve_runtime_storage_place_is_bounded_byte_buffer(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    resolved_target: &psi_checked_trees::expression::Expression,
) -> bool {
    let mut expressions = ExpressionTable::default();
    let handle = expressions.insert_tree(resolved_target);
    resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index,
        source_key,
        &expressions,
        handle,
    )
    .is_some_and(|descriptor| descriptor_is_bounded_byte_buffer(&descriptor))
}

pub(super) fn descriptor_is_bounded_byte_buffer(descriptor: &TypeLayoutDescriptor) -> bool {
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. } => {
            descriptor_is_bounded_byte_buffer(base_type)
        }
        TypeLayoutDescriptor::Reference { referee, .. } => {
            descriptor_is_bounded_byte_buffer(referee)
        }
        TypeLayoutDescriptor::BoundedByteBuffer { .. } => true,
        _ => false,
    }
}

fn descriptor_is_fixed_byte_array(descriptor: &TypeLayoutDescriptor) -> bool {
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. }
        | TypeLayoutDescriptor::Reference {
            referee: base_type, ..
        } => descriptor_is_fixed_byte_array(base_type),
        TypeLayoutDescriptor::FixedArray { element_type, .. } => {
            descriptor_primitive_type(element_type) == Some(PrimitiveType::U8)
        }
        _ => false,
    }
}

/// The element type of a `[u8; N]` carrier (`BoundedByteBuffer`), peeling a domain
/// `Constrained` wrapper. `None` for any non-carrier descriptor.
fn bounded_byte_buffer_element_type(
    descriptor: &TypeLayoutDescriptor,
) -> Option<&TypeLayoutDescriptor> {
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. } => {
            bounded_byte_buffer_element_type(base_type)
        }
        TypeLayoutDescriptor::BoundedByteBuffer { element_type, .. } => Some(element_type),
        _ => None,
    }
}

/// Resolve `<carrier>[index]` (a byte read into a `[u8; N]` carrier) to its
/// storage place. The carrier holds its content inline at `+pointer_size` (after
/// the length word) with `element_type`-sized elements, so the indexed byte sits
/// at `base + pointer_size + index * element_size`. Mirrors the fixed-array
/// element path (including any indexed-suffix layout) but offsets into the
/// carrier's content region. `None` for a non-carrier collection (the caller then
/// falls through to the fixed-array resolution).
fn bounded_byte_buffer_indexed_place(
    input: &InstructionSelectionInput<'_>,
    collection_descriptor: &TypeLayoutDescriptor,
    region: RuntimeStorageRegion,
    base_offset: usize,
    index: usize,
    expressions: &ExpressionTable,
    suffix_root: ExpressionHandle,
    boundary: ExpressionHandle,
) -> Option<RuntimeStoragePlace> {
    let element_descriptor = bounded_byte_buffer_element_type(collection_descriptor)?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_offset = index.checked_mul(element_layout.size)?;
    let root_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout, _) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        suffix_root,
        boundary,
    )?;
    Some(RuntimeStoragePlace {
        region,
        byte_offset: base_offset
            .checked_add(input.runtime_abi.pointer_size)?
            .checked_add(element_offset)?
            .checked_add(field_byte_offset)?,
        byte_count: field_layout.size,
    })
}

/// The decision-17 arithmetic domain a BINARY OPERAND TREE computes in, plus
/// whether its operands are SIGNED integers: the first non-Exact witness among
/// the expression's leaves decides both (the domain rides the operands'
/// declared types; mixed domain classes are a checker concern, so one witness
/// types the tree -- the same convention the signedness classifiers use).
/// Recorded on `ValueOperand::Binary` at construction: the Saturating/Trapping
/// operand-position lowering picks its width-correct op + clamp/trap bounds
/// from these, and encoding one as the plain op instead silently computed the
/// unclamped wide value (150 instead of the saturated 127) or skipped the
/// trap. For an all-Exact tree the signedness is the LEFT leaf's (unused by
/// the plain lowering).
pub(in crate::selection) fn resolve_binary_operand_arithmetic_domain_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> (psi_numerics::arithmetic::ArithmeticDomain, bool) {
    let left_witness = resolve_expression_arithmetic_domain_witness_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        left,
    );
    if left_witness.0 != psi_numerics::arithmetic::ArithmeticDomain::Exact {
        return left_witness;
    }
    let right_witness = resolve_expression_arithmetic_domain_witness_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        right,
    );
    if right_witness.0 != psi_numerics::arithmetic::ArithmeticDomain::Exact {
        return right_witness;
    }
    left_witness
}

/// The (domain, signedness) witness of ONE operand expression: a nested binary
/// node recurses into its own operands (its leaf descriptor never resolves --
/// a binary node is not a place); everything else resolves through the
/// place/cast rules of [`resolve_runtime_storage_arithmetic_domain_in_table`]
/// with the signedness read from the SAME expression (signed when
/// unresolvable, which only matters when the domain resolved non-Exact -- and
/// a resolved domain implies a resolved declared type).
fn resolve_expression_arithmetic_domain_witness_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> (psi_numerics::arithmetic::ArithmeticDomain, bool) {
    if let ExpressionNode::Binary(binary) = expressions.expression(expression) {
        return resolve_binary_operand_arithmetic_domain_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.left,
            binary.right,
        );
    }
    let domain = resolve_runtime_storage_arithmetic_domain_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    );
    let signed = resolve_runtime_storage_is_signed_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )
    .unwrap_or(true);
    (domain, signed)
}

/// The fused binary WRITE's domain witness: both operand expressions combined
/// (Exact neutral), seeing through NESTED binary operands. The leaf place
/// resolver returns Exact for a binary node (a binary is not a place), so the
/// old per-operand-place combine silently dropped the domain whenever an
/// operand was itself arithmetic -- `(a + b) + 50` at u8-Saturating lowered
/// the OUTER add PLAIN and the store truncated the unclamped value (49, not
/// 255). Same recursive witness the operand-position lowering uses, so write
/// and operand sites can never disagree.
pub(in crate::selection) fn resolve_binary_write_arithmetic_domain_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> psi_numerics::arithmetic::ArithmeticDomain {
    resolve_binary_operand_arithmetic_domain_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        left,
        right,
    )
    .0
}

/// The arithmetic policy selected for one table-shaped binary/named float
/// operation. Normalized checked operators consume the exact provider identity
/// and adapter carried through control flow. Neither fact may be reconstructed
/// from operand types during lowering.
///
/// `None` is a fail-closed result for contradictory evidence or a carried
/// binary32/binary64 adapter whose format disagrees with the actual operation
/// width.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn resolve_binary_operation_arithmetic_domain_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    left: ExpressionHandle,
    right: ExpressionHandle,
    is_float: bool,
    byte_width: usize,
) -> Option<psi_numerics::arithmetic::ArithmeticDomain> {
    if is_float {
        match crate::selection::lookups::carried_float_provider_plan(
            input,
            source_key,
            statement_index,
            expressions,
            expression,
        ) {
            crate::selection::lookups::CarriedFloatProviderPlan::Invalid => return None,
            crate::selection::lookups::CarriedFloatProviderPlan::Resolved(identity) => {
                let Some(plan) = input.selected_provider_plans.plan_by_identity(identity) else {
                    return None;
                };
                if plan.rows.len() != 1 {
                    return None;
                }
            }
            crate::selection::lookups::CarriedFloatProviderPlan::Missing => return None,
        }
        return match crate::selection::lookups::carried_float_policy_domain(
            input,
            source_key,
            statement_index,
            expressions,
            expression,
            byte_width,
        ) {
            crate::selection::lookups::CarriedFloatPolicyDomain::Resolved(domain) => Some(domain),
            crate::selection::lookups::CarriedFloatPolicyDomain::Invalid => None,
            crate::selection::lookups::CarriedFloatPolicyDomain::Missing => None,
        };
    }
    Some(resolve_binary_write_arithmetic_domain_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        left,
        right,
    ))
}

/// Non-table sibling of [`resolve_binary_write_arithmetic_domain_in_table`]
/// for the older `&Expression` write path.
pub(super) fn resolve_binary_write_arithmetic_domain(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    left: &Expression,
    right: &Expression,
) -> psi_numerics::arithmetic::ArithmeticDomain {
    let mut delegated_expressions = ExpressionTable::default();
    let left_handle = delegated_expressions.insert_tree(left);
    let right_handle = delegated_expressions.insert_tree(right);
    resolve_binary_write_arithmetic_domain_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        left_handle,
        right_handle,
    )
}

/// The arithmetic domain (`T in Wrapping/Saturating/Trapping`, decision 17) of a
/// storage PLACE — read from the `Constrained` wrapper the layout builder records
/// on the slot/field descriptor. Used at the binary-write site to decide whether
/// the target wants the plain wrapping op (Exact/Wrapping) or saturating/trapping
/// overflow handling. Defaults to `Exact` for unconstrained or unresolved places.
pub(super) fn resolve_runtime_storage_arithmetic_domain_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> psi_numerics::arithmetic::ArithmeticDomain {
    // A domain `as` cast (`x as u8 in Saturating`, decision 17 S2) re-tags the
    // value's domain explicitly, overriding the operand's own type.
    if let ExpressionNode::Cast(cast) = expressions.expression(expression) {
        return cast.domain;
    }
    resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )
    .map(|descriptor| descriptor.arithmetic_domain())
    .unwrap_or(psi_numerics::arithmetic::ArithmeticDomain::Exact)
}

/// The scalar primitive type of a VALUE/source expression, for codegen
/// classification (float-vs-integer, byte width). The single funnel every
/// binary-write / convert producer should use so they all agree: a storage PLACE
/// of any shape resolves to its leaf type; a LITERAL classifies from its node
/// and carried landing (anonymous float/integer literals default to f64/i64;
/// landed literals keep their format/type); a boolean is `bool`. Returns
/// `None` for non-scalar / unresolved expressions.
/// (A `Cast` value resolves via its own selection, so it is not classified here.)
pub(super) fn classify_scalar_value_type_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    if let Some(primitive) = resolve_runtime_storage_primitive_type_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(primitive);
    }
    match expressions.expression(expression) {
        // A float literal's LANDED format is its type (F2b destination
        // stamping): an f32-stamped literal in operand position must
        // classify F32 so a nested binary plans the 4-byte op (`addss`) --
        // classifying F64 here made the inner op of an f32 chain compute
        // `addsd` over f32 bit patterns (the width-witness divergence).
        ExpressionNode::Float(literal) => Some(match literal.landing() {
            Some(psi_numerics::literals::FloatFormat::F32) => PrimitiveType::F32,
            _ => PrimitiveType::F64,
        }),
        ExpressionNode::Integer(literal) => Some(
            literal
                .landing()
                .map(|landing| primitive_type_for_landed_integer(landing.landed_type))
                .unwrap_or(PrimitiveType::I64),
        ),
        ExpressionNode::Boolean(_) => Some(PrimitiveType::Bool),
        // A comparison or logical conjunction/disjunction always produces bool,
        // independently of its operands. This matters recursively: the left side
        // of `(float_compare && float_compare)` is itself a binary expression and
        // must not make the outer `&&` look like a float operation. Arithmetic and
        // bitwise sub-expressions instead retain the type of their operands. This
        // lets a convert see through an arithmetic source to pick single vs double
        // precision and the source width without leaking operand type through a
        // bool-producing node.
        ExpressionNode::Binary(binary) => {
            if binary_operator_result_is_bool(binary.operator) {
                return Some(PrimitiveType::Bool);
            }
            let left = classify_scalar_value_type_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                binary.left,
            );
            let right = classify_scalar_value_type_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                binary.right,
            );
            combine_binary_operand_scalar_types(left, right)
        }
        // Numeric builtins (`min`, `max`, and the nested form produced by
        // `clamp`) preserve their operands' scalar type. They can surface here
        // after a compiler-elided local initializer is substituted into a
        // nested named conversion call.
        ExpressionNode::Call(call)
            if crate::selection::runtime_dispatch::writes::mutation::builtin_runtime_call_operator_in_table(
                input, call,
            )
            .is_some() =>
        {
            let left = expressions.expression_handle_at_offset(call.arguments, 0);
            let right = expressions.expression_handle_at_offset(call.arguments, 1);
            combine_binary_operand_scalar_types(
                classify_scalar_value_type_in_table(
                    input,
                    dispatch_index,
                    source_key,
                    expressions,
                    left,
                ),
                classify_scalar_value_type_in_table(
                    input,
                    dispatch_index,
                    source_key,
                    expressions,
                    right,
                ),
            )
        }
        ExpressionNode::Call(call)
            if crate::selection::runtime_dispatch::writes::mutation::builtin_runtime_unary_call_operator_in_table(
                input, call,
            )
            .is_some_and(
                crate::selection::runtime_dispatch::writes::mutation::float_unary_result_is_bool,
            ) =>
        {
            Some(PrimitiveType::Bool)
        }
        // Float square root preserves its operand format. `FloatClassify`
        // also reaches this arm because its native operand-width metadata must
        // retain the source format; its aggregate result layout is resolved by
        // the write target rather than this scalar helper.
        ExpressionNode::Call(call)
            if crate::selection::runtime_dispatch::writes::mutation::builtin_runtime_unary_call_operator_in_table(
                input, call,
            )
            .is_some() =>
        {
            classify_scalar_value_type_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                expressions.expression_handle_at_offset(call.arguments, 0),
            )
        }
        // A nested cast (`(self.src as f64) as i32` after `wide` is folded into
        // the outer cast) has the type of its TARGET. Without this, the outer
        // cast's source-width re-derivation returned None and the entire write
        // was silently dropped (the f32->f64-local->i32 miscompile). The
        // `as`-value resolves via its own Convert selection; here we only need
        // its result type so a CONSUMING cast can size its source.
        ExpressionNode::Cast(cast) => input.program.primitive_type_reference(cast.target_type),
        // A slice/array element read (`s[0]`) has the COLLECTION's element type.
        // Without this, a cast of an element (`s[0] as i32 in Wrapping`) could not
        // classify its source, the cast's `?` bailed, the whole binary operand
        // nulled out, and the accumulator argument was silently dropped (read 0).
        ExpressionNode::Indexed(indexed) => {
            let collection_descriptor = resolve_runtime_storage_leaf_descriptor_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                indexed.collection,
            )?;
            descriptor_primitive_type(collection_descriptor.element_type()?)
        }
        _ => None,
    }
}

fn binary_operator_result_is_bool(operator: psi_checked_trees::expression::BinaryOperator) -> bool {
    use psi_checked_trees::expression::BinaryOperator;

    matches!(
        operator,
        BinaryOperator::And
            | BinaryOperator::Or
            | BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    )
}

fn primitive_type_for_landed_integer(
    landed: psi_numerics::literals::LandedIntegerType,
) -> PrimitiveType {
    use psi_numerics::literals::LandedIntegerType;
    match landed {
        LandedIntegerType::I8 => PrimitiveType::I8,
        LandedIntegerType::I16 => PrimitiveType::I16,
        LandedIntegerType::I32 => PrimitiveType::I32,
        LandedIntegerType::I64 => PrimitiveType::I64,
        LandedIntegerType::U8 => PrimitiveType::U8,
        LandedIntegerType::U16 => PrimitiveType::U16,
        LandedIntegerType::U32 => PrimitiveType::U32,
        LandedIntegerType::U64 => PrimitiveType::U64,
        LandedIntegerType::Addr => PrimitiveType::Addr,
    }
}

/// Combine two operands' classified scalar types into the binary's result type.
/// Prefer a place-resolved operand (its width is exact) over a bare literal
/// (which defaults to the widest f64/i64); a float on either side makes the
/// result float. Shared by `classify_scalar_value_type_in_table`'s `Binary`
/// arm and the value-operand width helper so they agree on ONE answer (the
/// scalar-width-rederivation smell — see wiki/architecture).
pub(super) fn combine_binary_operand_scalar_types(
    left: Option<PrimitiveType>,
    right: Option<PrimitiveType>,
) -> Option<PrimitiveType> {
    match (left, right) {
        (Some(l), Some(r)) if l.accepts_float_literal() && !r.accepts_float_literal() => Some(l),
        (Some(l), Some(r)) if r.accepts_float_literal() && !l.accepts_float_literal() => Some(r),
        (Some(l), Some(r)) => {
            // Same float-ness: take the narrower (place-resolved) width.
            if scalar_primitive_rank(r) < scalar_primitive_rank(l) {
                Some(r)
            } else {
                Some(l)
            }
        }
        (l, r) => l.or(r),
    }
}

/// A coarse width rank for picking the narrower of two same-float-ness primitives
/// when classifying a binary's type from its operands (a 4-byte f32/i32 ranks below
/// an 8-byte f64/i64; a literal that resolved to the widest default loses to a
/// place-resolved narrower operand).
fn scalar_primitive_rank(primitive: PrimitiveType) -> u8 {
    match primitive {
        PrimitiveType::Bool
        | PrimitiveType::I8
        | PrimitiveType::U8
        | PrimitiveType::I16
        | PrimitiveType::U16 => 0,
        PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => 1,
        PrimitiveType::F64 | PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Addr => 2,
    }
}

/// Non-table counterpart of [`resolve_runtime_storage_primitive_type_in_table`]:
/// resolve the leaf primitive type of a runtime storage target given as a resolved
/// `Expression`.
pub(super) fn resolve_runtime_storage_primitive_type(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<PrimitiveType> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_storage_primitive_type_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

/// Whether a storage TARGET is an atomic-typed place (`AtomicU32`/`AtomicU64`/
/// `AtomicBool`/`AtomicU64`). An RMW on such a place is lowered to a single
/// atomic instruction (`LOCK xadd`). Reads the RAW leaf descriptor name —
/// atomics survive here as `Named{"Atomic*"}` (the `AtomicU32 -> u32` primitive
/// mapping is applied only on demand by `descriptor_primitive_type`).
pub(super) fn runtime_storage_target_is_atomic(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> bool {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    runtime_storage_target_is_atomic_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

pub(super) fn runtime_storage_target_is_atomic_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )
    .is_some_and(|descriptor| descriptor_is_atomic(&descriptor))
}

fn descriptor_is_atomic(descriptor: &TypeLayoutDescriptor) -> bool {
    match descriptor {
        TypeLayoutDescriptor::Named { name, .. } => name.as_str().starts_with("Atomic"),
        TypeLayoutDescriptor::Constrained { base_type, .. } => descriptor_is_atomic(base_type),
        _ => false,
    }
}

/// Non-table sibling of [`resolve_runtime_storage_arithmetic_domain_in_table`]
/// (decision 17): the arithmetic domain of a storage target reached through the
/// older `&Expression` path.
pub(super) fn resolve_runtime_storage_arithmetic_domain(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> psi_numerics::arithmetic::ArithmeticDomain {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_storage_arithmetic_domain_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

/// Non-table sibling of [`resolve_runtime_storage_is_signed_in_table`]: the
/// signedness of a storage target reached through the `&Expression` path.
pub(super) fn resolve_runtime_storage_is_signed(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<bool> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_storage_is_signed_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

/// Walk a runtime storage target (frame slot or machine-owned `data` field, plus
/// any nested-field suffix) to its leaf [`TypeLayoutDescriptor`].
pub(super) fn resolve_runtime_storage_leaf_descriptor_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<TypeLayoutDescriptor> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.is_empty() {
        return None;
    }
    let suffix = path.suffix(1);
    let slot = find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
        slot_matches_table_path(slot, &path)
    })
    .or_else(|| {
        latest_dispatch_frame_slot(input, dispatch_index, |slot| {
            slot_matches_table_path(slot, &path)
        })
    })
    .or_else(|| resymbolized_local_slot_for_path(input, dispatch_index, source_key, &path));

    let Some(slot) = slot else {
        // Not a frame slot: most `data` fields are machine-owned. Resolve the
        // leaf type descriptor through that path instead.
        if let Some(collection) = resolve_machine_owned_collection_in_table(
            &input.layouts,
            input,
            dispatch_index,
            input.entry_key.machine,
            source_key.machine,
            expressions,
            expression,
        ) {
            return Some(collection.type_descriptor.clone());
        }
        // Carrier RECOGNITION for a slice-VIEW element through an elided local
        // (`r[i].label`, r = X.as_slice()): the local has no slot and is not
        // machine-owned, so trace its initializer + see through the as_slice view
        // to the underlying array, take the element type, and walk the field
        // suffix. Without this a carrier field on such an element was not
        // recognized and its `==`/copy lowering bailed (the lookup's `room.label`,
        // a sibling of the i32 element-place fix in
        // resolve_runtime_fixed_indexed_place_in_table).
        if path.member_index(0).is_some() {
            let array = resolve_elided_local_slice_view_array(
                input,
                dispatch_index,
                source_key,
                path.head_symbol(),
                path.member(0)?,
            )?;
            let element_descriptor = inline_fixed_array_element_type(&array.type_descriptor)?;
            let element_layout = descriptor_layout(input, element_descriptor);
            let root_field = FieldLayout {
                symbol: SymbolHandle::invalid(),
                name: "".into(),
                offset: 0,
                type_symbol: element_descriptor.storage_symbol(),
                type_name: "".into(),
                type_descriptor: element_descriptor.clone(),
                layout: element_layout,
            };
            let mut cursor = NestedFieldLayoutCursor::from_root(&root_field);
            for (field_name, field_symbol, field_index, case_variant) in suffix.iter() {
                cursor = resolve_nested_field_layout_step(
                    &input.layouts,
                    cursor,
                    field_name,
                    field_symbol,
                    field_index,
                    case_variant,
                )?;
            }
            return Some(cursor.type_descriptor().clone());
        }
        return None;
    };

    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: slot.byte_offset,
        type_symbol: slot.type_symbol,
        type_name: slot.type_name.clone(),
        type_descriptor: slot.type_descriptor.clone(),
        layout: TypeLayout {
            size: slot.byte_size,
            alignment: slot.alignment,
        },
    };

    let mut cursor = NestedFieldLayoutCursor::from_root(&root_field);
    for (field_name, field_symbol, field_index, case_variant) in suffix.iter() {
        cursor = resolve_nested_field_layout_step(
            &input.layouts,
            cursor,
            field_name,
            field_symbol,
            field_index,
            case_variant,
        )?;
    }
    Some(cursor.type_descriptor().clone())
}

fn descriptor_primitive_is_signed(descriptor: &TypeLayoutDescriptor) -> Option<bool> {
    Some(descriptor_primitive_type(descriptor)?.is_signed_integer())
}

pub(super) fn descriptor_primitive_type(
    descriptor: &TypeLayoutDescriptor,
) -> Option<PrimitiveType> {
    match descriptor {
        TypeLayoutDescriptor::Named { name, .. } => PrimitiveType::from_name(name),
        TypeLayoutDescriptor::Constrained { base_type, .. } => descriptor_primitive_type(base_type),
        _ => None,
    }
}

pub(super) fn resolve_fixed_array_length_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<usize> {
    if let Some(slot) = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        if let Some((_, length)) = slot.type_descriptor.fixed_array() {
            return Some(length);
        }

        let path = normalized_storage_name_path_in_table(expressions, expression)?;
        if path.len() > 1 {
            let root_field = FieldLayout {
                symbol: slot.symbol,
                name: slot.name.clone(),
                offset: 0,
                type_symbol: slot.type_descriptor.storage_symbol(),
                type_name: "".into(),
                type_descriptor: slot.type_descriptor.clone(),
                layout: TypeLayout {
                    size: slot.byte_size,
                    alignment: slot.alignment,
                },
            };
            let mut cursor = NestedFieldLayoutCursor::from_root(&root_field);
            for (field_name, field_symbol, field_index, case_variant) in path.suffix(1).iter() {
                cursor = resolve_nested_field_layout_step(
                    &input.layouts,
                    cursor,
                    field_name,
                    field_symbol,
                    field_index,
                    case_variant,
                )?;
            }
            let (_, length) = cursor.type_descriptor().fixed_array()?;
            return Some(length);
        }
    }

    if let Some(indexed) = indexed_target_path_in_table(expressions, expression) {
        let collection_slot = runtime_frame_slot_for_expression_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            indexed.collection,
        )?;
        let element_descriptor = inline_fixed_array_element_type(&collection_slot.type_descriptor)?;
        let element_layout = descriptor_layout(input, element_descriptor);
        let root_field = FieldLayout {
            symbol: collection_slot.symbol,
            name: collection_slot.name.clone(),
            offset: 0,
            type_symbol: element_descriptor.storage_symbol(),
            type_name: "".into(),
            type_descriptor: element_descriptor.clone(),
            layout: element_layout,
        };
        let cursor = resolve_indexed_target_suffix_cursor_in_table(
            &input.layouts,
            NestedFieldLayoutCursor::from_root(&root_field),
            expressions,
            indexed.suffix_root,
            indexed.boundary,
        )?;
        let (_, length) = cursor.type_descriptor().fixed_array()?;
        return Some(length);
    }

    let collection = resolve_machine_owned_collection_in_table(
        &input.layouts,
        input,
        dispatch_index,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        expression,
    )?;
    let (_, length) = collection.type_descriptor.fixed_array()?;
    Some(length)
}

pub(super) fn resolve_fixed_array_length(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<usize> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_fixed_array_length_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

pub(super) fn resolve_runtime_frame_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeFrameIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

pub(super) fn resolve_runtime_frame_base_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeFrameBaseIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

pub(super) fn resolve_runtime_frame_base_indexed_target_with_index_region(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeFrameBaseIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

pub(super) fn resolve_runtime_frame_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameIndexedTarget> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    if let Some(target) = resolve_runtime_pointee_indexed_target_from_path(
        input,
        dispatch_index,
        source_key,
        expressions,
        &indexed,
    ) {
        return Some(target);
    }
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    // Dynamic indexing for inline frame arrays needs a base-offset lowering path rather than the
    // descriptor-based slice/view path used by runtime frame indexed targets.
    if runtime_frame_slot_is_inline_fixed_array_storage(input, collection_slot) {
        return None;
    }
    let descriptor_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    if descriptor_place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }

    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout, field_descriptor) =
        resolve_indexed_target_suffix_layout_in_table(
            input,
            &root_field,
            expressions,
            indexed.suffix_root,
            indexed.boundary,
        )?;

    Some(RuntimeFrameIndexedTarget {
        descriptor_offset: descriptor_place.byte_offset,
        index_region: index_place.region,
        index_offset: index_place.byte_offset,
        index_byte_size: index_place.byte_count,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
        is_bounded_byte_buffer: descriptor_is_bounded_byte_buffer(&field_descriptor),
    })
}

/// Runtime indexing through a record-reference slot (`view.items[i].field`).
/// The ordinary frame-indexed resolver expects the slot itself to be a slice
/// descriptor. Recast/reference records instead keep one pointee address in
/// the slot and reach the array only after walking the referee's field layout.
fn resolve_runtime_pointee_indexed_target_from_path(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    indexed: &TableIndexedTargetPath,
) -> Option<RuntimeFrameIndexedTarget> {
    let collection_path = normalized_storage_name_path_in_table(expressions, indexed.collection)?;
    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let pointee_descriptor = slot.type_descriptor.reference_referee()?;
    let pointee_layout = descriptor_layout(input, pointee_descriptor);
    let wide_referee_slot = matches!(
        pointee_descriptor,
        omega_layout::TypeLayoutDescriptor::Named { .. }
    ) && pointee_layout.size > input.runtime_abi.pointer_size
        && slot.byte_size == pointee_layout.size;
    if slot.byte_size != input.runtime_abi.pointer_size && !wide_referee_slot {
        return None;
    }
    let shared_small_content_spill = matches!(
        &slot.type_descriptor,
        omega_layout::TypeLayoutDescriptor::Reference {
            is_mutable: false,
            ..
        }
    ) && matches!(
        pointee_descriptor,
        omega_layout::TypeLayoutDescriptor::Named { .. }
    ) && pointee_layout.size <= input.runtime_abi.pointer_size
        && !input.program.machines().iter().any(|machine| {
            machine.symbol == source_key.machine && machine.supply_mode.is_boundary_declaration()
        });
    if shared_small_content_spill {
        return None;
    }

    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: 0,
        type_symbol: pointee_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: pointee_descriptor.clone(),
        layout: pointee_layout,
    };
    let mut collection_cursor = NestedFieldLayoutCursor::from_root(&root_field);
    for (field_name, field_symbol, field_index, case_variant) in collection_path.suffix(1).iter() {
        collection_cursor = resolve_nested_field_layout_step(
            &input.layouts,
            collection_cursor,
            field_name,
            field_symbol,
            field_index,
            case_variant,
        )?;
    }
    let (element_descriptor, _) = collection_cursor.type_descriptor().fixed_array()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_root = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (element_field_offset, field_layout, field_descriptor) =
        resolve_indexed_target_suffix_layout_in_table(
            input,
            &element_root,
            expressions,
            indexed.suffix_root,
            indexed.boundary,
        )?;
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    Some(RuntimeFrameIndexedTarget {
        descriptor_offset: slot.byte_offset,
        index_region: index_place.region,
        index_offset: index_place.byte_offset,
        index_byte_size: index_place.byte_count,
        element_byte_size: collection_cursor
            .repeated_element_stride()
            .unwrap_or(element_layout.size),
        field_byte_offset: collection_cursor
            .byte_offset()
            .checked_add(element_field_offset)?,
        byte_count: field_layout.size,
        is_bounded_byte_buffer: descriptor_is_bounded_byte_buffer(&field_descriptor),
    })
}

fn resolve_runtime_pointee_indexed_stored_integer_projection_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoredIntegerProjection> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection_path = normalized_storage_name_path_in_table(expressions, indexed.collection)?;
    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let pointee_descriptor = slot.type_descriptor.reference_referee()?;
    let pointee_layout = descriptor_layout(input, pointee_descriptor);
    let wide_referee_slot = matches!(pointee_descriptor, TypeLayoutDescriptor::Named { .. })
        && pointee_layout.size > input.runtime_abi.pointer_size
        && slot.byte_size == pointee_layout.size;
    if slot.byte_size != input.runtime_abi.pointer_size && !wide_referee_slot {
        return None;
    }
    let shared_small_content_spill = matches!(
        &slot.type_descriptor,
        TypeLayoutDescriptor::Reference {
            is_mutable: false,
            ..
        }
    ) && matches!(
        pointee_descriptor,
        TypeLayoutDescriptor::Named { .. }
    ) && pointee_layout.size <= input.runtime_abi.pointer_size
        && !input.program.machines().iter().any(|machine| {
            machine.symbol == source_key.machine && machine.supply_mode.is_boundary_declaration()
        });
    if shared_small_content_spill {
        return None;
    }

    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: 0,
        type_symbol: pointee_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: pointee_descriptor.clone(),
        layout: pointee_layout,
    };
    let mut collection_cursor = NestedFieldLayoutCursor::from_root(&root_field);
    for (field_name, field_symbol, field_index, case_variant) in collection_path.suffix(1).iter() {
        collection_cursor = resolve_nested_field_layout_step(
            &input.layouts,
            collection_cursor,
            field_name,
            field_symbol,
            field_index,
            case_variant,
        )?;
    }
    let (element_descriptor, _) = collection_cursor.type_descriptor().fixed_array()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_root = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let cursor = resolve_indexed_stored_integer_suffix_cursor_in_table(
        &input.layouts,
        NestedFieldLayoutCursor::from_root(&element_root),
        expressions,
        indexed.suffix_root,
        indexed.boundary,
    )?;
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    let field_byte_offset = collection_cursor
        .byte_offset()
        .checked_add(cursor.byte_offset())?;
    stored_integer_projection_from_cursor(
        cursor,
        RuntimeStoredIntegerSource::FrameIndexed {
            descriptor_offset: slot.byte_offset,
            index_region: index_place.region,
            index_offset: index_place.byte_offset,
            index_byte_size: index_place.byte_count,
            element_byte_size: collection_cursor
                .repeated_element_stride()
                .unwrap_or(element_layout.size),
            field_byte_offset,
        },
    )
}

/// Whether a runtime-frame-INDEXED element field (`rooms[i].label`) is a fat
/// `{ptr, len}` slice descriptor (`&[u8]`, including `&[u8] in Utf8`) -- the
/// indexed-element analog of the leaf descriptor resolver. Lets a `text == "literal"` guard over
/// a `&[u8] in Utf8` element resolve to the content-compare text leaf instead of
/// falling through to a raw scalar compare of the descriptor's pointer words.
pub(super) fn resolve_runtime_frame_indexed_is_fat_slice_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    (|| {
        let indexed = indexed_target_path_in_table(expressions, expression)?;
        let collection_slot = runtime_frame_slot_for_expression_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            indexed.collection,
        )?;
        let element_descriptor = collection_slot.type_descriptor.element_type()?;
        let element_layout = descriptor_layout(input, element_descriptor);
        let root_field = FieldLayout {
            symbol: collection_slot.symbol,
            name: collection_slot.name.clone(),
            offset: 0,
            type_symbol: element_descriptor.storage_symbol(),
            type_name: "".into(),
            type_descriptor: element_descriptor.clone(),
            layout: element_layout,
        };
        let cursor = NestedFieldLayoutCursor::from_root(&root_field);
        let cursor = resolve_indexed_target_suffix_cursor_in_table(
            &input.layouts,
            cursor,
            expressions,
            indexed.suffix_root,
            indexed.boundary,
        )?;
        Some(descriptor_is_fat_slice(cursor.type_descriptor()))
    })()
    .unwrap_or(false)
}

pub(super) fn resolve_runtime_frame_indexed_target_near_slot_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    preferred_descriptor_offset: usize,
) -> Option<RuntimeFrameIndexedTarget> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection_path = normalized_storage_name_path_in_table(expressions, indexed.collection)?;
    let collection_slot = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && slot.byte_offset == preferred_descriptor_offset
                && slot_matches_table_path(slot, &collection_path))
            .then_some(slot)
        })?;
    if runtime_frame_slot_is_inline_fixed_array_storage(input, collection_slot) {
        return None;
    }

    let index_place = resolve_runtime_storage_place_near_frame_offset_in_table(
        input,
        dispatch_index,
        expressions,
        indexed.index,
        collection_slot.byte_offset,
    )
    .or_else(|| {
        resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            collection_slot.source_key,
            expressions,
            indexed.index,
        )
    })?;
    if index_place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }

    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout, field_descriptor) =
        resolve_indexed_target_suffix_layout_in_table(
            input,
            &root_field,
            expressions,
            indexed.suffix_root,
            indexed.boundary,
        )?;

    Some(RuntimeFrameIndexedTarget {
        descriptor_offset: collection_slot.byte_offset,
        index_region: index_place.region,
        index_offset: index_place.byte_offset,
        index_byte_size: index_place.byte_count,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
        is_bounded_byte_buffer: descriptor_is_bounded_byte_buffer(&field_descriptor),
    })
}

fn resolve_runtime_storage_place_near_frame_offset_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    minimum_byte_offset: usize,
) -> Option<RuntimeStoragePlace> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.is_empty() {
        return None;
    }
    let slot = input
        .runtime_storage
        .frame_slots
        .iter()
        .filter_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && slot.byte_offset >= minimum_byte_offset
                && slot_matches_table_path(slot, &path))
            .then_some(slot)
        })
        .min_by_key(|slot| slot.byte_offset)?;

    let suffix = path.suffix(1);
    if let Some(place) = runtime_slice_descriptor_member_place(
        input,
        slot.byte_offset,
        slot.byte_size,
        suffix.iter().next().map(|(name, _, _, _)| name.as_str()),
        suffix.iter().count(),
    ) {
        return Some(place);
    }

    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: slot.byte_offset,
        type_symbol: slot.type_symbol,
        type_name: slot.type_name.clone(),
        type_descriptor: slot.type_descriptor.clone(),
        layout: TypeLayout {
            size: slot.byte_size,
            alignment: slot.alignment,
        },
    };
    let (byte_offset, layout) =
        resolve_nested_field_layout_with_pairs(&input.layouts, &root_field, suffix.iter())?;

    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset,
        byte_count: layout.size,
    })
}

pub(super) fn resolve_runtime_frame_base_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameBaseIndexedTarget> {
    let target = resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    (target.index_region == RuntimeStorageRegion::RuntimeFrame).then_some(target)
}

pub(super) fn resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameBaseIndexedTarget> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    // The slot match is by HEAD symbol, so a MEMBER-of-slot collection
    // (`container.items[k]` -- an array FIELD of a by-value struct param)
    // returns the WHOLE struct slot. Walk the member suffix to the array
    // field's prefix offset + element descriptor; a direct array slot
    // (`arr[k]`) walks zero steps and keeps prefix 0.
    let member_element_descriptor: TypeLayoutDescriptor;
    let (array_prefix_offset, element_descriptor, element_stride) =
        if let Some(element) = inline_fixed_array_element_type(&collection_slot.type_descriptor) {
            (0usize, element, None)
        } else {
            let path = normalized_storage_name_path_in_table(expressions, indexed.collection)?;
            if path.len() <= 1 {
                return None;
            }
            let struct_root = FieldLayout {
                symbol: collection_slot.symbol,
                name: collection_slot.name.clone(),
                offset: 0,
                type_symbol: collection_slot.type_descriptor.storage_symbol(),
                type_name: "".into(),
                type_descriptor: collection_slot.type_descriptor.clone(),
                layout: TypeLayout {
                    size: collection_slot.byte_size,
                    alignment: collection_slot.alignment,
                },
            };
            let mut cursor = NestedFieldLayoutCursor::from_root(&struct_root);
            for (field_name, field_symbol, field_index, case_variant) in path.suffix(1).iter() {
                cursor = resolve_nested_field_layout_step(
                    &input.layouts,
                    cursor,
                    field_name,
                    field_symbol,
                    field_index,
                    case_variant,
                )?;
            }
            let prefix_offset = cursor.byte_offset();
            member_element_descriptor =
                inline_fixed_array_element_type(cursor.type_descriptor())?.clone();
            (
                prefix_offset,
                &member_element_descriptor,
                cursor.repeated_element_stride(),
            )
        };
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout, field_descriptor) =
        resolve_indexed_target_suffix_layout_in_table(
            input,
            &root_field,
            expressions,
            indexed.suffix_root,
            indexed.boundary,
        )?;

    Some(RuntimeFrameBaseIndexedTarget {
        base_byte_offset: collection_slot.byte_offset + array_prefix_offset,
        index_region: index_place.region,
        index_offset: index_place.byte_offset,
        index_byte_size: index_place.byte_count,
        element_byte_size: element_stride.unwrap_or(element_layout.size),
        field_byte_offset,
        byte_count: field_layout.size,
        is_bounded_byte_buffer: descriptor_is_bounded_byte_buffer(&field_descriptor),
    })
}

/// The BOTH-RUNTIME nested element of a FRAME-resident inline 2D array
/// (`g[i][j]`, `g` a by-value param or local `[[T; C]; R]`): outer = the ROW
/// index (stride = row byte size), inner = the ELEMENT index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeFrameBaseDoubleIndexedTarget {
    pub(super) base_byte_offset: usize,
    pub(super) outer_index_region: RuntimeStorageRegion,
    pub(super) outer_index_offset: usize,
    pub(super) outer_index_byte_size: usize,
    pub(super) outer_stride: usize,
    pub(super) inner_index_region: RuntimeStorageRegion,
    pub(super) inner_index_offset: usize,
    pub(super) inner_index_byte_size: usize,
    pub(super) inner_stride: usize,
    pub(super) field_byte_offset: usize,
    pub(super) byte_count: usize,
    pub(super) is_bounded_byte_buffer: bool,
}

/// Resolve `g[i][j]` -- a frame-resident 2D fixed array read with BOTH
/// indices runtime. A member suffix above the element is folded into the
/// fixed field offset. `None` for every other shape: single runtime level (the
/// frame single-index resolver), member links between the index levels,
/// machine collections (the machine double resolver). The region-aware form
/// retains frame- or machine-held indices; the compatibility wrapper below
/// admits only the historical all-frame subset for operation families whose
/// encoders have not opted into mixed roots.
pub(super) fn resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameBaseDoubleIndexedTarget> {
    let mut outer = expression;
    loop {
        match expressions.expression(outer) {
            ExpressionNode::Mutable(next) => outer = *next,
            ExpressionNode::Member(member) => outer = member.receiver,
            _ => break,
        }
    }
    let ExpressionNode::Indexed(outer_indexed) = expressions.expression(outer) else {
        return None;
    };
    let mut inner = outer_indexed.collection;
    while let ExpressionNode::Mutable(next) = expressions.expression(inner) {
        inner = *next;
    }
    let ExpressionNode::Indexed(inner_indexed) = expressions.expression(inner) else {
        return None;
    };
    if indexed_index_is_const(expressions, outer_indexed.index)
        || indexed_index_is_const(expressions, inner_indexed.index)
    {
        return None;
    }
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        inner_indexed.collection,
    )?;
    let row_type = inline_fixed_array_element_type(&collection_slot.type_descriptor)?;
    let row_layout = descriptor_layout(input, row_type);
    let element_type = inline_fixed_array_element_type(row_type)?;
    let element_layout = descriptor_layout(input, element_type);
    let element_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: element_type.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_type.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, leaf_layout, leaf_descriptor) =
        resolve_indexed_target_suffix_layout_in_table(
            input,
            &element_field,
            expressions,
            expression,
            outer,
        )?;

    let outer_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        inner_indexed.index,
    )?;
    let inner_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        outer_indexed.index,
    )?;
    Some(RuntimeFrameBaseDoubleIndexedTarget {
        base_byte_offset: collection_slot.byte_offset,
        outer_index_region: outer_place.region,
        outer_index_offset: outer_place.byte_offset,
        outer_index_byte_size: outer_place.byte_count,
        outer_stride: row_layout.size,
        inner_index_region: inner_place.region,
        inner_index_offset: inner_place.byte_offset,
        inner_index_byte_size: inner_place.byte_count,
        inner_stride: element_layout.size,
        field_byte_offset,
        byte_count: leaf_layout.size,
        is_bounded_byte_buffer: descriptor_is_bounded_byte_buffer(&leaf_descriptor),
    })
}

pub(super) fn resolve_runtime_frame_base_double_indexed_source_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameBaseDoubleIndexedTarget> {
    let target = resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    if target.outer_index_region != RuntimeStorageRegion::RuntimeFrame
        || target.inner_index_region != RuntimeStorageRegion::RuntimeFrame
    {
        return None;
    }
    Some(target)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeMachineIndexedTarget {
    pub(super) base_byte_offset: usize,
    pub(super) index_region: RuntimeStorageRegion,
    pub(super) index_offset: usize,
    pub(super) index_byte_size: usize,
    pub(super) element_byte_size: usize,
    pub(super) field_byte_offset: usize,
    pub(super) byte_count: usize,
    pub(super) is_bounded_byte_buffer: bool,
}

/// The BOTH-RUNTIME nested element `grid[i][j]`: outer = the ROW index (`i`,
/// stride = row byte size), inner = the ELEMENT index (`j`, stride = element
/// byte size). NOTE the tree/op naming flip: `i` is the tree-INNER `Indexed`
/// node's index but the op's OUTER (row) index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeMachineDoubleIndexedTarget {
    pub(super) base_byte_offset: usize,
    pub(super) outer_index_region: RuntimeStorageRegion,
    pub(super) outer_index_offset: usize,
    pub(super) outer_index_byte_size: usize,
    pub(super) outer_stride: usize,
    pub(super) inner_index_region: RuntimeStorageRegion,
    pub(super) inner_index_offset: usize,
    pub(super) inner_index_byte_size: usize,
    pub(super) inner_stride: usize,
    pub(super) field_byte_offset: usize,
    pub(super) byte_count: usize,
    pub(super) is_bounded_byte_buffer: bool,
}

/// A BOTH-RUNTIME nested element below a frame-held reference/recast pointer.
/// The pointer slot remains distinct from each index slot; the outer stride may
/// come from a validated plan-laid repeated field while the inner stride stays
/// compiler-derived from the recursively fixed element shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimePointeeDoubleIndexedTarget {
    pub(super) descriptor_offset: usize,
    pub(super) outer_index_region: RuntimeStorageRegion,
    pub(super) outer_index_offset: usize,
    pub(super) outer_index_byte_size: usize,
    pub(super) outer_stride: usize,
    pub(super) inner_index_region: RuntimeStorageRegion,
    pub(super) inner_index_offset: usize,
    pub(super) inner_index_byte_size: usize,
    pub(super) inner_stride: usize,
    pub(super) field_byte_offset: usize,
    pub(super) byte_count: usize,
    pub(super) is_bounded_byte_buffer: bool,
}

impl RuntimePointeeDoubleIndexedTarget {
    pub(super) fn place(self) -> Option<Place> {
        Place::at(RuntimeStorageRegion::RuntimeFrame, self.descriptor_offset)
            .with_step(PlaceStep::Deref)?
            .with_step(PlaceStep::ConstOffset(self.field_byte_offset))?
            .with_step(PlaceStep::ScaledIndex {
                index_region: self.outer_index_region,
                index_offset: self.outer_index_offset,
                index_byte_size: self.outer_index_byte_size,
                element_byte_size: self.outer_stride,
            })?
            .with_step(PlaceStep::ScaledIndex {
                index_region: self.inner_index_region,
                index_offset: self.inner_index_offset,
                index_byte_size: self.inner_index_byte_size,
                element_byte_size: self.inner_stride,
            })
    }
}

/// Resolve `view.rows[i][j]` through a frame-held reference or recast pointer.
/// Both indices must be runtime and independently resolve to exact frame or
/// machine storage. Constant/runtime mixtures remain owned by the existing
/// single-index resolver, and a third runtime level refuses rather than
/// truncating the address path.
pub(super) fn resolve_runtime_pointee_double_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimePointeeDoubleIndexedTarget> {
    let mut outer = expression;
    loop {
        match expressions.expression(outer) {
            ExpressionNode::Mutable(next) => outer = *next,
            ExpressionNode::Member(member) => outer = member.receiver,
            _ => break,
        }
    }
    let ExpressionNode::Indexed(outer_indexed) = expressions.expression(outer) else {
        return None;
    };

    let mut between_members: Vec<&psi_checked_trees::expression::TableMemberExpression> =
        Vec::new();
    let mut inner = outer_indexed.collection;
    loop {
        match expressions.expression(inner) {
            ExpressionNode::Mutable(next) => inner = *next,
            ExpressionNode::Member(member) => {
                between_members.push(member);
                inner = member.receiver;
            }
            _ => break,
        }
    }
    between_members.reverse();
    let ExpressionNode::Indexed(inner_indexed) = expressions.expression(inner) else {
        return None;
    };
    if indexed_index_is_const(expressions, outer_indexed.index)
        || indexed_index_is_const(expressions, inner_indexed.index)
    {
        return None;
    }

    let collection_path =
        normalized_storage_name_path_in_table(expressions, inner_indexed.collection)?;
    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        inner_indexed.collection,
    )?;
    let pointee_descriptor = slot.type_descriptor.reference_referee()?;
    let pointee_layout = descriptor_layout(input, pointee_descriptor);
    let wide_referee_slot = matches!(pointee_descriptor, TypeLayoutDescriptor::Named { .. })
        && pointee_layout.size > input.runtime_abi.pointer_size
        && slot.byte_size == pointee_layout.size;
    if slot.byte_size != input.runtime_abi.pointer_size && !wide_referee_slot {
        return None;
    }
    let shared_small_content_spill = matches!(
        &slot.type_descriptor,
        TypeLayoutDescriptor::Reference {
            is_mutable: false,
            ..
        }
    ) && matches!(
        pointee_descriptor,
        TypeLayoutDescriptor::Named { .. }
    ) && pointee_layout.size <= input.runtime_abi.pointer_size
        && !input.program.machines().iter().any(|machine| {
            machine.symbol == source_key.machine && machine.supply_mode.is_boundary_declaration()
        });
    if shared_small_content_spill {
        return None;
    }

    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: 0,
        type_symbol: pointee_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: pointee_descriptor.clone(),
        layout: pointee_layout,
    };
    let mut collection_cursor = NestedFieldLayoutCursor::from_root(&root_field);
    for (field_name, field_symbol, field_index, case_variant) in collection_path.suffix(1).iter() {
        collection_cursor = resolve_nested_field_layout_step(
            &input.layouts,
            collection_cursor,
            field_name,
            field_symbol,
            field_index,
            case_variant,
        )?;
    }
    if descriptor_is_bounded_byte_buffer(collection_cursor.type_descriptor()) {
        return None;
    }
    let (row_type, _) = collection_cursor.type_descriptor().fixed_array()?;
    let row_layout = descriptor_layout(input, row_type);
    let row_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: row_type.storage_symbol(),
        type_name: "".into(),
        type_descriptor: row_type.clone(),
        layout: row_layout,
    };
    let mut inner_array_cursor = NestedFieldLayoutCursor::from_root(&row_field);
    for member in &between_members {
        inner_array_cursor = resolve_nested_field_layout_step(
            &input.layouts,
            inner_array_cursor,
            &member.member,
            member.member_symbol,
            None,
            member.case_variant.as_ref(),
        )?;
    }
    if descriptor_is_bounded_byte_buffer(inner_array_cursor.type_descriptor()) {
        return None;
    }
    let (element_type, _) = inner_array_cursor.type_descriptor().fixed_array()?;
    let element_layout = descriptor_layout(input, element_type);
    let element_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: element_type.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_type.clone(),
        layout: element_layout,
    };
    let (suffix_offset, leaf_layout, leaf_descriptor) =
        resolve_indexed_target_suffix_layout_in_table(
            input,
            &element_field,
            expressions,
            expression,
            outer,
        )?;

    let outer_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        inner_indexed.index,
    )?;
    let inner_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        outer_indexed.index,
    )?;
    for place in [&outer_place, &inner_place] {
        if !matches!(
            place.region,
            RuntimeStorageRegion::RuntimeFrame | RuntimeStorageRegion::Machine
        ) {
            return None;
        }
    }

    Some(RuntimePointeeDoubleIndexedTarget {
        descriptor_offset: slot.byte_offset,
        outer_index_region: outer_place.region,
        outer_index_offset: outer_place.byte_offset,
        outer_index_byte_size: outer_place.byte_count,
        outer_stride: collection_cursor
            .repeated_element_stride()
            .unwrap_or(row_layout.size),
        inner_index_region: inner_place.region,
        inner_index_offset: inner_place.byte_offset,
        inner_index_byte_size: inner_place.byte_count,
        inner_stride: inner_array_cursor
            .repeated_element_stride()
            .unwrap_or(element_layout.size),
        field_byte_offset: collection_cursor
            .byte_offset()
            .checked_add(inner_array_cursor.byte_offset())?
            .checked_add(suffix_offset)?,
        byte_count: leaf_layout.size,
        is_bounded_byte_buffer: descriptor_is_bounded_byte_buffer(&leaf_descriptor),
    })
}

pub(super) fn resolve_runtime_pointee_double_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimePointeeDoubleIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_pointee_double_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

/// Resolve `grid[i][j]` -- a machine-owned 2D fixed array read with BOTH
/// indices runtime -- to its double-indexed place. `None` for every other
/// shape: a single runtime level (the single-index resolver), 3+ runtime
/// levels (no op carries three indices), a frame-resident collection (the
/// shadowing gate -- frame arrays use the frame op family), a carrier, or a
/// member suffix above the element (not wired yet; the fence keeps those
/// shapes rejected).
pub(super) fn resolve_runtime_machine_double_indexed_source_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeMachineDoubleIndexedTarget> {
    // Peel a member SUFFIX above the element (`boards[i][j].x`): the suffix
    // walk (bounded by the outer Indexed node) folds it into
    // field_byte_offset after the strides resolve.
    let mut outer = expression;
    loop {
        match expressions.expression(outer) {
            ExpressionNode::Mutable(next) => outer = *next,
            ExpressionNode::Member(member) => outer = member.receiver,
            _ => break,
        }
    }
    let ExpressionNode::Indexed(outer_indexed) = expressions.expression(outer) else {
        return None;
    };
    // A member chain BETWEEN the two indices (`rows[i].data[j]` -- `data` is
    // a field of the row element) contributes a fixed offset: the address is
    // base + i*row_stride + data_off + j*elem_stride, and addition commutes,
    // so it rides the op's field_byte_offset. Collect the chain (outermost
    // receiver first for the descent below).
    let mut between_members: Vec<&psi_checked_trees::expression::TableMemberExpression> =
        Vec::new();
    let mut inner = outer_indexed.collection;
    loop {
        match expressions.expression(inner) {
            ExpressionNode::Mutable(next) => inner = *next,
            ExpressionNode::Member(member) => {
                between_members.push(member);
                inner = member.receiver;
            }
            _ => break,
        }
    }
    between_members.reverse();
    let ExpressionNode::Indexed(inner_indexed) = expressions.expression(inner) else {
        return None;
    };
    // BOTH indices must be runtime -- a const level belongs to the
    // single-index paths (const levels fold into the collection resolution
    // or ride the suffix walk).
    if indexed_index_is_const(expressions, outer_indexed.index)
        || indexed_index_is_const(expressions, inner_indexed.index)
    {
        return None;
    }
    // The row collection must be a plain machine-owned place (possibly behind
    // CONST-indexed layers, which the const-prefix peel folds); a THIRD
    // runtime level makes the peel fail and the resolve correctly refuses.
    if runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        inner_indexed.collection,
    )
    .is_some()
    {
        return None;
    }
    let collection = resolve_machine_owned_collection_with_const_prefix_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        inner_indexed.collection,
    )?;
    if descriptor_is_bounded_byte_buffer(&collection.type_descriptor) {
        return None;
    }
    let (row_type, _rows) = collection.type_descriptor.fixed_array()?;
    let row_layout = descriptor_layout(input, row_type);

    // Descend the ROW element through the between-members chain to the INNER
    // array (`Row` --.data--> `[i32; 2]`), accumulating the fixed offset.
    let row_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: row_type.storage_symbol(),
        type_name: "".into(),
        type_descriptor: row_type.clone(),
        layout: row_layout,
    };
    let mut cursor = NestedFieldLayoutCursor::from_root(&row_field);
    for member in &between_members {
        cursor = resolve_nested_field_layout_step(
            &input.layouts,
            cursor,
            &member.member,
            member.member_symbol,
            None,
            member.case_variant.as_ref(),
        )?;
    }
    let between_offset = cursor.byte_offset();
    let inner_array_type = cursor.type_descriptor().clone();
    if descriptor_is_bounded_byte_buffer(&inner_array_type) {
        return None;
    }
    let (element_type, _columns) = inner_array_type.fixed_array()?;
    let element_layout = descriptor_layout(input, element_type);

    // A member SUFFIX above the element folds in via the boundary walk
    // (boundary = the outer Indexed node; for a bare `grid[i][j]` the walk
    // early-returns with offset 0 and the element layout).
    let element_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: element_type.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_type.clone(),
        layout: element_layout,
    };
    let (suffix_offset, leaf_layout, leaf_descriptor) =
        resolve_indexed_target_suffix_layout_in_table(
            input,
            &element_field,
            expressions,
            expression,
            outer,
        )?;

    let outer_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        inner_indexed.index,
    )?;
    let inner_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        outer_indexed.index,
    )?;
    for place in [&outer_place, &inner_place] {
        if !matches!(
            place.region,
            RuntimeStorageRegion::RuntimeFrame | RuntimeStorageRegion::Machine
        ) {
            return None;
        }
    }

    Some(RuntimeMachineDoubleIndexedTarget {
        base_byte_offset: collection.byte_offset,
        outer_index_region: outer_place.region,
        outer_index_offset: outer_place.byte_offset,
        outer_index_byte_size: outer_place.byte_count,
        outer_stride: collection.element_stride.unwrap_or(row_layout.size),
        inner_index_region: inner_place.region,
        inner_index_offset: inner_place.byte_offset,
        inner_index_byte_size: inner_place.byte_count,
        inner_stride: cursor
            .repeated_element_stride()
            .unwrap_or(element_layout.size),
        field_byte_offset: between_offset + suffix_offset,
        byte_count: leaf_layout.size,
        is_bounded_byte_buffer: descriptor_is_bounded_byte_buffer(&leaf_descriptor),
    })
}

pub(super) fn resolve_runtime_machine_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeMachineIndexedTarget> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    // SHADOWING gate: the machine-owned resolver matches the collection NAME
    // against the machine's fields, so a by-value PARAM/LOCAL array that
    // shadows a machine field (`pick(arr: [i32; 3])` beside `self.arr`) would
    // silently alias the machine's storage. A collection that resolves to a
    // FRAME place is frame-resident -- never machine-indexed.
    if runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )
    .is_some()
    {
        return None;
    }
    let collection = resolve_machine_owned_collection_with_const_prefix_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    if !matches!(
        index_place.region,
        RuntimeStorageRegion::RuntimeFrame | RuntimeStorageRegion::Machine
    ) {
        return None;
    }

    let element_descriptor = collection.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout, field_descriptor) =
        resolve_indexed_target_suffix_layout_in_table(
            input,
            &root_field,
            expressions,
            indexed.suffix_root,
            indexed.boundary,
        )?;

    // A carrier (`[u8;N]` BoundedByteBuffer) stores its inline bytes AFTER a
    // leading `len` word, so a runtime index must address past that prefix --
    // the same adjustment the constant-index path
    // (`bounded_byte_buffer_indexed_place`) makes. A plain inline array has no
    // prefix.
    let carrier_byte_prefix = if descriptor_is_bounded_byte_buffer(&collection.type_descriptor) {
        input.runtime_abi.pointer_size
    } else {
        0
    };

    Some(RuntimeMachineIndexedTarget {
        base_byte_offset: collection.byte_offset + carrier_byte_prefix,
        index_region: index_place.region,
        index_offset: index_place.byte_offset,
        index_byte_size: index_place.byte_count,
        element_byte_size: collection.element_stride.unwrap_or(element_layout.size),
        field_byte_offset,
        byte_count: field_layout.size,
        is_bounded_byte_buffer: descriptor_is_bounded_byte_buffer(&field_descriptor),
    })
}

pub(super) fn resolve_runtime_machine_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeMachineIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

/// Resolve a machine-owned COLLECTION place, peeling CONST-`Indexed` layers the
/// name-path resolution cannot carry. `normalized_storage_name_path_in_table`
/// holds ONE root element index per member, so `cube[1]` resolves directly but
/// `cube[1][1]` (the collection of `cube[1][1][k]`) does not: peel the outer
/// const `[1]`, resolve `cube[1]` (recursively -- any depth), then descend the
/// element type, biasing the base by `index * element_size`. The direct
/// resolution is always tried FIRST, so single-index and member-suffix shapes
/// (`rows[2].data`) keep their existing path.
fn resolve_machine_owned_collection_with_const_prefix_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<MachineOwnedCollectionTarget> {
    if let Some(collection) = resolve_machine_owned_collection_in_table(
        &input.layouts,
        input,
        dispatch_index,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        expression,
    ) {
        return Some(collection);
    }

    let mut peeled = expression;
    while let ExpressionNode::Mutable(inner) = expressions.expression(peeled) {
        peeled = *inner;
    }
    let ExpressionNode::Indexed(inner) = expressions.expression(peeled) else {
        return None;
    };
    let mut index = inner.index;
    while let ExpressionNode::Mutable(next) = expressions.expression(index) {
        index = *next;
    }
    let ExpressionNode::Integer(index) = expressions.expression(index) else {
        return None;
    };
    let index = usize::try_from(index.value_i64()?).ok()?;

    let base = resolve_machine_owned_collection_with_const_prefix_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        inner.collection,
    )?;
    let (element_type, length) = base.type_descriptor.fixed_array()?;
    if index >= length {
        return None;
    }
    let element_layout = descriptor_layout(input, element_type);
    Some(MachineOwnedCollectionTarget {
        byte_offset: base.byte_offset + index * base.element_stride.unwrap_or(element_layout.size),
        type_descriptor: element_type.clone(),
        element_stride: None,
    })
}

pub(super) fn resolve_runtime_machine_double_indexed_source(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeMachineDoubleIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_machine_double_indexed_source_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

pub(super) fn resolve_runtime_frame_base_double_indexed_source(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeFrameBaseDoubleIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_frame_base_double_indexed_source_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

pub(super) fn resolve_runtime_frame_base_double_indexed_source_with_index_regions(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeFrameBaseDoubleIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

pub(super) fn resolve_runtime_pointee_slot_offset(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimePointeeTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

pub(super) fn resolve_runtime_pointee_slot_offset_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimePointeeTarget> {
    let Some(path) = normalized_storage_name_path_in_table(expressions, expression) else {
        return resolve_runtime_pointee_stacked_fixed_path_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            expression,
        );
    };
    if path.is_empty() {
        return None;
    }
    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    // A pointer lives either in a pointer-sized slot (params, `&mut` locals)
    // or at the START of a referee-sized slot: the borrow-recast let
    // (`let d = &self.map_buf[k] as &Descriptor`) sizes its slot by the STATED
    // referee, but a referee wider than a pointer cannot content-spill, so
    // the lowering stores the ELEMENT ADDRESS in the slot's first pointer
    // width (WriteRuntimeMachineIndexedAddressToRuntimeFrame) and reads must
    // deref it.
    let recast_referee_size = slot
        .type_descriptor
        .reference_referee()
        .filter(|descriptor| {
            // Only NAMED-record referees: a slice/trait referee's slot is a
            // {ptr, len} DESCRIPTOR (also wider than a pointer), not an
            // element address -- treating it as one broke every text read.
            matches!(descriptor, omega_layout::TypeLayoutDescriptor::Named { .. })
        })
        .map(|descriptor| descriptor_layout(input, descriptor).size);
    let wide_referee_slot = recast_referee_size
        .is_some_and(|size| size > input.runtime_abi.pointer_size && slot.byte_size == size);
    if slot.byte_size != input.runtime_abi.pointer_size && !wide_referee_slot {
        return None;
    }
    // Deref-vs-flat for a SHARED `&Named` reference param. Two conventions
    // must both hold, and the distinguisher is the REFEREE SIZE, not
    // boundary-ness (bug 2026-07-12):
    //   * A referee that FITS in a pointer-sized slot (<= pointer_size) may be
    //     spilled by CONTENT into the param slot -- `read(&self.inner)` with an
    //     8-byte Inner passes the struct bytes, and flat member reads are
    //     correct (runtime_shared_ref_param_member canary, locked 2026-07-04).
    //     In a non-boundary machine this is the content-spill case: do NOT
    //     dereference (a field value read as an address segfaults).
    //   * A referee LARGER than a pointer cannot be content-spilled into an
    //     8-byte slot, so the param necessarily holds a REAL pointer and a
    //     field read MUST dereference -- `let bs = table.boot_services` with
    //     `table: &EfiSystemTable` (a large firmware struct) in the
    //     non-boundary `own_machine`. The earlier boundary-only gate got this
    //     wrong: it read `table_slot + field_offset` inline and Cathedral's M2
    //     boot dispatched get_memory_map through garbage and #UD'd.
    // A BOUNDARY machine's `&Struct` param is a genuine hand-off pointer even
    // when small, so it always dereferences. `&mut` slots were already
    // pointee-resolvable everywhere; slice/carrier referees are {ptr,len}
    // descriptors (not Named) and are unaffected.
    let slot_is_shared_reference = matches!(
        &slot.type_descriptor,
        omega_layout::TypeLayoutDescriptor::Reference {
            is_mutable: false,
            ..
        }
    );
    let pointee_descriptor = slot.type_descriptor.reference_referee()?;
    let pointee_layout = descriptor_layout(input, pointee_descriptor);
    if slot_is_shared_reference
        && matches!(
            pointee_descriptor,
            omega_layout::TypeLayoutDescriptor::Named { .. }
        )
        && pointee_layout.size <= input.runtime_abi.pointer_size
        && !input.program.machines().iter().any(|machine| {
            machine.symbol == source_key.machine && machine.supply_mode.is_boundary_declaration()
        })
    {
        return None;
    }
    let suffix = path.suffix(1);
    let (field_byte_offset, field_layout) = if path.len() <= 1 {
        (0, pointee_layout)
    } else {
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: 0,
            type_symbol: pointee_descriptor.storage_symbol(),
            type_name: "".into(),
            type_descriptor: pointee_descriptor.clone(),
            layout: pointee_layout,
        };
        resolve_nested_field_layout_with_pairs(&input.layouts, &root_field, suffix.iter())?
    };
    (field_layout.size > 0).then_some(RuntimePointeeTarget {
        pointer_byte_offset: slot.byte_offset,
        field_byte_offset,
        pointee_byte_size: field_layout.size,
    })
}

/// Resolve a reference-backed field path containing stacked constant array
/// indexes (`view.record.matrix[1][0]`). `StorageNamePath` deliberately refuses
/// stacked indices because one path member can retain only one index. Walk the
/// checked expression tree instead, starting from the exact reference slot, so
/// every fixed-array level contributes its own validated stride.
fn resolve_runtime_pointee_stacked_fixed_path_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimePointeeTarget> {
    let root = pointee_path_root_in_table(expressions, expression)?;
    let root_path = normalized_storage_name_path_in_table(expressions, root)?;
    if root_path.len() != 1 {
        return None;
    }
    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        root,
    )?;
    let pointee_descriptor = slot.type_descriptor.reference_referee()?;
    let pointee_layout = descriptor_layout(input, pointee_descriptor);
    let recast_referee_size = matches!(pointee_descriptor, TypeLayoutDescriptor::Named { .. })
        .then_some(pointee_layout.size);
    let wide_referee_slot = recast_referee_size
        .is_some_and(|size| size > input.runtime_abi.pointer_size && slot.byte_size == size);
    if slot.byte_size != input.runtime_abi.pointer_size && !wide_referee_slot {
        return None;
    }
    let shared = matches!(
        &slot.type_descriptor,
        TypeLayoutDescriptor::Reference {
            is_mutable: false,
            ..
        }
    );
    if shared
        && matches!(pointee_descriptor, TypeLayoutDescriptor::Named { .. })
        && pointee_layout.size <= input.runtime_abi.pointer_size
        && !input.program.machines().iter().any(|machine| {
            machine.symbol == source_key.machine && machine.supply_mode.is_boundary_declaration()
        })
    {
        return None;
    }

    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: 0,
        type_symbol: pointee_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: pointee_descriptor.clone(),
        layout: pointee_layout,
    };
    let cursor = resolve_stacked_fixed_path_cursor_in_table(
        &input.layouts,
        NestedFieldLayoutCursor::from_root(&root_field),
        expressions,
        expression,
        root,
    )?;
    (cursor.layout().size > 0).then_some(RuntimePointeeTarget {
        pointer_byte_offset: slot.byte_offset,
        field_byte_offset: cursor.byte_offset(),
        pointee_byte_size: cursor.layout().size,
    })
}

fn pointee_path_root_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(inner) => pointee_path_root_in_table(expressions, *inner),
        ExpressionNode::Indexed(indexed) => {
            pointee_path_root_in_table(expressions, indexed.collection)
        }
        ExpressionNode::Member(member) => pointee_path_root_in_table(expressions, member.receiver),
        ExpressionNode::Name(_) => Some(expression),
        _ => None,
    }
}

fn resolve_stacked_fixed_path_cursor_in_table<'layout>(
    layouts: &'layout omega_layout::LayoutPlan,
    cursor: NestedFieldLayoutCursor<'layout>,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    root: ExpressionHandle,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    if expression == root {
        return Some(cursor);
    }
    match expressions.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            resolve_stacked_fixed_path_cursor_in_table(layouts, cursor, expressions, *inner, root)
        }
        ExpressionNode::Member(member) => {
            let cursor = resolve_stacked_fixed_path_cursor_in_table(
                layouts,
                cursor,
                expressions,
                member.receiver,
                root,
            )?;
            resolve_nested_field_layout_step(
                layouts,
                cursor,
                &member.member,
                member.member_symbol,
                None,
                member.case_variant.as_ref(),
            )
        }
        ExpressionNode::Indexed(indexed) => {
            let cursor = resolve_stacked_fixed_path_cursor_in_table(
                layouts,
                cursor,
                expressions,
                indexed.collection,
                root,
            )?;
            let ExpressionNode::Integer(index) = expressions.expression(indexed.index) else {
                return None;
            };
            apply_fixed_array_index_to_cursor(cursor, usize::try_from(index.value_i64()?).ok()?)
        }
        _ => None,
    }
}

/// `IntegerAt` sibling of the ordinary pointee resolver. This deliberately
/// walks with the dedicated stored-integer cursor; the ordinary resolver keeps
/// rejecting the same field so no byte-copy consumer can bypass extension.
fn resolve_runtime_pointee_stored_integer_projection_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoredIntegerProjection> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.len() <= 1 {
        return None;
    }
    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    let recast_referee_size = slot
        .type_descriptor
        .reference_referee()
        .filter(|descriptor| matches!(descriptor, TypeLayoutDescriptor::Named { .. }))
        .map(|descriptor| descriptor_layout(input, descriptor).size);
    let wide_referee_slot = recast_referee_size
        .is_some_and(|size| size > input.runtime_abi.pointer_size && slot.byte_size == size);
    if slot.byte_size != input.runtime_abi.pointer_size && !wide_referee_slot {
        return None;
    }
    let slot_is_shared_reference = matches!(
        &slot.type_descriptor,
        TypeLayoutDescriptor::Reference {
            is_mutable: false,
            ..
        }
    );
    let pointee_descriptor = slot.type_descriptor.reference_referee()?;
    let pointee_layout = descriptor_layout(input, pointee_descriptor);
    if slot_is_shared_reference
        && matches!(pointee_descriptor, TypeLayoutDescriptor::Named { .. })
        && pointee_layout.size <= input.runtime_abi.pointer_size
        && !input.program.machines().iter().any(|machine| {
            machine.symbol == source_key.machine && machine.supply_mode.is_boundary_declaration()
        })
    {
        return None;
    }

    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: 0,
        type_symbol: pointee_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: pointee_descriptor.clone(),
        layout: pointee_layout,
    };
    let mut cursor = NestedFieldLayoutCursor::from_root(&root_field);
    for (field_name, field_symbol, field_index, case_variant) in path.suffix(1).iter() {
        cursor = resolve_nested_stored_integer_layout_step(
            &input.layouts,
            cursor,
            field_name,
            field_symbol,
            field_index,
            case_variant,
        )?;
    }
    let field_byte_offset = cursor.byte_offset();
    stored_integer_projection_from_cursor(
        cursor,
        RuntimeStoredIntegerSource::Pointee {
            pointer_byte_offset: slot.byte_offset,
            field_byte_offset,
        },
    )
}

pub(super) fn resolve_runtime_pointee_fixed_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimePointeeTarget> {
    let fixed = fixed_indexed_target_path_in_table(expressions, expression)?;
    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    if place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }

    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    let collection_descriptor = slot.type_descriptor.reference_referee()?;
    let element_descriptor = collection_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_index = usize::try_from(fixed.index).ok()?;
    let element_offset = element_index.checked_mul(element_layout.size)?;
    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout, _) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        fixed.suffix_root,
        fixed.boundary,
    )?;

    (field_layout.size > 0).then_some(RuntimePointeeTarget {
        pointer_byte_offset: pointee_pointer_offset(input, place)?,
        field_byte_offset: element_offset.checked_add(field_byte_offset)?,
        pointee_byte_size: field_layout.size,
    })
}

pub(super) fn resolve_runtime_pointee_fixed_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimePointeeTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_pointee_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

fn pointee_pointer_offset(
    input: &InstructionSelectionInput<'_>,
    place: RuntimeStoragePlace,
) -> Option<usize> {
    if place.byte_count == input.runtime_abi.pointer_size {
        return Some(place.byte_offset);
    }

    let descriptor = input.runtime_abi.slice_descriptor();
    if place.byte_count == descriptor.total_size() {
        return place.byte_offset.checked_add(descriptor.ptr_offset());
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimePointeeTarget {
    pub(super) pointer_byte_offset: usize,
    pub(super) field_byte_offset: usize,
    pub(super) pointee_byte_size: usize,
}

fn slot_matches_table_path(
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    path: &StorageNamePath<'_>,
) -> bool {
    let root_symbol = path.head_symbol();
    if root_symbol.is_valid() {
        // A resolved symbol is the authoritative identity. Falling back to
        // the spelling as an OR condition lets a same-named local in another
        // inline callee win the dispatch-wide fallback (for example the f64
        // classifier's `class` slot answering an f32 classifier guard).
        return slot_matches_root(slot.symbol, root_symbol);
    }
    path.member(0).is_some_and(|name| *name == slot.name)
}

fn slot_matches_root(slot_symbol: SymbolHandle, root_symbol: SymbolHandle) -> bool {
    slot_symbol.is_valid() && root_symbol.is_valid() && slot_symbol == root_symbol
}

/// Resolve a materialized local whose checked expression still carries the
/// source declaration symbol while runtime-storage planning has cloned and
/// re-symbolized the local. Identify the source declaration by symbol when it
/// survived; otherwise require one unique same-spelled declaration in the exact
/// source state. The planned slot must then have that declaration's statement
/// coordinate and spelling. This keeps same-named locals in other inline scopes
/// ineligible while allowing later arithmetic to read the slot that captured
/// this declaration's value.
fn resymbolized_local_slot_for_path<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    source_key: StateKey,
    path: &StorageNamePath<'_>,
) -> Option<&'plan omega_runtime_storage::RuntimeFrameSlot> {
    let source_symbol = path.head_symbol();
    if !source_symbol.is_valid() {
        return None;
    }
    let root_name = path.member(0)?;
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
    let statements = input
        .program
        .statement_table
        .statements(state.statement_nodes);
    let exact_declaration =
        statements
            .iter()
            .enumerate()
            .find_map(|(statement_index, statement)| {
                let psi_checked_trees::statement::StatementNode::LocalData(local) = statement
                else {
                    return None;
                };
                (local.symbol == source_symbol).then_some((statement_index, &local.name))
            });
    let (declaration_index, declaration_name) = exact_declaration.or_else(|| {
        let mut declarations =
            statements
                .iter()
                .enumerate()
                .filter_map(|(statement_index, statement)| {
                    let psi_checked_trees::statement::StatementNode::LocalData(local) = statement
                    else {
                        return None;
                    };
                    (local.name == *root_name).then_some((statement_index, &local.name))
                });
        let declaration = declarations.next()?;
        declarations.next().is_none().then_some(declaration)
    })?;
    if root_name != declaration_name {
        return None;
    }

    input
        .runtime_storage
        .frame_slots
        .iter()
        .filter_map(|(_, slot)| {
            (slot.dispatch_index <= dispatch_index
                && state_key_matches_statement_source(slot.source_key, source_key)
                && slot.statement_index == declaration_index
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                )
                && slot.name == *declaration_name)
                .then_some(slot)
        })
        .max_by_key(|slot| slot.dispatch_index)
}

/// Byte size of one element of the slice (or fixed array) named by `expression`,
/// from the resolved frame slot's element type. Used to scale subslice pointer
/// arithmetic on a runtime slice descriptor.
pub(super) fn resolve_slice_element_byte_size_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<usize> {
    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    let element_descriptor = slot.type_descriptor.element_type()?;
    Some(descriptor_layout(input, element_descriptor).size)
}

fn runtime_frame_slot_for_expression_in_table<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<&'plan omega_runtime_storage::RuntimeFrameSlot> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;

    find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
        slot_matches_table_path(slot, &path)
    })
    .or_else(|| {
        latest_dispatch_frame_slot(input, dispatch_index, |slot| {
            slot_matches_table_path(slot, &path)
        })
    })
    .or_else(|| resymbolized_local_slot_for_path(input, dispatch_index, source_key, &path))
}

/// GENUINELY SCOPED frame-slot resolution for a name-path expression under
/// `source_key`: only slots whose source matches the key (segment-
/// insensitive, per `state_key_matches_statement_source`) answer -- none of
/// the lenient name-only arms `find_runtime_frame_slot_for_path` ends in.
/// The leaf terminal-write uses this to test whether a resolution key can
/// answer for a bare-name terminal AT ALL before attempting it: under a key
/// that owns no slot for the name, the lenient fallbacks would match a
/// SAME-NAMED slot of another scope (the nested-inline result scramble,
/// 2026-07-11e/g).
pub(in crate::selection) fn runtime_frame_slot_for_expression_scoped<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<&'plan omega_runtime_storage::RuntimeFrameSlot> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;

    input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && state_key_matches_statement_source(slot.source_key, source_key)
                && slot_matches_table_path(slot, &path))
            .then_some(slot)
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .filter_map(|(_, slot)| {
                    (slot.dispatch_index <= dispatch_index
                        && state_key_matches_statement_source(slot.source_key, source_key)
                        && slot_matches_table_path(slot, &path))
                    .then_some(slot)
                })
                .max_by_key(|slot| slot.dispatch_index)
        })
}

/// The resolution key of the UNIQUE frame slot a MACHINE owns for a
/// name-path expression under `dispatch_index`. State symbols differ
/// between planning layers (a state-call's `target_key.state` is not the
/// control-flow state symbol slots record), so the leaf terminal-write's
/// CALL-TARGET scope matches by MACHINE and takes the slot's own exact
/// source key -- but ONLY when unique: a machine with several same-named
/// slots (one `b` per idx arm, account_ledger) cannot say which instance
/// answers, and contributes no key.
pub(in crate::selection) fn unique_machine_frame_slot_key_for_expression(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    machine: psi_symbols::SymbolHandle,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<StateKey> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    let mut matches = input
        .runtime_storage
        .frame_slots
        .iter()
        .filter(|(_, slot)| {
            slot.dispatch_index == dispatch_index
                && slot.source_key.machine == machine
                && slot_matches_table_path(slot, &path)
        })
        .map(|(_, slot)| slot.source_key);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn find_runtime_frame_slot_for_path<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    source_key: StateKey,
    matches_path: impl Fn(&omega_runtime_storage::RuntimeFrameSlot) -> bool,
) -> Option<&'plan omega_runtime_storage::RuntimeFrameSlot> {
    input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && slot.source_key == source_key
                && matches_path(slot))
            .then_some(slot)
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .find_map(|(_, slot)| {
                    (slot.dispatch_index == dispatch_index
                        && state_key_matches_statement_source(slot.source_key, source_key)
                        && matches_path(slot))
                    .then_some(slot)
                })
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .filter_map(|(_, slot)| {
                    (slot.dispatch_index <= dispatch_index
                        && slot.source_key == source_key
                        && matches_path(slot))
                    .then_some(slot)
                })
                .max_by_key(|slot| slot.dispatch_index)
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .filter_map(|(_, slot)| {
                    (slot.dispatch_index <= dispatch_index
                        && state_key_matches_statement_source(slot.source_key, source_key)
                        && matches_path(slot))
                    .then_some(slot)
                })
                .max_by_key(|slot| slot.dispatch_index)
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .find_map(|(_, slot)| {
                    (slot.source_key == source_key && matches_path(slot)).then_some(slot)
                })
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .find_map(|(_, slot)| {
                    (state_key_matches_statement_source(slot.source_key, source_key)
                        && matches_path(slot))
                    .then_some(slot)
                })
        })
        .or_else(|| {
            // Inline branch tables can lose the resolved symbol on a callee
            // local name. Before the outer-scope name-only fallback, accept a
            // same-machine slot only when it is unique in this dispatch. This
            // preserves the callee scope without guessing between two locals
            // of the same spelling in different states of one machine.
            let mut matches = input
                .runtime_storage
                .frame_slots
                .iter()
                .filter_map(|(_, slot)| {
                    (slot.dispatch_index == dispatch_index
                        && slot.source_key.machine == source_key.machine
                        && matches_path(slot))
                    .then_some(slot)
                });
            let first = matches.next();
            match (first, matches.next()) {
                (Some(slot), None) => Some(slot),
                _ => None,
            }
        })
        .or_else(|| {
            // Last resort: a slot owned by an OUTER call scope whose source_key
            // does not match the querying source. This happens when a caller's
            // `&mut` parameter is aliased into a callee arm AND the caller body
            // was split into dispatch segments by a dispatched (looping) call --
            // the arm's write resolves against the leaf source, which never
            // matches the caller-parameter slot under any source-scoped clause
            // above. The path match is symbol-specific for a resolved parameter
            // reference, so it identifies exactly that one slot; take the nearest
            // dispatch at or before this one (segments of one control state share
            // the slot).
            input
                .runtime_storage
                .frame_slots
                .iter()
                .filter_map(|(_, slot)| {
                    (slot.dispatch_index <= dispatch_index && matches_path(slot)).then_some(slot)
                })
                .max_by_key(|slot| slot.dispatch_index)
        })
}

fn latest_dispatch_frame_slot<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    matches_path: impl Fn(&omega_runtime_storage::RuntimeFrameSlot) -> bool,
) -> Option<&'plan omega_runtime_storage::RuntimeFrameSlot> {
    input
        .runtime_storage
        .frame_slots
        .iter()
        .fold(None, |matched_slot, (_, slot)| {
            if slot.dispatch_index == dispatch_index && matches_path(slot) {
                Some(slot)
            } else {
                matched_slot
            }
        })
}

fn resolve_runtime_fixed_indexed_place_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoragePlace> {
    let fixed = fixed_indexed_target_path_in_table(expressions, expression)?;
    let index = usize::try_from(fixed.index).ok()?;
    if let Some(slot) = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    ) {
        if let Some(place) = bounded_byte_buffer_indexed_place(
            input,
            &slot.type_descriptor,
            RuntimeStorageRegion::RuntimeFrame,
            slot.byte_offset,
            index,
            expressions,
            fixed.suffix_root,
            fixed.boundary,
        ) {
            return Some(place);
        }
        let element_descriptor = inline_fixed_array_element_type(&slot.type_descriptor)?;
        let element_layout = descriptor_layout(input, element_descriptor);
        let element_offset = index.checked_mul(element_layout.size)?;
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: 0,
            type_symbol: element_descriptor.storage_symbol(),
            type_name: "".into(),
            type_descriptor: element_descriptor.clone(),
            layout: element_layout,
        };
        let (field_byte_offset, field_layout, _) = resolve_indexed_target_suffix_layout_in_table(
            input,
            &root_field,
            expressions,
            fixed.suffix_root,
            fixed.boundary,
        )?;

        return Some(RuntimeStoragePlace {
            region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot
                .byte_offset
                .checked_add(element_offset)?
                .checked_add(field_byte_offset)?,
            byte_count: field_layout.size,
        });
    }

    let collection = match resolve_machine_owned_collection_in_table(
        &input.layouts,
        input,
        dispatch_index,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        fixed.collection,
    ) {
        Some(collection) => collection,
        // Elided-local collection (`let r = X.as_slice(); ... r[i]`): the local has
        // no frame slot, so trace it to the underlying machine array (see
        // resolve_elided_local_slice_view_array). This lets a slice-VIEW element
        // forwarded by value to a value-call (bound as a BranchParameter alias
        // `room = r[i]`, the lookup shape) reach the underlying machine array's
        // element instead of resolving to nothing. Only a bare single-name path
        // can be such a local.
        None => {
            let path = normalized_storage_name_path_in_table(expressions, fixed.collection)?;
            if path.len() != 1 {
                return None;
            }
            resolve_elided_local_slice_view_array(
                input,
                dispatch_index,
                source_key,
                path.head_symbol(),
                path.member(0)?,
            )?
        }
    };
    if let Some(place) = bounded_byte_buffer_indexed_place(
        input,
        &collection.type_descriptor,
        RuntimeStorageRegion::Machine,
        collection.byte_offset,
        index,
        expressions,
        fixed.suffix_root,
        fixed.boundary,
    ) {
        return Some(place);
    }
    let element_descriptor = inline_fixed_array_element_type(&collection.type_descriptor)?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_offset =
        index.checked_mul(collection.element_stride.unwrap_or(element_layout.size))?;
    let root_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout, _) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        fixed.suffix_root,
        fixed.boundary,
    )?;

    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::Machine,
        byte_offset: collection
            .byte_offset
            .checked_add(element_offset)?
            .checked_add(field_byte_offset)?,
        byte_count: field_layout.size,
    })
}

pub(super) fn resolve_runtime_frame_fixed_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameFixedIndexedTarget> {
    if let Some(target) = resolve_runtime_frame_fixed_indexed_storage_path_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(target);
    }

    let fixed = fixed_indexed_target_path_in_table(expressions, expression)?;
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    if runtime_frame_slot_is_inline_fixed_array_storage(input, collection_slot) {
        return None;
    }
    let descriptor_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    if descriptor_place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }

    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_index = usize::try_from(fixed.index).ok()?;
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout, _) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        fixed.suffix_root,
        fixed.boundary,
    )?;

    Some(RuntimeFrameFixedIndexedTarget {
        descriptor_offset: descriptor_place.byte_offset,
        element_index,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

fn resolve_runtime_frame_fixed_indexed_storage_path_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameFixedIndexedTarget> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    let element_index = path
        .member_index(0)
        .or_else(|| root_member_fixed_index(path.member(0)?))?;
    let collection_slot =
        find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
            slot_matches_table_path(slot, &path)
        })?;
    if runtime_frame_slot_is_inline_fixed_array_storage(input, collection_slot) {
        return None;
    }

    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) =
        resolve_nested_field_layout_with_pairs(&input.layouts, &root_field, path.suffix(1).iter())?;

    Some(RuntimeFrameFixedIndexedTarget {
        descriptor_offset: collection_slot.byte_offset,
        element_index,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

fn root_member_fixed_index(member: &psi_checked_trees::name::Identifier) -> Option<usize> {
    let (_, suffix) = member.as_str().rsplit_once('[')?;
    suffix.strip_suffix(']')?.parse().ok()
}

pub(super) fn resolve_runtime_frame_fixed_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeFrameFixedIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

#[derive(Debug, Clone, Copy)]
struct TableFixedIndexedTargetPath {
    collection: ExpressionHandle,
    index: i64,
    suffix_root: ExpressionHandle,
    /// The fixed `Indexed` node already consumed as the base element. The
    /// suffix walk stops here so a nested path does not apply that index twice.
    boundary: ExpressionHandle,
}

#[derive(Debug, Clone, Copy)]
struct TableIndexedTargetPath {
    collection: ExpressionHandle,
    index: ExpressionHandle,
    suffix_root: ExpressionHandle,
    /// The `Indexed` node whose `collection`+`index` the base resolution consumed.
    /// The suffix-layout walk stops here: the root cursor already IS this node's
    /// element, so everything at or below it must not be re-applied. Explicit
    /// (rather than the walk's collection-unresolvable sentinel) because in a
    /// nested split that prefers the RUNTIME level (`grid[1][j]` -> collection
    /// `grid[1]`, index `j`) the boundary sits ABOVE const-indexed nodes the
    /// sentinel would wrongly re-walk.
    boundary: ExpressionHandle,
}

/// Fold a COMPILE-TIME-CONSTANT index expression to its value: a bare integer
/// literal, or a binary of constants. A projected plan-laid offset plus a
/// constant index becomes a pure-const BINARY, which the bare-`Integer` match below
/// used to reject, so the fixed-index resolver fell through and the read landed
/// at offset 0. Runtime (non-const) indices still return None here: those are
/// hoisted to a slotted temp at the frontend, so ONLY a pure-const binary newly
/// resolves. The miscompile was NATIVE + value-machine only -- the interpreter
/// evaluates the AST, and a plain machine masked it via const-propagation of an
/// adjacent write -- so it needed a value-machine native run to surface. Uses
/// checked arithmetic (an overflowing const index folds to None -> the clean
/// cannot-resolve path, not a wrong offset).
fn const_fold_index_value_in_table(
    table: &ExpressionTable,
    index: ExpressionHandle,
) -> Option<i64> {
    match table.expression(index) {
        ExpressionNode::Mutable(inner) => const_fold_index_value_in_table(table, *inner),
        ExpressionNode::Integer(value) => value.value_i64(),
        ExpressionNode::Binary(binary) => {
            let left = const_fold_index_value_in_table(table, binary.left)?;
            let right = const_fold_index_value_in_table(table, binary.right)?;
            match binary.operator {
                psi_checked_trees::expression::BinaryOperator::Add => left.checked_add(right),
                psi_checked_trees::expression::BinaryOperator::Subtract => left.checked_sub(right),
                psi_checked_trees::expression::BinaryOperator::Multiply => left.checked_mul(right),
                _ => None,
            }
        }
        _ => None,
    }
}

fn fixed_indexed_target_path_in_table(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<TableFixedIndexedTargetPath> {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => fixed_indexed_target_path_in_table(table, *target),
        ExpressionNode::Member(member) => {
            let path = fixed_indexed_target_path_in_table(table, member.receiver)?;
            Some(TableFixedIndexedTargetPath {
                collection: path.collection,
                index: path.index,
                suffix_root: expression,
                boundary: path.boundary,
            })
        }
        ExpressionNode::Indexed(indexed) => {
            // See through an `as_slice`/`as_mut_slice` VIEW of an array:
            // `(X.as_slice())[i]` indexes the same element as `X[i]`. A `let r =
            // X.as_slice()` local folds into `(X.as_slice())[i]`, whose collection
            // is an unmaterialized view (no frame slot), so the element place
            // failed to resolve -- a slice-view element forwarded by value to a
            // value-call (`read(r[i])`, bound as a BranchParameter alias) left the
            // callee's `room.field` reading a zero slot. Unwrapping to `X[i]`
            // resolves it against the underlying array. (Path normalization already
            // peels FULL as_slice views; this peels the INDEXED-element form.)
            let collection = see_through_as_slice_view(table, indexed.collection);
            if let Some(path) = fixed_indexed_target_path_in_table(table, collection) {
                return Some(TableFixedIndexedTargetPath {
                    collection: path.collection,
                    index: path.index,
                    suffix_root: expression,
                    boundary: path.boundary,
                });
            }
            let index = const_fold_index_value_in_table(table, indexed.index)?;
            Some(TableFixedIndexedTargetPath {
                collection,
                index,
                suffix_root: expression,
                boundary: expression,
            })
        }
        _ => None,
    }
}

/// See through an `as_slice`/`as_mut_slice` view of an array to its receiver:
/// `X.as_slice()` -> `X`. `(X.as_slice())[i]` indexes the same element as `X[i]`,
/// and as_slice's receiver is always an array, so peeling the view is sound and
/// lets a slice-VIEW element resolve to the underlying array element. Returns the
/// expression unchanged when it is not such a view.
fn see_through_as_slice_view(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    if let ExpressionNode::Call(call) = table.expression(expression)
        && call.receiver.is_valid()
        && call.arguments.is_empty()
        && matches!(call.target.as_str(), "as_slice" | "as_mut_slice")
    {
        return call.receiver;
    }
    expression
}

/// Trace an ELIDED LOCAL that VIEWS an array (`let r = X.as_slice()`) to the
/// underlying machine-owned array it views. The local has no frame slot, so its
/// declared initializer is resolved and the as_slice view peeled to the array.
/// Shared by the slice-VIEW element resolvers -- the indexed PLACE
/// (resolve_runtime_fixed_indexed_place_in_table) and the leaf DESCRIPTOR
/// (resolve_runtime_storage_leaf_descriptor_in_table) -- so a slice-view element
/// forwarded by value (`r[i]`, e.g. bound as a value-call BranchParameter alias)
/// resolves against the underlying array instead of the unmaterialized view.
fn resolve_elided_local_slice_view_array(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    head_symbol: SymbolHandle,
    head_name: &Identifier,
) -> Option<MachineOwnedCollectionTarget> {
    let initializer = state_local_initializer(input, source_key, head_symbol, head_name)?;
    let underlying = see_through_as_slice_view(&input.program.expression_table, initializer);
    resolve_machine_owned_collection_in_table(
        &input.layouts,
        input,
        dispatch_index,
        input.entry_key.machine,
        source_key.machine,
        &input.program.expression_table,
        underlying,
    )
}

fn indexed_target_path_in_table(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<TableIndexedTargetPath> {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => indexed_target_path_in_table(table, *target),
        ExpressionNode::Member(member) => {
            let path = indexed_target_path_in_table(table, member.receiver)?;
            Some(TableIndexedTargetPath {
                collection: path.collection,
                index: path.index,
                suffix_root: expression,
                boundary: path.boundary,
            })
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(path) = indexed_target_path_in_table(table, indexed.collection) {
                // Nested indices: the op carries ONE runtime index. When THIS
                // level's index is runtime and every level below is const
                // (`grid[1][j]`, `rows[2].data[j]`), split HERE -- the const
                // levels fold into the collection resolution's fixed offset and
                // this node becomes the walk boundary. Otherwise keep the
                // INNERMOST split (`grid[i][2]`: runtime `i` is base-closest;
                // the const `[2]` rides the suffix walk). Both-runtime never
                // lowers: the outer runtime index is not an Integer, so the
                // suffix walk refuses and the loud blockers/fences report it.
                if !indexed_index_is_const(table, indexed.index)
                    && indexed_index_is_const(table, path.index)
                {
                    return Some(TableIndexedTargetPath {
                        collection: indexed.collection,
                        index: indexed.index,
                        suffix_root: expression,
                        boundary: expression,
                    });
                }
                return Some(TableIndexedTargetPath {
                    collection: path.collection,
                    index: path.index,
                    suffix_root: expression,
                    boundary: path.boundary,
                });
            }
            Some(TableIndexedTargetPath {
                collection: indexed.collection,
                index: indexed.index,
                suffix_root: expression,
                boundary: expression,
            })
        }
        _ => None,
    }
}

fn indexed_index_is_const(table: &ExpressionTable, index: ExpressionHandle) -> bool {
    let mut index = index;
    while let ExpressionNode::Mutable(inner) = table.expression(index) {
        index = *inner;
    }
    matches!(table.expression(index), ExpressionNode::Integer(_))
}

fn resolve_indexed_target_suffix_layout_in_table(
    input: &InstructionSelectionInput<'_>,
    root_field: &FieldLayout,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    boundary: ExpressionHandle,
) -> Option<(usize, TypeLayout, TypeLayoutDescriptor)> {
    let cursor = NestedFieldLayoutCursor::from_root(root_field);
    let cursor = resolve_indexed_target_suffix_cursor_in_table(
        &input.layouts,
        cursor,
        expressions,
        expression,
        boundary,
    )?;
    Some((
        cursor.byte_offset(),
        cursor.layout(),
        cursor.type_descriptor().clone(),
    ))
}

/// Walk the suffix of an indexed place (`.field` / `[const]` steps ABOVE the
/// indexed element the root cursor represents) to the leaf's offset+layout.
/// `boundary` is the `Indexed` node the base resolution consumed -- the walk
/// returns the cursor unchanged there (see `TableIndexedTargetPath::boundary`).
/// An invalid `boundary` falls back to the legacy sentinel: the first `Indexed`
/// whose collection is unresolvable (a plain place) is treated as the boundary,
/// which is only correct when the boundary is the INNERMOST `Indexed`.
fn resolve_indexed_target_suffix_cursor_in_table<'layout>(
    layouts: &'layout omega_layout::LayoutPlan,
    cursor: NestedFieldLayoutCursor<'layout>,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    boundary: ExpressionHandle,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    resolve_indexed_target_suffix_cursor_in_table_internal(
        layouts,
        cursor,
        expressions,
        expression,
        boundary,
        false,
    )
}

fn resolve_indexed_stored_integer_suffix_cursor_in_table<'layout>(
    layouts: &'layout omega_layout::LayoutPlan,
    cursor: NestedFieldLayoutCursor<'layout>,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    boundary: ExpressionHandle,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    resolve_indexed_target_suffix_cursor_in_table_internal(
        layouts,
        cursor,
        expressions,
        expression,
        boundary,
        true,
    )
}

fn resolve_indexed_target_suffix_cursor_in_table_internal<'layout>(
    layouts: &'layout omega_layout::LayoutPlan,
    cursor: NestedFieldLayoutCursor<'layout>,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    boundary: ExpressionHandle,
    allow_stored_integer: bool,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    if boundary.is_valid() && expression == boundary {
        return Some(cursor);
    }
    match expressions.expression(expression) {
        ExpressionNode::Mutable(target) => resolve_indexed_target_suffix_cursor_in_table_internal(
            layouts,
            cursor,
            expressions,
            *target,
            boundary,
            allow_stored_integer,
        ),
        ExpressionNode::Indexed(indexed) => {
            let Some(collection_cursor) = resolve_indexed_target_suffix_cursor_in_table_internal(
                layouts,
                cursor,
                expressions,
                indexed.collection,
                boundary,
                allow_stored_integer,
            ) else {
                return Some(cursor);
            };

            let ExpressionNode::Integer(index) = expressions.expression(indexed.index) else {
                return None;
            };
            apply_fixed_array_index_to_cursor(
                collection_cursor,
                usize::try_from(index.value_i64()?).ok()?,
            )
        }
        ExpressionNode::Member(member) => {
            let cursor = resolve_indexed_target_suffix_cursor_in_table_internal(
                layouts,
                cursor,
                expressions,
                member.receiver,
                boundary,
                allow_stored_integer,
            )?;
            if allow_stored_integer {
                resolve_nested_stored_integer_layout_step(
                    layouts,
                    cursor,
                    &member.member,
                    member.member_symbol,
                    None,
                    member.case_variant.as_ref(),
                )
            } else {
                resolve_nested_field_layout_step(
                    layouts,
                    cursor,
                    &member.member,
                    member.member_symbol,
                    None,
                    member.case_variant.as_ref(),
                )
            }
        }
        _ => None,
    }
}

fn apply_fixed_array_index_to_cursor<'layout>(
    cursor: NestedFieldLayoutCursor<'layout>,
    index: usize,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    let (element_type, length) = cursor.type_descriptor().fixed_array()?;
    if index >= length {
        return None;
    }

    let element_layout = TypeLayout {
        size: cursor.layout().size / length,
        alignment: cursor.layout().alignment,
    };

    Some(NestedFieldLayoutCursor::from_indexed_fixed_array_element(
        cursor,
        index,
        element_type,
        element_layout,
    ))
}

fn descriptor_layout(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
) -> TypeLayout {
    match descriptor {
        TypeLayoutDescriptor::Reference { .. } => {
            return TypeLayout {
                size: input.runtime_abi.pointer_size,
                alignment: input.runtime_abi.pointer_alignment,
            };
        }
        TypeLayoutDescriptor::Constrained { base_type, .. } => {
            return descriptor_layout(input, base_type);
        }
        TypeLayoutDescriptor::FixedArray {
            element_type,
            length,
        } => {
            let element = descriptor_layout(input, element_type);
            return TypeLayout {
                size: element.size.saturating_mul(*length),
                alignment: element.alignment,
            };
        }
        TypeLayoutDescriptor::BoundedByteBuffer {
            element_type,
            capacity,
        } => {
            // Owned `{ len, bytes }` inline: a pointer-sized length word followed
            // by `capacity` inline element bytes. Must agree with the omega-layout
            // field sizing for the carrier (a leading len word, then the bytes).
            let element = descriptor_layout(input, element_type);
            return TypeLayout {
                size: input
                    .runtime_abi
                    .pointer_size
                    .saturating_add(element.size.saturating_mul(*capacity)),
                alignment: input.runtime_abi.pointer_alignment,
            };
        }
        TypeLayoutDescriptor::Slice { .. } => {
            let descriptor = input.runtime_abi.slice_descriptor();
            return TypeLayout {
                size: descriptor.total_size(),
                alignment: descriptor.align(),
            };
        }
        TypeLayoutDescriptor::DynamicTrait { .. } => {
            let descriptor = input.runtime_abi.dynamic_trait_descriptor();
            return TypeLayout {
                size: descriptor.total_size(),
                alignment: descriptor.align(),
            };
        }
        TypeLayoutDescriptor::Named { symbol, name } => {
            let type_symbol = *symbol;
            if let Some(primitive_type) = PrimitiveType::from_name(name) {
                return primitive_layout(input, primitive_type);
            }

            if let Some(layout) = builtin_type_layout(input, type_symbol) {
                return layout;
            }

            if type_symbol.is_valid() {
                if let Some(layout) = input
                    .layouts
                    .data_layouts
                    .iter()
                    .find(|(_, layout)| layout.symbol == type_symbol)
                    .map(|(_, layout)| layout.layout)
                {
                    return layout;
                }

                if let Some(layout) = input
                    .layouts
                    .machine_layouts
                    .iter()
                    .find(|(_, layout)| layout.symbol == type_symbol)
                    .map(|(_, layout)| layout.layout)
                {
                    return layout;
                }
            }
        }
        TypeLayoutDescriptor::Unit => {}
    }

    TypeLayout::default()
}

fn inline_fixed_array_element_type(
    descriptor: &TypeLayoutDescriptor,
) -> Option<&TypeLayoutDescriptor> {
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. } => {
            inline_fixed_array_element_type(base_type)
        }
        TypeLayoutDescriptor::FixedArray { element_type, .. } => Some(element_type),
        _ => None,
    }
}

fn runtime_frame_slot_is_inline_fixed_array_storage(
    input: &InstructionSelectionInput<'_>,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
) -> bool {
    // A direct (non-reference) fixed array is stored inline when the slot holds
    // the whole array rather than a slice descriptor. Compare against the array's
    // own layout size rather than the descriptor size: a previous heuristic keyed
    // off `byte_size != slice_descriptor_size`, which misclassified inline arrays
    // that happen to be exactly the descriptor size (e.g. `[T; 2]` with 8-byte T)
    // as descriptors, dereferencing the inline element data as a wild pointer.
    if inline_fixed_array_element_type(&slot.type_descriptor).is_none() {
        return false;
    }
    slot.byte_size == descriptor_layout(input, &slot.type_descriptor).size
}

fn builtin_type_layout(
    input: &InstructionSelectionInput<'_>,
    type_symbol: SymbolHandle,
) -> Option<TypeLayout> {
    if Some(type_symbol) == input.program.symbols.builtin_type_symbol(BuiltinType::UInt) {
        return Some(TypeLayout {
            size: input.runtime_abi.pointer_size,
            alignment: input.runtime_abi.pointer_alignment,
        });
    }

    if Some(type_symbol) == input.program.symbols.builtin_type_symbol(BuiltinType::Int) {
        return Some(TypeLayout {
            size: input.runtime_abi.pointer_size,
            alignment: input.runtime_abi.pointer_alignment,
        });
    }

    None
}

/// Thin wrapper over the shared `omega_layout::primitive_layout`; extracts this
/// crate's pointer geometry and delegates the byte-width match.
fn primitive_layout(
    input: &InstructionSelectionInput<'_>,
    primitive_type: PrimitiveType,
) -> TypeLayout {
    omega_layout::primitive_layout(
        input.runtime_abi.pointer_size,
        input.runtime_abi.pointer_alignment,
        primitive_type,
    )
}

#[cfg(test)]
mod tests {
    use psi_checked_trees::expression::BinaryOperator;
    use psi_checked_trees::types::PrimitiveType;
    use psi_numerics::literals::LandedIntegerType;

    use super::{binary_operator_result_is_bool, primitive_type_for_landed_integer};

    #[test]
    fn comparison_and_logical_binary_results_are_boolean() {
        for operator in [
            BinaryOperator::And,
            BinaryOperator::Or,
            BinaryOperator::Equal,
            BinaryOperator::NotEqual,
            BinaryOperator::Less,
            BinaryOperator::LessOrEqual,
            BinaryOperator::Greater,
            BinaryOperator::GreaterOrEqual,
        ] {
            assert!(binary_operator_result_is_bool(operator), "{operator:?}");
        }
        for operator in [
            BinaryOperator::Add,
            BinaryOperator::BitwiseAnd,
            BinaryOperator::BitwiseOr,
            BinaryOperator::BitwiseXor,
            BinaryOperator::Divide,
            BinaryOperator::Modulo,
            BinaryOperator::Multiply,
            BinaryOperator::ShiftLeft,
            BinaryOperator::ShiftRight,
            BinaryOperator::Subtract,
        ] {
            assert!(!binary_operator_result_is_bool(operator), "{operator:?}");
        }
    }

    #[test]
    fn landed_integer_classification_preserves_width_and_signedness() {
        let cases = [
            (LandedIntegerType::I8, PrimitiveType::I8),
            (LandedIntegerType::I16, PrimitiveType::I16),
            (LandedIntegerType::I32, PrimitiveType::I32),
            (LandedIntegerType::I64, PrimitiveType::I64),
            (LandedIntegerType::U8, PrimitiveType::U8),
            (LandedIntegerType::U16, PrimitiveType::U16),
            (LandedIntegerType::U32, PrimitiveType::U32),
            (LandedIntegerType::U64, PrimitiveType::U64),
            (LandedIntegerType::Addr, PrimitiveType::Addr),
        ];

        for (landing, expected) in cases {
            assert_eq!(primitive_type_for_landed_integer(landing), expected);
        }
    }
}
