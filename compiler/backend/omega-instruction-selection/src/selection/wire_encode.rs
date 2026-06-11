//! Selection of the synthesized wire encoder call (chapter 20, wire stage
//! 2a): `Schema::encode_wire(&value, &mut out, &mut written)` lowers into a
//! straight-line sequence of wire-append operations -- zero the cursor, emit
//! the CURRENT era discriminator varint, then per field in field-number order
//! a field-number varint (compile-time bytes) and a value varint (runtime
//! scalar). Front-end validation (`omega-validation::wire`) has already
//! guaranteed the call shape, the field coverage, the stage 2a scalar set,
//! and the out-buffer capacity, so an unresolvable place here is a planning
//! blocker rather than a silent skip.

use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{SelectedInstruction, SelectedInstructionKind};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::statement::StatementNode;
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_checked_trees::wire::{WireMember, WireScalarEncoding, wire_varint_bytes};

use super::storage_places::{RuntimeStoragePlace, resolve_runtime_storage_place_in_table};

/// One field of the CURRENT era, ready to append: its tag bytes and the
/// resolved runtime scalar place.
struct WireFieldAppend {
    number: i64,
    encoding: WireScalarEncoding,
    place: RuntimeStoragePlace,
}

/// Lower a recognized `encode_wire` call statement; `true` when the statement
/// produced its append sequence (the emission planner checks for the wire
/// appends when it exempts the call from the unlowered-call blockers).
pub(super) fn select_wire_encode_call(
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
    let Some(schema) = input.program.wire_encode_call_schema(call) else {
        return false;
    };

    let [value_argument, out_argument, written_argument] = input
        .program
        .statement_table
        .expression_handles(call.arguments)
    else {
        return false;
    };

    // Copy the three argument expressions into a scratch table so the
    // per-field member reads can be synthesized next to them.
    let mut expressions = ExpressionTable::with_expression_capacity(8);
    let value_root = copied_place_argument(input, &mut expressions, *value_argument);
    let out_root = copied_place_argument(input, &mut expressions, *out_argument);
    let written_root = copied_place_argument(input, &mut expressions, *written_argument);

    let Some(out_place) =
        resolve_runtime_storage_place_in_table(input, dispatch_index, source_key, &expressions, out_root)
    else {
        return false;
    };
    let Some(written_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        &expressions,
        written_root,
    ) else {
        return false;
    };
    if written_place.byte_count != 8 {
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

        fields.push(WireFieldAppend {
            number: field.number,
            encoding,
            place,
        });
    }
    fields.sort_by_key(|field| field.number);

    let era = input.program.wire_schema_current_era(schema);

    let mut push = |kind: SelectedInstructionKind| {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key,
            source_statement: statement_index,
        });
    };

    // written = 0: the cursor convention starts every encode at the buffer
    // head, and the final cursor value IS the written-byte count.
    push(SelectedInstructionKind::WriteRuntimeStorageInteger {
        target_region: written_place.region,
        byte_offset: written_place.byte_offset,
        byte_size: written_place.byte_count,
        value: 0,
    });

    for byte in wire_varint_bytes(era) {
        push(SelectedInstructionKind::AppendWireLiteralByte {
            out_region: out_place.region,
            out_offset: out_place.byte_offset,
            written_region: written_place.region,
            written_offset: written_place.byte_offset,
            value: byte,
        });
    }

    for field in &fields {
        for byte in wire_varint_bytes(field.number as u64) {
            push(SelectedInstructionKind::AppendWireLiteralByte {
                out_region: out_place.region,
                out_offset: out_place.byte_offset,
                written_region: written_place.region,
                written_offset: written_place.byte_offset,
                value: byte,
            });
        }
        push(SelectedInstructionKind::AppendWireScalarVarint {
            source_region: field.place.region,
            source_offset: field.place.byte_offset,
            byte_size: field.encoding.byte_size,
            zigzag: field.encoding.zigzag,
            out_region: out_place.region,
            out_offset: out_place.byte_offset,
            written_region: written_place.region,
            written_offset: written_place.byte_offset,
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
