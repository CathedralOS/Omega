use crate::InstructionSelectionInput;
use crate::selection::runtime_dispatch::text_writes::string_literal_data_handle;
use crate::selection::runtime_dispatch::writes::mutation::operators::supports_scalar_integer_write;
use crate::selection::storage_places::{
    descriptor_is_bounded_byte_buffer, enum_variant_value_in_table,
    resolve_runtime_frame_base_double_indexed_source_in_table,
    resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table,
    resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_base_indexed_target_with_index_region_in_table,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_near_slot_in_table,
    resolve_runtime_machine_double_indexed_source_in_table,
    resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_pointee_double_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table,
    resolve_runtime_storage_arithmetic_domain_in_table, resolve_runtime_storage_place_in_table,
    resolve_runtime_storage_primitive_type_in_table, static_fixed_array_len_in_table,
};
use omega_abstract_operations::{
    Place, RuntimeStorageRegion, RuntimeValueOperand, SelectedInstructionKind, StateGuardOperator,
};
use omega_control_flow::StateKey;
use omega_layout::ENUM_TAG_BYTES;
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableNamePath,
};

use super::super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_integer_value_in_table,
};

pub(in crate::selection) fn runtime_frame_slot_target_expression(
    expressions: &mut ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
) -> ExpressionHandle {
    let mut members = HandleSpan::empty();
    expressions.push_name_path_member(&mut members, slot.name.clone());

    let mut member_symbols = HandleSpan::empty();
    expressions.push_name_path_member_symbol(&mut member_symbols, slot.symbol);

    expressions.insert(ExpressionNode::Name(TableNamePath {
        members,
        member_symbols,
        head_symbol: slot.symbol,
        symbol: slot.symbol,
    }))
}

/// Decision 17: a folded CONSTANT stored into a Trapping frame slot (`let b: iN in
/// Trapping = <const overflow>`) whose value is out of the slot type's range MUST
/// trap at runtime -- the trap is a runtime abort that cannot be pre-computed
/// (unlike a Saturating clamp, which the fold bakes into the value, so Saturating
/// never reaches here out of range). The static-integer store arm below would
/// otherwise write the raw value and short-circuit the mutation-write fallback
/// that traps. Mirrors the field path's `trapping_constant_overflow_write`: re-emit
/// a guaranteed-overflowing Trapping binary write (`bound ± 1`) so the encoder's
/// ud2/brk fires. `None` when the slot is not Trapping / not an integer primitive /
/// the value is in range (then the normal store is correct).
fn trapping_frame_slot_constant_overflow_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: i64,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let mut scratch = ExpressionTable::default();
    let target = runtime_frame_slot_target_expression(&mut scratch, slot);
    if resolve_runtime_storage_arithmetic_domain_in_table(
        input,
        dispatch_index,
        source_key,
        &scratch,
        target,
    ) != psi_numerics::arithmetic::ArithmeticDomain::Trapping
    {
        return None;
    }
    let primitive = resolve_runtime_storage_primitive_type_in_table(
        input,
        dispatch_index,
        source_key,
        &scratch,
        target,
    )?;
    let (min, max) = super::saturating_integer_bounds(primitive)?;
    if value >= min && value <= max {
        return None;
    }
    let (bound, operator) = if value > max {
        (max, StateGuardOperator::Add)
    } else {
        (min, StateGuardOperator::Subtract)
    };
    let left = runtime_value_operands.insert(RuntimeValueOperand::Immediate(bound));
    let right = runtime_value_operands.insert(RuntimeValueOperand::Immediate(1));
    Some(
        crate::selection::runtime_dispatch::write_place_binary_direct(
            RuntimeStorageRegion::RuntimeFrame,
            slot.byte_offset,
            slot.byte_size,
            left,
            operator,
            right,
            false,
            psi_numerics::arithmetic::ArithmeticDomain::Trapping,
            primitive.is_signed_integer(),
        ),
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_frame_slot_value_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    select_runtime_frame_slot_value_write_in_table_with_source_anchor(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        slot,
        value,
        static_values,
        runtime_value_operands,
        slot.byte_offset,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_frame_slot_value_write_in_table_with_call_ordinal(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    minimum_call_ordinal: usize,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    select_runtime_frame_slot_value_write_in_table_with_source_anchor_and_call_ordinal(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        slot,
        value,
        static_values,
        runtime_value_operands,
        slot.byte_offset,
        Some(minimum_call_ordinal),
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_frame_slot_value_write_in_table_with_source_anchor(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    source_anchor_byte_offset: usize,
) -> Option<SelectedInstructionKind> {
    select_runtime_frame_slot_value_write_in_table_with_source_anchor_and_call_ordinal(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        slot,
        value,
        static_values,
        runtime_value_operands,
        source_anchor_byte_offset,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_frame_slot_value_write_in_table_with_source_anchor_and_call_ordinal(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    source_anchor_byte_offset: usize,
    minimum_call_ordinal: Option<usize>,
) -> Option<SelectedInstructionKind> {
    // `let n = arr.len` / `let n = s.len` where the receiver views a FIXED array
    // (directly, through `.as_slice()`, or through an unmaterialized local alias):
    // the length is a compile-time constant. Without this the resolver drops the
    // `.len` member source (a fixed array has no runtime descriptor len slot to
    // copy from), leaving the local slot UNWRITTEN -- a later guard/use then reads
    // the zeroed slot (a silent read-0 miscompile; the direct guard `arr.len == N`
    // already folds via this same resolver, so the captured-into-a-local form must
    // agree). Emit the constant length directly. Placed first so a `.len` into a
    // pointer-width (usize) slot is never mistaken for an address write below.
    if supports_scalar_integer_write(slot.byte_size)
        && let Some(length) = static_fixed_array_len_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_direct(
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                length,
                slot.byte_size,
            ),
        );
    }

    if slot.byte_size == input.runtime_abi.pointer_size
        && let Some(kind) = select_runtime_frame_slot_address_write_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            slot,
            value,
        )
    {
        return Some(kind);
    }

    // A FLOAT literal value (a folded terminal return, a stamped let
    // initializer, a struct field) writes as its IEEE-754 bit pattern -- the
    // store is bit-blind, and the landing-aware reads keep f32 slots
    // single-rounded. This arm was MISSING: a float value fell through every
    // arm below, the writer returned None, and the slot stayed ZII (the
    // float value-call RETURN divergence).
    if matches!(slot.byte_size, 4 | 8) {
        let mut float_value = value;
        while let ExpressionNode::Mutable(inner) = expressions.expression(float_value) {
            float_value = *inner;
        }
        if let ExpressionNode::Float(literal) = expressions.expression(float_value) {
            let bits = match slot.byte_size {
                4 => i64::from(literal.f32_bits()),
                _ => literal.landed_f64().to_bits() as i64,
            };
            return Some(
                crate::selection::runtime_dispatch::write_place_integer_direct(
                    RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    bits,
                    slot.byte_size,
                ),
            );
        }
    }

    if supports_scalar_integer_write(slot.byte_size)
        && let Some(value) =
            resolve_runtime_static_integer_value_in_table(input, expressions, value, static_values)
    {
        // A Trapping slot given an out-of-range folded constant must TRAP, not
        // store raw (the field path already does this; this closes the `let` gap).
        if let Some(kind) = trapping_frame_slot_constant_overflow_write(
            input,
            dispatch_index,
            value_source_key,
            slot,
            value,
            runtime_value_operands,
        ) {
            return Some(kind);
        }
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_direct(
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                value,
                slot.byte_size,
            ),
        );
    }

    // A NULLARY enum variant (`PairResult::Err`, a bare `Type::Case` Name) written
    // into a slot whose enum is LARGER than a scalar (a payload-carrying result enum,
    // byte_size > 8): the scalar paths above are skipped, but the value is still just
    // the variant's TAG. Write the tag at the slot start (the enum tag lives at
    // offset 0); the nullary variant carries no payload, so no field writes follow.
    // Without this the leaf terminal-value-write emits NOTHING for the nullary arm,
    // so its result-slot copy reads the ZII frame and the arm delivers the wrong tag
    // (the enum-transition-leaf delivery bug, TASKS_FS.md). The payload-carrying arm
    // (a `StructLiteral`) is handled by the mutation-write path, not here.
    if !supports_scalar_integer_write(slot.byte_size)
        && let Some(tag) = enum_variant_value_in_table(&input.layouts, expressions, value)
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_direct(
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                tag,
                ENUM_TAG_BYTES,
            ),
        );
    }

    if slot.byte_size == input.runtime_abi.string_descriptor_size()
        && let Some(value) = expressions.string_literal_value(value)
    {
        let data = string_literal_data_handle(input, value_source_key, statement_index, &value);
        if data.is_valid() {
            return Some(
                crate::selection::runtime_dispatch::write_place_string_direct(
                    omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    data,
                    value.len(),
                ),
            );
        }
    }

    // A literal returned into an owned bounded-carrier result slot must build
    // `{len, inline_bytes}`. Treating it as ordinary storage (or as the legacy
    // `{ptr, len}` String descriptor) leaves the guarded value-call result
    // malformed before the caller copies it through a mutable reference.
    if descriptor_is_bounded_byte_buffer(&slot.type_descriptor)
        && let Some(value) = expressions.string_literal_value(value)
    {
        return Some(SelectedInstructionKind::WritePlaceBoundedBuffer {
            target: omega_abstract_operations::Place::at(
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
            ),
            literal: value,
        });
    }

    if let Some(kind) = super::select_runtime_stored_integer_projection_write_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
        RuntimeStorageRegion::RuntimeFrame,
        slot.byte_offset,
        slot.byte_size,
        runtime_value_operands,
    ) {
        return Some(kind);
    }

    if let Some(pointee) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && pointee.pointee_byte_size == slot.byte_size
        && pointee.pointee_byte_size > 0
        && !matches!(expressions.expression(value), ExpressionNode::Mutable(_))
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_from_pointee(
                pointee.pointer_byte_offset,
                pointee.field_byte_offset,
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                slot.byte_size,
            ),
        );
    }

    // `let d = rooms[2].depth` where `rooms: &[Room]` is a slice DESCRIPTOR local:
    // read the element field through the descriptor's pointer. The fixed-indexed
    // pointee target folds the CONSTANT index into its field offset (deref ptr +
    // index*element_size + field_offset). Without this, the flat storage-place
    // fallback below treats the descriptor as a struct and reads its bytes at the
    // bare field offset -- dropping both the index and the deref, so every constant
    // index aliased element 0's garbage. (A dynamic index lowers via the
    // FrameIndexed path; this mirrors the guard-side constant-index fix 8e775fbd.)
    if let Some(pointee) = resolve_runtime_pointee_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && pointee.pointee_byte_size == slot.byte_size
        && pointee.pointee_byte_size > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_from_pointee(
                pointee.pointer_byte_offset,
                pointee.field_byte_offset,
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                slot.byte_size,
            ),
        );
    }

    // `let n: usize = s.len` where `s` is a runtime slice DESCRIPTOR whose length
    // is not statically known (a slice PARAM). The `.len` place resolver reports
    // the descriptor's low 4-byte len word (the `i32`-assignable convention), so a
    // wider `usize` (8-byte) local slot fails the exact-size copy below and the
    // write is dropped -- the slot stays zeroed and the local reads 0 (a silent
    // miscompile; the field-assign `self.count = s.len` works only because an
    // `i32` count matches the 4-byte word). The descriptor's len is 8 bytes of
    // storage holding the full `usize`, so read it at the target's width. Bounded
    // by the descriptor's len-field size so the read stays inside the descriptor.
    let value_is_len_member = match expressions.expression(value) {
        ExpressionNode::Member(member) => member.member.as_str() == "len",
        _ => false,
    };
    if slot.byte_size > 4
        && supports_scalar_integer_write(slot.byte_size)
        && value_is_len_member
        && let Some(source_place) = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
        && source_place.byte_count == 4
        && slot.byte_size <= input.runtime_abi.slice_descriptor().len_size()
    {
        return Some(crate::selection::runtime_dispatch::copy_places_direct(
            source_place.region,
            source_place.byte_offset,
            RuntimeStorageRegion::RuntimeFrame,
            slot.byte_offset,
            slot.byte_size,
        ));
    }

    if let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && source_place.byte_count == slot.byte_size
        && source_place.byte_count > 0
    {
        return Some(crate::selection::runtime_dispatch::copy_places_direct(
            source_place.region,
            source_place.byte_offset,
            RuntimeStorageRegion::RuntimeFrame,
            slot.byte_offset,
            slot.byte_size,
        ));
    }

    if let Some(indexed_source) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && indexed_source.byte_count == slot.byte_size
        && indexed_source.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_from_fixed_indexed(
                indexed_source.descriptor_offset,
                indexed_source.element_index,
                indexed_source.element_byte_size,
                indexed_source.field_byte_offset,
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                slot.byte_size,
            ),
        );
    }

    if let Some(indexed_source) = resolve_runtime_frame_indexed_target_near_slot_in_table(
        input,
        dispatch_index,
        expressions,
        value,
        source_anchor_byte_offset,
    )
    .or_else(|| {
        resolve_runtime_frame_indexed_target_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
    }) && indexed_source.byte_count == slot.byte_size
        && indexed_source.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_from_indexed(
                indexed_source.descriptor_offset,
                indexed_source.index_region,
                indexed_source.index_offset,
                indexed_source.index_byte_size,
                indexed_source.element_byte_size,
                indexed_source.field_byte_offset,
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                slot.byte_size,
            ),
        );
    }

    // A MACHINE-owned array's runtime-indexed READ (`self.arr[self.k]`) into a
    // frame slot -- the transition-ARGUMENT face used to fall through every
    // strategy here and silently pass a stale/zero parameter (`self.report(
    // self.arr[self.k])` took the wrong arm while the interpreter was right).
    // The write-side machinery already had the resolver and the copy op
    // carries a target_region, so the read is the same lowering pointed the
    // other way.
    if let Some(machine_source) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && machine_source.byte_count == slot.byte_size
        && machine_source.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_from_machine_indexed(
                machine_source.base_byte_offset,
                machine_source.index_region,
                machine_source.index_offset,
                machine_source.index_byte_size,
                machine_source.element_byte_size,
                machine_source.field_byte_offset,
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                slot.byte_size,
            ),
        );
    }

    // A BOTH-RUNTIME nested read below a frame-held recast/reference pointer.
    // This is the read twin of the immediate mutation path: preserve the full
    // deref + two-scaled-index Place and copy its exact leaf into the frame
    // slot used by a local, transition argument, or guarded value.
    if let Some(double_source) = resolve_runtime_pointee_double_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && double_source.byte_count == slot.byte_size
        && double_source.byte_count > 0
        && let Some(source) = double_source.place()
    {
        return Some(SelectedInstructionKind::CopyPlaces {
            source,
            target: Place::at(RuntimeStorageRegion::RuntimeFrame, slot.byte_offset),
            byte_count: slot.byte_size,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        });
    }

    // A BOTH-RUNTIME nested read (`grid[i][j]`) into a frame slot -- the
    // let/transition-argument face of the double-indexed op.
    if let Some(double_source) = resolve_runtime_machine_double_indexed_source_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && double_source.byte_count == slot.byte_size
        && double_source.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_from_machine_double_indexed(
                double_source.base_byte_offset,
                double_source.outer_index_region,
                double_source.outer_index_offset,
                double_source.outer_index_byte_size,
                double_source.outer_stride,
                double_source.inner_index_region,
                double_source.inner_index_offset,
                double_source.inner_index_byte_size,
                double_source.inner_stride,
                double_source.field_byte_offset,
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                slot.byte_size,
            ),
        );
    }

    // A BOTH-RUNTIME nested read of a FRAME-resident 2D array (`g[i][j]`)
    // into a frame slot -- the frame twin of the double-indexed arm above.
    if let Some(double_source) =
        resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
        && double_source.byte_count == slot.byte_size
        && double_source.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_from_frame_base_double_indexed(
                double_source.base_byte_offset,
                double_source.outer_index_region,
                double_source.outer_index_offset,
                double_source.outer_index_byte_size,
                double_source.outer_stride,
                double_source.inner_index_region,
                double_source.inner_index_offset,
                double_source.inner_index_byte_size,
                double_source.inner_stride,
                double_source.field_byte_offset,
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                slot.byte_size,
            ),
        );
    }

    // A FRAME-resident inline array's runtime-indexed READ (`arr[k]` where
    // `arr` is a by-value param or local, no descriptor) into a frame slot.
    // Every value position funnels here through the operand hoist
    // (`let __hoist_N = arr[k]`), so this one arm covers bare reads, binary
    // operands, and transition arguments -- all of which used to fall through
    // and silently read 0 while the interpreter was right. The write-side
    // resolver already computes {base, index, elem, field}; the copy op is the
    // LOAD counterpart of the address computation in
    // `WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame`.
    if let Some(frame_source) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && frame_source.byte_count == slot.byte_size
        && frame_source.byte_count > 0
    {
        // Rung 2c-ix: a FRAME inline array is a no-deref frame-rooted
        // indexed place -- the same materializer discipline as every other
        // indexed shape.
        return Some(SelectedInstructionKind::CopyPlaces {
            source: omega_abstract_operations::Place::at(
                RuntimeStorageRegion::RuntimeFrame,
                frame_source.base_byte_offset,
            )
            .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: frame_source.index_offset,
                index_byte_size: frame_source.index_byte_size,
                element_byte_size: frame_source.element_byte_size,
            })
            .and_then(|place| {
                place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                    frame_source.field_byte_offset,
                ))
            })
            .expect("a frame-base-indexed place is three steps, within PLACE_MAX_STEPS"),
            target: omega_abstract_operations::Place::at(
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
            ),
            byte_count: slot.byte_size,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        });
    }

    if let Some(kind) = super::select_runtime_frame_slot_convert_write_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        slot,
        value,
        static_values,
        runtime_value_operands,
    ) {
        return Some(kind);
    }

    if let Some(kind) = super::select_runtime_logical_not_write_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        RuntimeStorageRegion::RuntimeFrame,
        slot.byte_offset,
        slot.byte_size,
        value,
        runtime_value_operands,
    ) {
        return Some(kind);
    }

    // Integer domain and signedness come from the operands. Float domain and
    // provider identity come from checked evidence carried through control flow.
    super::select_runtime_storage_binary_write_in_table_with_call_ordinal(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        RuntimeStorageRegion::RuntimeFrame,
        slot.byte_offset,
        slot.byte_size,
        value,
        minimum_call_ordinal,
        static_values,
        runtime_value_operands,
    )
}

fn select_runtime_frame_slot_address_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    // `&mut <place>` / `&<place>` (e.g. `&mut cells[index]`): bind the reference
    // slot to the *address* of the referenced place rather than copying the
    // referent. Without this the place is mis-lowered as a copy through the
    // still-uninitialized reference slot.
    if let ExpressionNode::Mutable(inner) = expressions.expression(value) {
        return select_runtime_frame_slot_place_address_write_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            slot,
            *inner,
        );
    }

    if let ExpressionNode::Call(call) = expressions.expression(value)
        && call.receiver.is_valid()
        && call.arguments.is_empty()
        && (call.target.as_str() == "as_slice" || call.target.as_str() == "as_mut_slice")
    {
        if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) {
            return Some(
                crate::selection::runtime_dispatch::write_place_address_pointee(
                    pointer_target.pointer_byte_offset,
                    pointer_target.field_byte_offset,
                    slot.byte_offset,
                ),
            );
        }

        if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) {
            return Some(
                crate::selection::runtime_dispatch::write_place_address_frame_indexed_deref(
                    indexed_target.descriptor_offset,
                    RuntimeStorageRegion::RuntimeFrame,
                    indexed_target.index_offset,
                    indexed_target.index_byte_size,
                    indexed_target.element_byte_size,
                    indexed_target.field_byte_offset,
                    slot.byte_offset,
                ),
            );
        }

        if let Some(target) = resolve_runtime_frame_base_double_indexed_source_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) {
            return Some(
                crate::selection::runtime_dispatch::write_place_address_base_double_indexed(
                    target.base_byte_offset,
                    target.outer_index_offset,
                    target.outer_index_byte_size,
                    target.outer_stride,
                    target.inner_index_offset,
                    target.inner_index_byte_size,
                    target.inner_stride,
                    target.field_byte_offset,
                    slot.byte_offset,
                ),
            );
        }

        if let Some(target) = resolve_runtime_machine_double_indexed_source_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) {
            return Some(
                crate::selection::runtime_dispatch::write_place_address_machine_double_indexed(
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
                    slot.byte_offset,
                ),
            );
        }

        if let Some(target) = resolve_runtime_machine_indexed_target_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) {
            return Some(
                crate::selection::runtime_dispatch::write_place_address_machine_indexed(
                    target.base_byte_offset,
                    target.index_region,
                    target.index_offset,
                    target.index_byte_size,
                    target.element_byte_size,
                    target.field_byte_offset,
                    slot.byte_offset,
                ),
            );
        }

        if let Some(indexed_target) =
            resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                call.receiver,
            )
        {
            return Some(
                crate::selection::runtime_dispatch::write_place_address_base_indexed_with_index_region(
                    indexed_target.base_byte_offset,
                    indexed_target.index_region,
                    indexed_target.index_offset,
                    indexed_target.index_byte_size,
                    indexed_target.element_byte_size,
                    indexed_target.field_byte_offset,
                    slot.byte_offset,
                ),
            );
        }

        let source_place = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        )?;
        return Some(
            crate::selection::runtime_dispatch::write_place_address_direct(
                source_place.region,
                source_place.byte_offset,
                slot.byte_offset,
            ),
        );
    }

    // Shared borrows are represented by their place expression. If the
    // referent cannot fit in the pointer-sized reference slot, the only sound
    // representation is its address. This is the large-record twin of the
    // explicit `Mutable` path above; same-sized small referees have already
    // taken the deliberate content-spill path in argument materialization.
    if slot.type_descriptor.reference_referee().is_some()
        && let Some(source_place) = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
        && source_place.byte_count != slot.byte_size
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_address_direct(
                source_place.region,
                source_place.byte_offset,
                slot.byte_offset,
            ),
        );
    }

    None
}

/// Writes the address of the referenced place `referent` (the target of a `&` /
/// `&mut`) into the reference slot. Resolves the place through the same target
/// shapes as a read — fixed/runtime slice index, inline-array index, pointee
/// field, or a plain storage place — and emits the matching address-to-frame
/// write so the slot ends up holding the element address, not a copy of it.
fn select_runtime_frame_slot_place_address_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    referent: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    if let Some(target) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_address_fixed_indexed(
                target.descriptor_offset,
                target.element_index,
                target.element_byte_size,
                target.field_byte_offset,
                slot.byte_offset,
            ),
        );
    }

    if let Some(target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_address_frame_indexed_deref(
                target.descriptor_offset,
                RuntimeStorageRegion::RuntimeFrame,
                target.index_offset,
                target.index_byte_size,
                target.element_byte_size,
                target.field_byte_offset,
                slot.byte_offset,
            ),
        );
    }

    if let Some(target) = resolve_runtime_frame_base_double_indexed_source_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_address_base_double_indexed(
                target.base_byte_offset,
                target.outer_index_offset,
                target.outer_index_byte_size,
                target.outer_stride,
                target.inner_index_offset,
                target.inner_index_byte_size,
                target.inner_stride,
                target.field_byte_offset,
                slot.byte_offset,
            ),
        );
    }

    if let Some(target) = resolve_runtime_machine_double_indexed_source_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_address_machine_double_indexed(
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
                slot.byte_offset,
            ),
        );
    }

    if let Some(target) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_address_machine_indexed(
                target.base_byte_offset,
                target.index_region,
                target.index_offset,
                target.index_byte_size,
                target.element_byte_size,
                target.field_byte_offset,
                slot.byte_offset,
            ),
        );
    }

    if let Some(target) = resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_address_base_indexed_with_index_region(
                target.base_byte_offset,
                target.index_region,
                target.index_offset,
                target.index_byte_size,
                target.element_byte_size,
                target.field_byte_offset,
                slot.byte_offset,
            ),
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_address_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                slot.byte_offset,
            ),
        );
    }

    let source_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    )?;
    Some(
        crate::selection::runtime_dispatch::write_place_address_direct(
            source_place.region,
            source_place.byte_offset,
            slot.byte_offset,
        ),
    )
}
