//! Selection of the synthesized wire decoder call (chapter 20, wire stage
//! 2b): `Schema::decode_wire(&mut value, &buffer, &mut read, &mut ok)` lowers
//! into a straight-line sequence of wire-read operations -- zero the cursor,
//! set the sticky ok flag, expect the CURRENT era discriminator bytes, then
//! per field in field-number order the expected field-number varint bytes
//! (compile-time) and a value varint read into the field's storage. Only the
//! current era decodes; a payload carrying any other era fails on its first
//! discriminator byte (historical eras await the stage 3 `Versioned<T>`
//! container). Front-end validation (`omega-validation::wire`) has already
//! guaranteed the call shape, the field coverage, and the stage 2 scalar set,
//! so an unresolvable place here is a planning blocker rather than a silent
//! skip.

use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{SelectedInstruction, SelectedInstructionKind};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::statement::StatementNode;
use omega_checked_trees::wire::{WireMember, WireScalarEncoding, wire_varint_bytes};
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;

use super::storage_places::{RuntimeStoragePlace, resolve_runtime_storage_place_in_table};

/// One field of the CURRENT era, ready to read: its expected tag bytes and
/// the resolved runtime scalar target place.
struct WireFieldRead {
    number: i64,
    encoding: WireScalarEncoding,
    place: RuntimeStoragePlace,
}

/// Lower a recognized `decode_wire` call statement; `true` when the statement
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
    // schema field name against the runtime value's matching member.
    let mut fields = Vec::new();
    for member in input.program.wire_members(schema.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        let Some(primitive) = input.program.primitive_type_reference(field.type_reference) else {
            return false;
        };
        let Some(encoding) = WireScalarEncoding::for_primitive(primitive) else {
            return false;
        };
        if field.number < 0 {
            return false;
        }

        let member_handle = expressions.insert(ExpressionNode::Member(
            omega_checked_trees::expression::TableMemberExpression {
                receiver: value_root,
                member_symbol: SymbolHandle::invalid(),
                member: field.name.clone(),
            },
        ));
        let Some(place) = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            source_key,
            &expressions,
            member_handle,
        ) else {
            return false;
        };
        if place.byte_count != encoding.byte_size {
            return false;
        }

        fields.push(WireFieldRead {
            number: field.number,
            encoding,
            place,
        });
    }
    fields.sort_by_key(|field| field.number);

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

    for field in &fields {
        for byte in wire_varint_bytes(field.number as u64) {
            push(expected_byte_kind(byte));
        }
        push(SelectedInstructionKind::ReadWireScalarVarint {
            buffer_region: buffer_place.region,
            buffer_offset: buffer_place.byte_offset,
            buffer_length,
            read_region: read_place.region,
            read_offset: read_place.byte_offset,
            ok_region: ok_place.region,
            ok_offset: ok_place.byte_offset,
            target_region: field.place.region,
            target_offset: field.place.byte_offset,
            byte_size: field.encoding.byte_size,
            zigzag: field.encoding.zigzag,
        });
    }

    true
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
