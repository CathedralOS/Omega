use super::super::text_writes::string_literal_data_handle;
use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
    StateGuardOperator,
};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;

use super::super::super::storage_places::{
    RuntimeStoragePlace, resolve_fixed_array_length, resolve_fixed_array_length_in_table,
    resolve_runtime_frame_base_indexed_target, resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_fixed_indexed_target,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place,
    resolve_runtime_storage_place_in_table, resolve_slice_element_byte_size_in_table,
};
use super::fixed_array_slices::{
    literal_subslice_range_bounds, resolved_subslice_descriptor_base_in_table,
};

/// One bound of a subslice range as the backend can lower it: a compile-time
/// integer or a runtime value living in a frame slot (a `usize` parameter or
/// local). Anything else (computed expressions like `i + 1`) is unsupported and
/// makes the emitter decline, so the emission-planning blocker pass reports it.
enum SubsliceBound {
    Literal(usize),
    Slot(RuntimeStoragePlace),
}

/// The runtime-descriptor base a subslice resolves against: the innermost frame
/// slot holding a fat `{ptr, len}` descriptor, plus the window the peeled
/// literal-bounded subslice layers select inside it. `bias_elements` is how many
/// elements the window start sits past the descriptor's pointer;
/// `window_len_elements` is the window's literal length when every peeled layer
/// pinned it (`None` = the window runs to `source.len - bias`).
struct RuntimeDescriptorWindow {
    place: RuntimeStoragePlace,
    element_byte_size: usize,
    bias_elements: usize,
    window_len_elements: Option<usize>,
}

/// A runtime-bounded subslice base the emitter can lower: an existing runtime
/// slice descriptor (a `&[T]` param/local frame slot), or a directly-placed
/// FIXED array (`self.arr[lo..hi]` -- a machine field or frame aggregate whose
/// address and declared length are known). The fixed-array case synthesizes the
/// whole-array descriptor into the TARGET slot and then shrinks it in place,
/// reusing the descriptor-window emission unchanged.
enum SubsliceBase {
    Descriptor(RuntimeDescriptorWindow),
    FixedArray {
        place: RuntimeStoragePlace,
        element_byte_size: usize,
        length: usize,
    },
}

/// Resolve a subslice base that is a directly-placed FIXED array: the collection
/// resolves to a raw element place (machine field like `self.arr`, or a frame
/// aggregate) with a compile-time length. A `&[T]` slice descriptor never
/// matches (`resolve_fixed_array_length_in_table` is type-driven and returns
/// `None` for it), so this cannot shadow the descriptor-window path.
fn resolve_fixed_array_subslice_base(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    collection: ExpressionHandle,
) -> Option<SubsliceBase> {
    let length = resolve_fixed_array_length_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        collection,
    )?;
    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        collection,
    )?;
    if length == 0 || place.byte_count == 0 || place.byte_count % length != 0 {
        return None;
    }
    Some(SubsliceBase::FixedArray {
        element_byte_size: place.byte_count / length,
        place,
        length,
    })
}

/// Materialize a subslice of a *runtime* slice descriptor (`entries[start..]` /
/// `entries[start..end]` where `entries` is a `&[T]` whose length is only known
/// at runtime) into the target descriptor `slot`. Unlike the fixed-array path,
/// the length is not a compile-time constant, so the new descriptor is computed
/// from the source one:
///   target.ptr = source.ptr + start * element_byte_size
///   target.len = (end - start)          when `end` is a literal
///              = source.len - start      when the range is open-ended
/// Because the binary write reads `left` from the source descriptor and writes
/// the target, this also handles the self-recursive case where source and target
/// are the same slot (an in-place shrink) — exactly what a `decreases … Length`
/// recursion needs.
#[allow(clippy::too_many_arguments)]
fn emit_runtime_frame_slot_runtime_subslice_descriptor_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let descriptor_size = input.runtime_abi.slice_descriptor_size();
    if slot.byte_size != descriptor_size {
        return false;
    }
    let ExpressionNode::Indexed(indexed) = expressions.expression(value) else {
        return false;
    };
    let ExpressionNode::Range(range) = expressions.expression(indexed.index) else {
        return false;
    };

    // The base must bottom out in a runtime slice descriptor living in a frame
    // slot (literal-bounded subslice layers fold into a window), OR be a
    // directly-placed fixed array whose whole-array descriptor can be
    // synthesized into the target slot before the in-place shrink.
    let base = match resolve_runtime_descriptor_window(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        indexed.collection,
    ) {
        Some(window) => SubsliceBase::Descriptor(window),
        None => match resolve_fixed_array_subslice_base(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            indexed.collection,
        ) {
            Some(base) => base,
            None => return false,
        },
    };

    // `start` (default 0) and optional `end`, each a literal or a frame slot.
    let start = if range.start.is_valid() {
        match resolve_subslice_bound(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            range.start,
            // The START lowers through `WriteRuntimeFrameIndexedAddressToRuntimeFrame`,
            // whose index is region-tagged since 2026-07-10 -- a machine field
            // start (`self.arr[self.lo..]`) reads off the machine base.
            true,
        ) {
            Some(start) => start,
            None => return false,
        }
    } else {
        SubsliceBound::Literal(0)
    };
    let end = if range.end.is_valid() {
        match resolve_subslice_bound(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            range.end,
            // The END rides in a region-tagged `Storage` operand of a `Subtract`,
            // so a machine FIELD end (`path[0..self.k]`) is lowerable.
            true,
        ) {
            // An inclusive end (`a..=b`) is the exclusive end `b + 1`; folding
            // it here keeps every emission path on half-open ranges. A RUNTIME
            // inclusive end would need a +1 at runtime — decline for now.
            Some(SubsliceBound::Literal(end)) if range.end_inclusive => match end.checked_add(1) {
                Some(end) => Some(SubsliceBound::Literal(end)),
                None => return false,
            },
            Some(_) if range.end_inclusive => return false,
            Some(end) => Some(end),
            None => return false,
        }
    } else {
        None
    };
    if let (SubsliceBound::Literal(start), Some(SubsliceBound::Literal(end))) = (&start, &end)
        && end < start
    {
        return false;
    }

    let window = match base {
        SubsliceBase::Descriptor(window) => window,
        SubsliceBase::FixedArray {
            place,
            element_byte_size,
            length,
        } => {
            // Seed the target slot with the WHOLE-ARRAY descriptor -- ptr = the
            // array's address, len = its declared length (the same two writes
            // the literal fixed-array path emits) -- then shrink IN PLACE with
            // the runtime bounds. The shrink's ptr write dereferences
            // target.ptr (just seeded) and its len write reads target.len only
            // for an open end (also just seeded), and every read happens
            // before its own overwrite -- the documented self-recursive
            // in-place order of `emit_runtime_descriptor_subslice`.
            let descriptor = input.runtime_abi.slice_descriptor();
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
                    source_region: place.region,
                    source_offset: place.byte_offset,
                    target_offset: slot.byte_offset + descriptor.ptr_offset(),
                },
                source_key: value_source_key,
                source_statement: statement_index,
            });
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                    target_region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: slot.byte_offset + descriptor.len_offset(),
                    byte_size: descriptor.len_size(),
                    value: length as i64,
                },
                source_key: value_source_key,
                source_statement: statement_index,
            });
            RuntimeDescriptorWindow {
                place: RuntimeStoragePlace {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: slot.byte_offset,
                    byte_count: descriptor_size,
                },
                element_byte_size,
                bias_elements: 0,
                window_len_elements: Some(length),
            }
        }
    };

    emit_runtime_descriptor_subslice(
        input,
        value_source_key,
        statement_index,
        slot,
        &window,
        &start,
        end.as_ref(),
        runtime_value_operands,
        selected_instructions,
    );
    true
}

/// Resolve one subslice range bound to a lowerable operand: a literal integer
/// or a pointer-sized runtime value living in storage. A RuntimeFrame bound (a
/// `usize` param/local) is always lowerable; a Machine-region bound (a `usize`
/// machine FIELD such as `self.k`) is lowerable only where the consuming
/// instruction reads through a region-tagged operand -- the END of the range
/// (`allow_machine_region`), not the START (whose indexed-address instruction
/// has a frame-only index field).
fn resolve_subslice_bound(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    bound: ExpressionHandle,
    allow_machine_region: bool,
) -> Option<SubsliceBound> {
    if let ExpressionNode::Integer(value) = expressions.expression(bound) {
        return usize::try_from(value.value_i64()?)
            .ok()
            .map(SubsliceBound::Literal);
    }
    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        bound,
    )?;
    let len_size = input.runtime_abi.slice_descriptor().len_size();
    let region_ok = place.region == RuntimeStorageRegion::RuntimeFrame
        || (allow_machine_region && place.region == RuntimeStorageRegion::Machine);
    (region_ok && place.byte_count == len_size).then_some(SubsliceBound::Slot(place))
}

/// Resolve a subslice base expression down to its runtime descriptor frame slot,
/// folding any literal-bounded subslice layers in between into the window.
///
/// `sub` (a slice param/local with a descriptor slot) resolves directly;
/// `sub[1..]` / `sub[1..3]` layers compose by accumulating the start bias and
/// narrowing the literal window length. A layer with a non-literal bound cannot
/// fold (its offset is unknown at selection time), so the whole base declines.
fn resolve_runtime_descriptor_window(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    collection: ExpressionHandle,
) -> Option<RuntimeDescriptorWindow> {
    if let ExpressionNode::Indexed(inner) = expressions.expression(collection)
        && let ExpressionNode::Range(range) = expressions.expression(inner.index)
    {
        let window = resolve_runtime_descriptor_window(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            inner.collection,
        )?;
        let start = if range.start.is_valid() {
            let ExpressionNode::Integer(start) = expressions.expression(range.start) else {
                return None;
            };
            usize::try_from(start.value_i64()?).ok()?
        } else {
            0
        };
        let end = if range.end.is_valid() {
            let ExpressionNode::Integer(end) = expressions.expression(range.end) else {
                return None;
            };
            let end = usize::try_from(end.value_i64()?).ok()?;
            // Inclusive literal ends fold to the half-open `end + 1`.
            Some(if range.end_inclusive {
                end.checked_add(1)?
            } else {
                end
            })
        } else {
            None
        };
        // This layer's `[start, end)` is relative to the current window: the
        // bias advances by `start`; a literal end pins the window length to
        // `end - start`, an open end shortens any already-pinned length.
        let window_len_elements = match end {
            Some(end) => Some(end.checked_sub(start)?),
            None => match window.window_len_elements {
                Some(len) => Some(len.checked_sub(start)?),
                None => None,
            },
        };
        return Some(RuntimeDescriptorWindow {
            place: window.place,
            element_byte_size: window.element_byte_size,
            bias_elements: window.bias_elements.checked_add(start)?,
            window_len_elements,
        });
    }

    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        collection,
    )?;
    if place.region != RuntimeStorageRegion::RuntimeFrame
        || place.byte_count != input.runtime_abi.slice_descriptor_size()
    {
        return None;
    }
    let element_byte_size = resolve_slice_element_byte_size_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        collection,
    )?;
    Some(RuntimeDescriptorWindow {
        place,
        element_byte_size,
        bias_elements: 0,
        window_len_elements: None,
    })
}

/// Emit the canonical fat-descriptor subslice (chapter 19, owned by
/// `omega-runtime-abi`): `new.ptr = base.ptr + start * element_byte_size`,
/// `new.len = end - start`, against a runtime descriptor window.
///
/// All reads happen before the corresponding target byte is overwritten, so the
/// self-recursive in-place shrink (source slot == target slot) stays correct:
/// the pointer write reads `source.ptr` and stores the new pointer in one
/// instruction, and the length writes never read the pointer half.
#[allow(clippy::too_many_arguments)]
fn emit_runtime_descriptor_subslice(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    window: &RuntimeDescriptorWindow,
    start: &SubsliceBound,
    end: Option<&SubsliceBound>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let descriptor = input.runtime_abi.slice_descriptor();
    let ptr_offset = descriptor.ptr_offset();
    let len_offset = descriptor.len_offset();
    let len_size = descriptor.len_size();
    let ptr_size = len_offset - ptr_offset;
    let source = &window.place;
    let bias = window.bias_elements;
    let element_byte_size = window.element_byte_size;

    // target.ptr = source.ptr + (bias + start) * element_byte_size.
    match start {
        SubsliceBound::Literal(start) => {
            // The literal pointer delta comes from the canonical subslice seam.
            let ptr_delta = descriptor
                .subslice(0, element_byte_size, bias + start, bias + start)
                .map(|layout| layout.ptr_delta)
                .unwrap_or(0);
            let ptr_left = runtime_value_operands.insert(RuntimeValueOperand::Storage {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: source.byte_offset + ptr_offset,
                byte_size: ptr_size,
            });
            let ptr_right =
                runtime_value_operands.insert(RuntimeValueOperand::Immediate(ptr_delta as i64));
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeStorageBinary {
                    target_region: RuntimeStorageRegion::RuntimeFrame,
                    target_offset: slot.byte_offset + ptr_offset,
                    byte_size: ptr_size,
                    left: ptr_left,
                    operator: StateGuardOperator::Add,
                    right: ptr_right,
                    is_float: false,
                    domain: omega_core::arithmetic::ArithmeticDomain::Exact,
                    target_signed: false,
                },
                source_key: value_source_key,
                source_statement: statement_index,
            });
        }
        SubsliceBound::Slot(start_place) => {
            // Runtime start: the descriptor-indexed address op computes
            // `*(descriptor.ptr) + index * element + field` in one instruction;
            // the literal bias rides in the field offset.
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
                    descriptor_offset: source.byte_offset + ptr_offset,
                    index_offset: start_place.byte_offset,
                    index_region: start_place.region,
                    element_byte_size,
                    field_byte_offset: bias * element_byte_size,
                    target_offset: slot.byte_offset + ptr_offset,
                },
                source_key: value_source_key,
                source_statement: statement_index,
            });
        }
    }

    // target.len = end - start, where an open end means "to the window's end".
    let target_len_offset = slot.byte_offset + len_offset;
    let push_len_binary = |runtime_value_operands: &mut Arena<RuntimeValueOperand>,
                           selected_instructions: &mut SelectedInstructionSink,
                           left: RuntimeValueOperand,
                           right: RuntimeValueOperand| {
        let left = runtime_value_operands.insert(left);
        let right = runtime_value_operands.insert(right);
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeStorageBinary {
                target_region: RuntimeStorageRegion::RuntimeFrame,
                target_offset: target_len_offset,
                byte_size: len_size,
                left,
                operator: StateGuardOperator::Subtract,
                right,
                is_float: false,
                domain: omega_core::arithmetic::ArithmeticDomain::Exact,
                target_signed: false,
            },
            source_key: value_source_key,
            source_statement: statement_index,
        });
    };
    let source_len = RuntimeValueOperand::Storage {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset: source.byte_offset + len_offset,
        byte_size: len_size,
    };
    let bound_operand = |bound: &SubsliceBound| match bound {
        SubsliceBound::Literal(value) => RuntimeValueOperand::Immediate(*value as i64),
        // A bound Slot reads through its own region (frame param/local or a
        // Machine-region field END like `self.k`); the operand is region-tagged.
        SubsliceBound::Slot(place) => RuntimeValueOperand::Storage {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
        },
    };

    match (start, end) {
        (SubsliceBound::Literal(start), Some(SubsliceBound::Literal(end))) => {
            // Both bounds literal: the canonical seam yields the length.
            let len = descriptor
                .subslice(0, element_byte_size, *start, *end)
                .map(|layout| layout.len)
                .unwrap_or(0);
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                    target_region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: target_len_offset,
                    byte_size: len_size,
                    value: len as i64,
                },
                source_key: value_source_key,
                source_statement: statement_index,
            });
        }
        (SubsliceBound::Literal(start), None) => match window.window_len_elements {
            Some(window_len) => selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                    target_region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: target_len_offset,
                    byte_size: len_size,
                    value: window_len.saturating_sub(*start) as i64,
                },
                source_key: value_source_key,
                source_statement: statement_index,
            }),
            // Open end over an open window: len = source.len - (bias + start).
            None => push_len_binary(
                runtime_value_operands,
                selected_instructions,
                source_len,
                RuntimeValueOperand::Immediate((bias + start) as i64),
            ),
        },
        (start, Some(end)) => push_len_binary(
            runtime_value_operands,
            selected_instructions,
            bound_operand(end),
            bound_operand(start),
        ),
        (SubsliceBound::Slot(_), None) => match window.window_len_elements {
            Some(window_len) => push_len_binary(
                runtime_value_operands,
                selected_instructions,
                RuntimeValueOperand::Immediate(window_len as i64),
                bound_operand(start),
            ),
            None if bias == 0 => push_len_binary(
                runtime_value_operands,
                selected_instructions,
                source_len,
                bound_operand(start),
            ),
            // Three terms (source.len - bias - start): stage through the target
            // length slot itself. The first write consumes source.len, so the
            // second is safe even for the in-place shrink.
            None => {
                push_len_binary(
                    runtime_value_operands,
                    selected_instructions,
                    source_len,
                    RuntimeValueOperand::Immediate(bias as i64),
                );
                push_len_binary(
                    runtime_value_operands,
                    selected_instructions,
                    RuntimeValueOperand::Storage {
                        region: RuntimeStorageRegion::RuntimeFrame,
                        byte_offset: target_len_offset,
                        byte_size: len_size,
                    },
                    bound_operand(start),
                );
            }
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn emit_runtime_frame_slot_slice_descriptor_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if slot.byte_size != input.runtime_abi.slice_descriptor_size() {
        return false;
    }

    // A bare STRING / byte-slice LITERAL (`"hello"` passed as a slice argument or
    // bound to a `&[u8]` slot): its bytes live in a rodata data object with no frame
    // place, so the subslice/as_slice strategies below all miss it and the descriptor
    // slot would keep its zero bytes -- an EMPTY slice (ptr 0, len 0). Write the full
    // `{ptr -> rodata, len}` descriptor in one shot with `WriteRuntimeFrameString`
    // (the same instruction a string local/field initializer uses). Every consumer of
    // this seam -- transition-edge args, value-call leaves, straight-line branches,
    // preludes -- gets the fix here. (TASKS_FS.md step 14: the forwarded-slice-length
    // bug; unblocks passing a slice literal as a machine-call argument.)
    if let Some(literal) = expressions.string_literal_value(value) {
        let data = string_literal_data_handle(input, value_source_key, statement_index, &literal);
        if data.is_valid() {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeFrameString {
                    byte_offset: slot.byte_offset,
                    data,
                    byte_length: literal.len(),
                },
                source_key: value_source_key,
                source_statement: statement_index,
            });
            return true;
        }
    }

    if emit_runtime_frame_slot_literal_subslice_descriptor_write_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        slot,
        value,
        selected_instructions,
        statement_index,
    ) {
        return true;
    }

    // Subslice of a runtime descriptor (slice param/local): every consumer of
    // slice-descriptor writes — locals, arguments, branch preludes — routes
    // through the same generalized lowering.
    if emit_runtime_frame_slot_runtime_subslice_descriptor_write_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        slot,
        value,
        runtime_value_operands,
        selected_instructions,
    ) {
        return true;
    }

    // A BARE fixed-array PLACE passed as a slice (`self.pick(self.nums, ..)`
    // -- no as_slice(), no subslice): write the {ptr = region_base + offset,
    // len = N} descriptor. Without this arm the value fell through EVERY
    // strategy; a [i32; 4]'s 16 bytes coincidentally matched the descriptor
    // size, so the RAW ARRAY BYTES became the pointer (native SIGSEGV --
    // probed 2026-07-09m; a [u8; 4] took a different wrong path and read
    // garbage). `WriteRuntimeStorageAddressToRuntimeFrame` carries the source
    // REGION, so machine and frame arrays both relocate correctly.
    if !matches!(expressions.expression(value), ExpressionNode::Call(_))
        && let Some(place) = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
        && let Some(length) = resolve_fixed_array_length_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
                source_region: place.region,
                source_offset: place.byte_offset,
                target_offset: slot.byte_offset,
            },
            source_key: value_source_key,
            source_statement: statement_index,
        });
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                target_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                byte_offset: slot.byte_offset + input.runtime_abi.slice_descriptor().len_offset(),
                byte_size: 8,
                value: length as i64,
            },
            source_key: value_source_key,
            source_statement: statement_index,
        });
        return true;
    }

    let ExpressionNode::Call(call) = expressions.expression(value) else {
        return false;
    };
    if !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || (call.target.as_str() != "as_slice" && call.target.as_str() != "as_mut_slice")
    {
        return false;
    }

    if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        let Some(length) = resolve_fixed_array_length_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) else {
            return false;
        };

        emit_fixed_indexed_slice_descriptor(
            input,
            value_source_key,
            statement_index,
            slot,
            indexed_target.descriptor_offset,
            indexed_target.element_index,
            indexed_target.element_byte_size,
            indexed_target.field_byte_offset,
            length,
            selected_instructions,
        );
        return true;
    }

    // `ptr.field.as_[mut_]slice()` where `ptr` is a runtime reference parameter: the
    // slice's data pointer is the referent's ADDRESS, computed from the runtime
    // pointer (the param slot's value) plus the field offset -- NOT a static address
    // (which only coincidentally matched the runtime pointer before the frame layout
    // changed).
    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        let Some(length) = resolve_fixed_array_length_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) else {
            return false;
        };
        emit_pointee_slice_descriptor(
            input,
            value_source_key,
            statement_index,
            slot,
            pointer_target.pointer_byte_offset,
            pointer_target.field_byte_offset,
            length,
            selected_instructions,
        );
        return true;
    }

    let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) else {
        let receiver = expressions.to_tree(call.receiver);
        let simplified_receiver = super::mutation::simplify_runtime_expression_with_state_locals(
            input,
            value_source_key,
            statement_index,
            &receiver,
        );
        if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target(
            input,
            dispatch_index,
            value_source_key,
            &simplified_receiver,
        ) {
            let Some(length) = resolve_fixed_array_length(
                input,
                dispatch_index,
                value_source_key,
                &simplified_receiver,
            ) else {
                return false;
            };

            emit_fixed_indexed_slice_descriptor(
                input,
                value_source_key,
                statement_index,
                slot,
                indexed_target.descriptor_offset,
                indexed_target.element_index,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                length,
                selected_instructions,
            );
            return true;
        }
        if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target(
            input,
            dispatch_index,
            value_source_key,
            &simplified_receiver,
        ) {
            let Some(length) = resolve_fixed_array_length(
                input,
                dispatch_index,
                value_source_key,
                &simplified_receiver,
            ) else {
                return false;
            };

            emit_indexed_slice_descriptor(
                input,
                value_source_key,
                statement_index,
                slot,
                indexed_target.base_byte_offset,
                indexed_target.index_offset,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                length,
                selected_instructions,
            );
            return true;
        }

        let Some(source_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            value_source_key,
            "",
            "",
            &simplified_receiver,
        ) else {
            return false;
        };
        let Some(length) = resolve_fixed_array_length(
            input,
            dispatch_index,
            value_source_key,
            &simplified_receiver,
        ) else {
            return false;
        };

        emit_static_slice_descriptor(
            input,
            value_source_key,
            statement_index,
            slot,
            source_place.region,
            source_place.byte_offset,
            length,
            selected_instructions,
        );
        return true;
    };

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        let Some(length) = resolve_fixed_array_length_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) else {
            return false;
        };

        emit_indexed_slice_descriptor(
            input,
            value_source_key,
            statement_index,
            slot,
            indexed_target.base_byte_offset,
            indexed_target.index_offset,
            indexed_target.element_byte_size,
            indexed_target.field_byte_offset,
            length,
            selected_instructions,
        );
        return true;
    }

    let Some(length) = resolve_fixed_array_length_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) else {
        return false;
    };

    emit_static_slice_descriptor(
        input,
        value_source_key,
        statement_index,
        slot,
        source_place.region,
        source_place.byte_offset,
        length,
        selected_instructions,
    );
    true
}

#[allow(clippy::too_many_arguments)]
fn emit_fixed_indexed_slice_descriptor(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset: slot.byte_offset,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    emit_slice_descriptor_length(
        input,
        value_source_key,
        statement_index,
        slot,
        length,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_indexed_slice_descriptor(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset: slot.byte_offset,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    emit_slice_descriptor_length(
        input,
        value_source_key,
        statement_index,
        slot,
        length,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_pointee_slice_descriptor(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    // ptr = *(runtime pointer at pointer_byte_offset) + field_byte_offset.
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame {
            pointer_byte_offset,
            field_byte_offset,
            target_offset: slot.byte_offset,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    emit_slice_descriptor_length(
        input,
        value_source_key,
        statement_index,
        slot,
        length,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_static_slice_descriptor(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region,
            source_offset,
            target_offset: slot.byte_offset,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    emit_slice_descriptor_length(
        input,
        value_source_key,
        statement_index,
        slot,
        length,
        selected_instructions,
    );
}

fn emit_slice_descriptor_length(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let descriptor = input.runtime_abi.slice_descriptor();
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset + descriptor.len_offset(),
            byte_size: descriptor.len_size(),
            value: length as i64,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
}

#[allow(clippy::too_many_arguments)]
fn emit_runtime_frame_slot_literal_subslice_descriptor_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    selected_instructions: &mut SelectedInstructionSink,
    statement_index: usize,
) -> bool {
    let ExpressionNode::Indexed(indexed) = expressions.expression(value) else {
        return false;
    };
    let ExpressionNode::Range(range) = expressions.expression(indexed.index) else {
        return false;
    };
    let Some(source) = resolved_subslice_descriptor_base_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        indexed.collection,
    ) else {
        return false;
    };
    let Some((start, end)) = literal_subslice_range_bounds(expressions, range, source.length)
    else {
        return false;
    };
    // Uniform subslice: new.ptr = base.ptr + start * element_byte_size,
    // new.len = end - start. The shape is owned by omega-runtime-abi.
    let Some(subslice) = input.runtime_abi.slice_descriptor().subslice(
        source.place.byte_offset,
        source.element_byte_size,
        start,
        end,
    ) else {
        return false;
    };

    emit_static_slice_descriptor(
        input,
        value_source_key,
        statement_index,
        slot,
        source.place.region,
        subslice.ptr_delta,
        subslice.len,
        selected_instructions,
    );
    true
}
