//! Selection of the synthesized wire decoder call (chapter 20, wire stage
//! 2b): `Schema::decode(&mut value, &buffer, &mut read, &mut ok)` lowers
//! into a straight-line sequence of wire-read operations -- zero the cursor,
//! set the sticky ok flag, expect the CURRENT era discriminator bytes, then
//! per field in field-number order the expected field-number varint bytes
//! (compile-time) and a value varint read into the field's storage. Only the
//! current era decodes; a payload carrying any other era fails on its first
//! discriminator byte (historical eras await the stage 3 `Versioned<T>`
//! container).
//!
//! A NESTED MESSAGE field decodes as: expected tag bytes, a LENGTH varint
//! read into the wire scratch's end slot, a nested OPEN (end slot becomes the
//! absolute sub-region bound, checked against the buffer), the child's
//! expected tags and value varints one member deeper -- WITHOUT an era
//! discriminator (decision 10: the era rides only the top-level envelope) --
//! and a nested CLOSE that fails the sticky ok unless the cursor landed
//! exactly on the bound (the declared length must equal the bytes the fields
//! consumed). Child reads stay bounds-checked against the full buffer length
//! for memory safety; the exact-end check catches a length that disagrees
//! with the content in either direction.
//!
//! Front-end validation (`omega-validation::wire`) has already guaranteed
//! the call shape, the field coverage, and the stage 2 field set (scalar-only
//! children, one nesting level), so an unresolvable place here is a planning
//! blocker rather than a silent skip.

use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, SelectedInstruction, SelectedInstructionKind,
};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::statement::StatementNode;
use omega_checked_trees::wire::{WireMember, WireScalarEncoding, wire_varint_bytes};
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;

use super::storage_places::{RuntimeStoragePlace, resolve_runtime_storage_place_in_table};

/// One field of the CURRENT era, ready to read.
struct WireFieldRead {
    number: i64,
    content: WireReadContent,
}

/// What follows a field's expected tag bytes: a scalar target place, a
/// nested sub-message's own field list (resolved one member deeper), or a
/// repeated field's packed element run.
enum WireReadContent {
    Scalar {
        encoding: WireScalarEncoding,
        place: RuntimeStoragePlace,
    },
    Nested { children: Vec<WireFieldRead> },
    /// A borrowed `&[u8]` field: a byte-LENGTH varint then a fat `{ptr, len}`
    /// descriptor viewing the buffer content, stored zero-copy into `place`.
    ByteSlice { place: RuntimeStoragePlace },
    /// A repeated field: a byte-LENGTH varint opens a bounded sub-region,
    /// then up to `max_count` guarded element reads (each runs only while
    /// the cursor sits below the bound, bumping the count companion), and
    /// the exact-end close rejects payloads whose length disagrees with the
    /// elements -- including MORE elements than the declared maximum.
    Repeated {
        element: WireScalarEncoding,
        base: RuntimeStoragePlace,
        count: RuntimeStoragePlace,
        max_count: usize,
    },
}

/// Lower a recognized `decode` call statement; `true` when the statement
/// produced its read sequence (the emission planner checks for the wire reads
/// when it exempts the call from the unlowered-call blockers).
pub(super) fn select_wire_decode_call(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
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
    let Some(StatementNode::Call(call)) = input
        .program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return false;
    };
    let Some(schema) = input.program.wire_decode_call_schema(call) else {
        return false;
    };

    let [value_argument, buffer_argument, read_argument, ok_argument] = input
        .program
        .statement_table
        .expression_handles(call.arguments)
    else {
        return false;
    };

    // Copy the four argument expressions into a scratch table so the
    // per-field member writes can be synthesized next to them.
    let mut expressions = ExpressionTable::with_expression_capacity(8);
    let value_root = copied_place_argument(input, &mut expressions, *value_argument);
    let buffer_root = copied_place_argument(input, &mut expressions, *buffer_argument);
    let read_root = copied_place_argument(input, &mut expressions, *read_argument);
    let ok_root = copied_place_argument(input, &mut expressions, *ok_argument);

    let Some(buffer_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        &expressions,
        buffer_root,
    ) else {
        return false;
    };
    let Some(read_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        &expressions,
        read_root,
    ) else {
        return false;
    };
    if read_place.byte_count != 8 {
        return false;
    }
    let Some(ok_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        &expressions,
        ok_root,
    ) else {
        return false;
    };
    if ok_place.byte_count != 1 {
        return false;
    }

    // Collect the CURRENT era's fields in field-number order, resolving each
    // schema field name against the runtime value's matching member (nested
    // message fields resolve their CHILD schema's fields one member deeper).
    let Some(fields) = collect_field_reads(
        input,
        dispatch_index,
        source_key,
        &mut expressions,
        value_root,
        schema,
        true,
    ) else {
        return false;
    };
    let has_nested = fields.iter().any(|field| {
        matches!(
            field.content,
            WireReadContent::Nested { .. } | WireReadContent::Repeated { .. }
        )
    });
    // The nested/repeated end-bound slot is the wire scratch's first 8 bytes
    // (the encoder's descriptor ptr slot -- never live at the same time,
    // since wire ops run strictly inside one statement).
    let end_offset = input.runtime_storage.wire_scratch_base;
    if has_nested && (end_offset == 0 || input.runtime_storage.wire_scratch_size < 8) {
        return false;
    }

    let era = input.program.wire_schema_current_era(schema);
    // The decode buffer's compile-time byte length bounds every runtime read.
    let buffer_length = buffer_place.byte_count;

    let mut push = |kind: SelectedInstructionKind| {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key,
            source_statement: statement_index,
        });
    };

    // read = 0: the cursor convention starts every decode at the buffer head,
    // and the final cursor value IS the consumed-byte count.
    push(SelectedInstructionKind::WriteRuntimeStorageInteger {
        target_region: read_place.region,
        byte_offset: read_place.byte_offset,
        byte_size: read_place.byte_count,
        value: 0,
    });
    // ok = true: the flag is sticky -- every wire read ANDs its own success
    // bit in, so the first failure wins.
    push(SelectedInstructionKind::WriteRuntimeStorageInteger {
        target_region: ok_place.region,
        byte_offset: ok_place.byte_offset,
        byte_size: ok_place.byte_count,
        value: 1,
    });

    let expected_byte_kind = |byte: u8| SelectedInstructionKind::ReadWireExpectedByte {
        buffer_region: buffer_place.region,
        buffer_offset: buffer_place.byte_offset,
        buffer_length,
        read_region: read_place.region,
        read_offset: read_place.byte_offset,
        ok_region: ok_place.region,
        ok_offset: ok_place.byte_offset,
        expected: byte,
    };

    // The era discriminator must equal the CURRENT era byte for byte;
    // anything else is a different era (or garbage) and fails the decode.
    for byte in wire_varint_bytes(era) {
        push(expected_byte_kind(byte));
    }

    let scalar_read_kind = |place: &RuntimeStoragePlace, encoding: &WireScalarEncoding| {
        SelectedInstructionKind::ReadWireScalarVarint {
            buffer_region: buffer_place.region,
            buffer_offset: buffer_place.byte_offset,
            buffer_length,
            read_region: read_place.region,
            read_offset: read_place.byte_offset,
            ok_region: ok_place.region,
            ok_offset: ok_place.byte_offset,
            target_region: place.region,
            target_offset: place.byte_offset,
            byte_size: encoding.byte_size,
            zigzag: encoding.zigzag,
        }
    };

    for field in &fields {
        for byte in wire_varint_bytes(field.number as u64) {
            push(expected_byte_kind(byte));
        }
        match &field.content {
            WireReadContent::Scalar { encoding, place } => {
                push(scalar_read_kind(place, encoding));
            }
            WireReadContent::ByteSlice { place } => {
                push(SelectedInstructionKind::ReadWireByteSlice {
                    buffer_region: buffer_place.region,
                    buffer_offset: buffer_place.byte_offset,
                    buffer_length,
                    read_region: read_place.region,
                    read_offset: read_place.byte_offset,
                    ok_region: ok_place.region,
                    ok_offset: ok_place.byte_offset,
                    target_region: place.region,
                    target_offset: place.byte_offset,
                });
            }
            WireReadContent::Nested { children } => {
                // LENGTH varint into the end-bound slot, then OPEN turns it
                // into the absolute sub-region bound (and bounds-checks it),
                // the child's fields decode WITHOUT an era discriminator, and
                // CLOSE fails ok unless the cursor landed exactly on the
                // bound.
                push(SelectedInstructionKind::ReadWireScalarVarint {
                    buffer_region: buffer_place.region,
                    buffer_offset: buffer_place.byte_offset,
                    buffer_length,
                    read_region: read_place.region,
                    read_offset: read_place.byte_offset,
                    ok_region: ok_place.region,
                    ok_offset: ok_place.byte_offset,
                    target_region: RuntimeStorageRegion::RuntimeFrame,
                    target_offset: end_offset,
                    byte_size: 8,
                    zigzag: false,
                });
                push(SelectedInstructionKind::ReadWireNestedOpen {
                    buffer_region: buffer_place.region,
                    buffer_offset: buffer_place.byte_offset,
                    buffer_length,
                    read_region: read_place.region,
                    read_offset: read_place.byte_offset,
                    ok_region: ok_place.region,
                    ok_offset: ok_place.byte_offset,
                    end_region: RuntimeStorageRegion::RuntimeFrame,
                    end_offset,
                });
                for child in children {
                    for byte in wire_varint_bytes(child.number as u64) {
                        push(expected_byte_kind(byte));
                    }
                    let WireReadContent::Scalar { encoding, place } = &child.content else {
                        unreachable!("collection admits only scalar children");
                    };
                    push(scalar_read_kind(place, encoding));
                }
                push(SelectedInstructionKind::ReadWireNestedClose {
                    buffer_region: buffer_place.region,
                    buffer_offset: buffer_place.byte_offset,
                    read_region: read_place.region,
                    read_offset: read_place.byte_offset,
                    ok_region: ok_place.region,
                    ok_offset: ok_place.byte_offset,
                    end_region: RuntimeStorageRegion::RuntimeFrame,
                    end_offset,
                });
            }
            WireReadContent::Repeated {
                element,
                base,
                count,
                max_count,
            } => {
                // Byte-LENGTH varint into the end-bound slot, OPEN bounds it
                // against the buffer (a hostile length cannot wrap the
                // 64-bit sum or run past the buffer), the count companion
                // zeroes, then `max_count` unrolled guarded reads -- each
                // runs only while the cursor sits below the bound, so the
                // element count is capped by the declared maximum -- and
                // CLOSE fails ok unless the cursor landed exactly on the
                // bound (a payload packing more than `max_count` elements
                // leaves the cursor short and rejects).
                push(SelectedInstructionKind::ReadWireScalarVarint {
                    buffer_region: buffer_place.region,
                    buffer_offset: buffer_place.byte_offset,
                    buffer_length,
                    read_region: read_place.region,
                    read_offset: read_place.byte_offset,
                    ok_region: ok_place.region,
                    ok_offset: ok_place.byte_offset,
                    target_region: RuntimeStorageRegion::RuntimeFrame,
                    target_offset: end_offset,
                    byte_size: 8,
                    zigzag: false,
                });
                push(SelectedInstructionKind::ReadWireNestedOpen {
                    buffer_region: buffer_place.region,
                    buffer_offset: buffer_place.byte_offset,
                    buffer_length,
                    read_region: read_place.region,
                    read_offset: read_place.byte_offset,
                    ok_region: ok_place.region,
                    ok_offset: ok_place.byte_offset,
                    end_region: RuntimeStorageRegion::RuntimeFrame,
                    end_offset,
                });
                push(SelectedInstructionKind::WriteRuntimeStorageInteger {
                    target_region: count.region,
                    byte_offset: count.byte_offset,
                    byte_size: count.byte_count,
                    value: 0,
                });
                for index in 0..*max_count {
                    push(SelectedInstructionKind::ReadWireRepeatedScalarVarint {
                        buffer_region: buffer_place.region,
                        buffer_offset: buffer_place.byte_offset,
                        buffer_length,
                        read_region: read_place.region,
                        read_offset: read_place.byte_offset,
                        ok_region: ok_place.region,
                        ok_offset: ok_place.byte_offset,
                        end_region: RuntimeStorageRegion::RuntimeFrame,
                        end_offset,
                        count_region: count.region,
                        count_offset: count.byte_offset,
                        target_region: base.region,
                        target_offset: base.byte_offset + index * element.byte_size,
                        byte_size: element.byte_size,
                        zigzag: element.zigzag,
                    });
                }
                push(SelectedInstructionKind::ReadWireNestedClose {
                    buffer_region: buffer_place.region,
                    buffer_offset: buffer_place.byte_offset,
                    read_region: read_place.region,
                    read_offset: read_place.byte_offset,
                    ok_region: ok_place.region,
                    ok_offset: ok_place.byte_offset,
                    end_region: RuntimeStorageRegion::RuntimeFrame,
                    end_offset,
                });
            }
        }
    }

    true
}

/// Collect a schema's CURRENT-era fields in field-number order, resolving
/// each schema field name against the runtime value's matching member under
/// `receiver`. A nested message field (when `allow_nested`) recurses one
/// member deeper for its child schema with nesting disallowed -- validation
/// admits exactly one level, and the single end-bound slot matches.
fn collect_field_reads(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &mut ExpressionTable,
    receiver: ExpressionHandle,
    schema: &omega_checked_trees::wire::WireSchema,
    allow_nested: bool,
) -> Option<Vec<WireFieldRead>> {
    let mut fields = Vec::new();
    for member in input.program.wire_members(schema.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        if field.number < 0 {
            return None;
        }

        let member_handle = expressions.insert(ExpressionNode::Member(
            omega_checked_trees::expression::TableMemberExpression {
                receiver,
                member_symbol: SymbolHandle::invalid(),
                member: field.name.clone(),
                case_variant: None,
            },
        ));

        // A repeated field resolves the array member AND its count companion
        // (`<name>_count`); validation has guaranteed both exist with the
        // right shapes. Repeated fields live at the top level only -- a
        // child body admits plain scalars alone.
        if let Some(repeated) = input.program.wire_field_repeated_encoding(field) {
            if !allow_nested {
                return None;
            }
            let base = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                member_handle,
            )?;
            if base.byte_count != repeated.max_count * repeated.element.byte_size {
                return None;
            }
            let count_name = omega_checked_trees::wire::wire_repeated_count_field_name(
                field.name.as_str(),
            );
            let count_handle = expressions.insert(ExpressionNode::Member(
                omega_checked_trees::expression::TableMemberExpression {
                    receiver,
                    member_symbol: SymbolHandle::invalid(),
                    member: count_name.as_str().into(),
                    case_variant: None,
                },
            ));
            let count = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                count_handle,
            )?;
            if count.byte_count != 8 {
                return None;
            }
            fields.push(WireFieldRead {
                number: field.number,
                content: WireReadContent::Repeated {
                    element: repeated.element,
                    base,
                    count,
                    max_count: repeated.max_count,
                },
            });
            continue;
        }

        if let Some(child) = input.program.wire_field_nested_schema(field) {
            if !allow_nested {
                return None;
            }
            let children = collect_field_reads(
                input,
                dispatch_index,
                source_key,
                expressions,
                member_handle,
                child,
                false,
            )?;
            fields.push(WireFieldRead {
                number: field.number,
                content: WireReadContent::Nested { children },
            });
            continue;
        }

        // A borrowed `&[u8]` field decodes ZERO-COPY into its `{ptr, len}`
        // descriptor slot (16 bytes): the read op stores a view of the buffer.
        if input.program.is_borrowed_byte_slice(field.type_reference) {
            let place = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                member_handle,
            )?;
            if place.byte_count != input.runtime_abi.slice_descriptor_size() {
                return None;
            }
            fields.push(WireFieldRead {
                number: field.number,
                content: WireReadContent::ByteSlice { place },
            });
            continue;
        }

        let primitive = input.program.primitive_type_reference(field.type_reference)?;
        let encoding = WireScalarEncoding::for_primitive(primitive)?;
        let place = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            member_handle,
        )?;
        if place.byte_count != encoding.byte_size {
            return None;
        }

        fields.push(WireFieldRead {
            number: field.number,
            content: WireReadContent::Scalar { encoding, place },
        });
    }
    fields.sort_by_key(|field| field.number);
    Some(fields)
}

/// Copy one call argument into the scratch table, unwrapping the `&mut`
/// marker so the place expression underneath resolves directly.
fn copied_place_argument(
    input: &InstructionSelectionInput<'_>,
    expressions: &mut ExpressionTable,
    argument: ExpressionHandle,
) -> ExpressionHandle {
    let copied = expressions.copy_from(&input.program.expression_table, argument);
    match expressions.expression(copied) {
        ExpressionNode::Mutable(inner) => *inner,
        _ => copied,
    }
}
