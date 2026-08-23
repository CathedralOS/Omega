//! Selection of the synthesized wire encoder call (chapter 20, wire stage
//! 2a): `Schema::encode(&value, &mut out, &mut written)` lowers into a
//! straight-line sequence of wire-append operations -- zero the cursor, emit
//! the CURRENT era discriminator varint, then per field in field-number order
//! a field-number varint (compile-time bytes) and a value varint (runtime
//! scalar). A runtime-sized borrowed text field (at most one, encoding
//! LAST) lowers to a text-bytes append instead: its length varint plus a
//! byte-copy bounded by the out buffer's compile-time capacity.
//!
//! A NESTED MESSAGE field (a field whose type is a sibling wire schema)
//! encodes as tag + LENGTH varint + the sub-message's fields WITHOUT an era
//! discriminator (decision 10: the era rides only the top-level envelope).
//! The length is runtime-sized (varints), so the sub-message stages through
//! the planner-reserved wire scratch region first: the scratch holds a
//! `{ptr @ +0, len @ +8}` text descriptor whose ptr is written to point at
//! the staging buffer at +16 and whose len slot doubles as the staging
//! CURSOR. The child's fields append into the staging buffer through the
//! ordinary wire appends (cursor in the len slot, capacity guaranteed by the
//! scratch's worst-case size), and one text-bytes append then replays the
//! descriptor into the caller's out buffer -- length varint + bounded
//! byte-copy, exactly the nested framing.
//!
//! Front-end validation (`psi-validation::wire`) has already guaranteed
//! the call shape, the field coverage, the stage 2 field set (scalar-only
//! children, one nesting level), and the out-buffer capacity for everything
//! but the runtime-sized text content, so an unresolvable place here is a
//! planning blocker rather than a silent skip.

use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, SelectedInstruction, SelectedInstructionKind,
};
use omega_control_flow::StateKey;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_checked_trees::statement::StatementNode;
use psi_checked_trees::wire::{WireFieldEncoding, WireMember, WirePlacement, wire_varint_bytes};
use psi_symbols::SymbolHandle;

use super::storage_places::{RuntimeStoragePlace, resolve_runtime_storage_place_in_table};

/// One field of the CURRENT era, ready to append.
struct WireFieldAppend {
    number: u64,
    content: WireFieldContent,
}

/// What follows a field's tag varint: a scalar/text place, a nested
/// sub-message's own field list (scalar places resolved one level down), or
/// a repeated field's packed element run.
enum WireFieldContent {
    /// A scalar slot, or a text descriptor for `String`.
    Direct {
        encoding: WireFieldEncoding,
        place: RuntimeStoragePlace,
    },
    /// A nested message: the child's fields in field-number order, staged
    /// through the wire scratch region before the length-prefixed replay.
    Nested {
        /// The child schema (its own plan supplies the child tags).
        schema: SymbolHandle,
        children: Vec<WireFieldAppend>,
    },
    /// A bounded repeated field's packed element varints, staged through the
    /// wire scratch before replay. Fixed arrays append every element;
    /// FixedVec guards each unrolled index against its intrinsic length.
    Repeated {
        carrier: psi_checked_trees::wire::WireRepeatedCarrier,
        element: psi_checked_trees::wire::WireScalarEncoding,
        base: RuntimeStoragePlace,
        count: Option<RuntimeStoragePlace>,
        max_count: usize,
    },
    /// An unbounded borrowed scalar slice. The source place is its
    /// `{ptr, element_count}` descriptor; one dynamic operation retains and
    /// realizes the plan's two-pass work/capacity obligation.
    ScalarSlice {
        element: psi_checked_trees::wire::WireScalarEncoding,
        descriptor: RuntimeStoragePlace,
    },
}

/// Lower a recognized `encode` call statement; `true` when the statement
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

    let Some(out_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        &expressions,
        out_root,
    ) else {
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

    // A `String` member resolves to its `{ptr, len}` text descriptor slot.
    let text_descriptor_size = input.runtime_abi.text_descriptor().total_size();

    // Collect the CURRENT era's fields in field-number order, resolving each
    // schema field name against the runtime value's matching member (nested
    // message fields resolve their CHILD schema's fields one member deeper).
    let Some(fields) = collect_field_appends(
        input,
        dispatch_index,
        source_key,
        &mut expressions,
        value_root,
        schema,
        true,
        text_descriptor_size,
    ) else {
        return false;
    };

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
    push(
        crate::selection::runtime_dispatch::write_place_integer_direct(
            written_place.region,
            written_place.byte_offset,
            0,
            written_place.byte_count,
        ),
    );

    for byte in wire_varint_bytes(era) {
        push(SelectedInstructionKind::AppendWireLiteralByte {
            out_region: out_place.region,
            out_offset: out_place.byte_offset,
            written_region: written_place.region,
            written_offset: written_place.byte_offset,
            value: byte,
        });
    }

    // MINT ARC RUNG 2a: the derived wire plan drives the tag bytes. The plan
    // pass mirrors this walk exactly, so per-field agreement is asserted; a
    // schema WITHOUT a plan (the pass was more conservative than the codec)
    // falls back to the schema walk unchanged. Nested CHILD tags remain
    // schema-driven in this rung (each child schema carries its own plan,
    // exercised when it is encoded top-level).
    let plan = input.program.wire_schema_plan(schema.symbol);
    if let Some(placements) = plan {
        let agrees = placements.len() == fields.len()
            && placements
                .iter()
                .zip(fields.iter())
                .all(|(placement, field)| {
                    let field_is_varint = matches!(
                        field.content,
                        WireFieldContent::Direct {
                            encoding: WireFieldEncoding::Scalar(_),
                            ..
                        }
                    );
                    placement.tag() == field.number
                        && matches!(placement, WirePlacement::Varint { .. }) == field_is_varint
                });
        if !agrees {
            debug_assert!(false, "derived wire plan disagrees with the schema walk");
            return false;
        }
    }

    for (field_index, field) in fields.iter().enumerate() {
        let tag = plan
            .and_then(|placements| placements.get(field_index))
            .map(|placement| placement.tag())
            .unwrap_or(field.number);
        for byte in wire_varint_bytes(tag) {
            push(SelectedInstructionKind::AppendWireLiteralByte {
                out_region: out_place.region,
                out_offset: out_place.byte_offset,
                written_region: written_place.region,
                written_offset: written_place.byte_offset,
                value: byte,
            });
        }
        match &field.content {
            WireFieldContent::Direct {
                encoding: WireFieldEncoding::Scalar(scalar),
                place,
            } => {
                push(SelectedInstructionKind::AppendWireScalarVarint {
                    source_region: place.region,
                    source_offset: place.byte_offset,
                    byte_size: scalar.byte_size,
                    zigzag: scalar.zigzag,
                    out_region: out_place.region,
                    out_offset: out_place.byte_offset,
                    written_region: written_place.region,
                    written_offset: written_place.byte_offset,
                });
            }
            WireFieldContent::Direct {
                encoding: WireFieldEncoding::Text,
                place,
            } => {
                // Length varint + raw bytes from the descriptor; the
                // byte-copy bounds every store against the out buffer's
                // compile-time byte length (the one runtime-sized append).
                push(SelectedInstructionKind::AppendWireTextBytes {
                    source_region: place.region,
                    source_offset: place.byte_offset,
                    out_region: out_place.region,
                    out_offset: out_place.byte_offset,
                    out_length: out_place.byte_count,
                    written_region: written_place.region,
                    written_offset: written_place.byte_offset,
                });
            }
            WireFieldContent::Repeated {
                carrier,
                element,
                base,
                count,
                max_count,
            } => {
                // A repeated field packs LENGTH-delimited, exactly like a
                // nested message: stage the carrier's live elements into
                // wire scratch, then replay the run through the text-bytes
                // append. Fixed arrays stage all N; FixedVec guards each
                // unrolled index on `index < length`. The planner reserved
                // scratch from the same worst-case capacity math validation
                // budgeted with; drift blocks emission rather than overruns.
                let staging_worst = max_count * element.max_varint_length();
                let scratch_base = input.runtime_storage.wire_scratch_base;
                if scratch_base == 0 || input.runtime_storage.wire_scratch_size < 16 + staging_worst
                {
                    return false;
                }
                let staging_offset = scratch_base + 16;
                let cursor_offset = scratch_base + 8;

                push(
                    crate::selection::runtime_dispatch::write_place_address_direct(
                        RuntimeStorageRegion::RuntimeFrame,
                        staging_offset,
                        scratch_base,
                    ),
                );
                push(
                    crate::selection::runtime_dispatch::write_place_integer_direct(
                        RuntimeStorageRegion::RuntimeFrame,
                        cursor_offset,
                        0,
                        8,
                    ),
                );
                for index in 0..*max_count {
                    let source_offset = base.byte_offset + index * element.byte_size;
                    match carrier {
                        psi_checked_trees::wire::WireRepeatedCarrier::FixedArray => {
                            push(SelectedInstructionKind::AppendWireScalarVarint {
                                source_region: base.region,
                                source_offset,
                                byte_size: element.byte_size,
                                zigzag: element.zigzag,
                                out_region: RuntimeStorageRegion::RuntimeFrame,
                                out_offset: staging_offset,
                                written_region: RuntimeStorageRegion::RuntimeFrame,
                                written_offset: cursor_offset,
                            });
                        }
                        psi_checked_trees::wire::WireRepeatedCarrier::FixedVec => {
                            let count = count
                                .as_ref()
                                .expect("FixedVec repeated field carries its length place");
                            push(SelectedInstructionKind::AppendWireRepeatedScalarVarint {
                                source_region: base.region,
                                source_offset,
                                byte_size: element.byte_size,
                                zigzag: element.zigzag,
                                index: index as u64,
                                count_region: count.region,
                                count_offset: count.byte_offset,
                                out_region: RuntimeStorageRegion::RuntimeFrame,
                                out_offset: staging_offset,
                                written_region: RuntimeStorageRegion::RuntimeFrame,
                                written_offset: cursor_offset,
                            });
                        }
                    }
                }
                push(SelectedInstructionKind::AppendWireTextBytes {
                    source_region: RuntimeStorageRegion::RuntimeFrame,
                    source_offset: scratch_base,
                    out_region: out_place.region,
                    out_offset: out_place.byte_offset,
                    out_length: out_place.byte_count,
                    written_region: written_place.region,
                    written_offset: written_place.byte_offset,
                });
            }
            WireFieldContent::ScalarSlice {
                element,
                descriptor,
            } => {
                push(SelectedInstructionKind::AppendWireScalarSlice {
                    source_region: descriptor.region,
                    source_offset: descriptor.byte_offset,
                    element_byte_size: element.byte_size,
                    zigzag: element.zigzag,
                    out_region: out_place.region,
                    out_offset: out_place.byte_offset,
                    out_length: out_place.byte_count,
                    written_region: written_place.region,
                    written_offset: written_place.byte_offset,
                });
            }
            WireFieldContent::Nested {
                schema: child_schema,
                children,
            } => {
                // RUNG 2c: the CHILD schema's own plan supplies the child
                // tags -- same agreement discipline as the top level (children
                // are scalar-only by collection's gate, so every placement
                // must be a Varint at the child's number).
                let child_plan = input.program.wire_schema_plan(*child_schema);
                if let Some(placements) = child_plan {
                    let agrees = placements.len() == children.len()
                        && placements
                            .iter()
                            .zip(children.iter())
                            .all(|(placement, child)| {
                                placement.tag() == child.number
                                    && matches!(placement, WirePlacement::Varint { .. })
                            });
                    if !agrees {
                        debug_assert!(
                            false,
                            "derived child wire plan disagrees with the schema walk"
                        );
                        return false;
                    }
                }
                // The planner reserved the wire scratch from the same
                // worst-case math validation budgeted with; if the staging
                // buffer cannot hold this child's worst case, the plan
                // drifted -- block emission rather than overrun the frame.
                let staging_worst: usize = children
                    .iter()
                    .map(|child| {
                        let WireFieldContent::Direct {
                            encoding: WireFieldEncoding::Scalar(scalar),
                            ..
                        } = &child.content
                        else {
                            unreachable!("collection admits only scalar children");
                        };
                        wire_varint_bytes(child.number).len() + scalar.max_varint_length()
                    })
                    .sum();
                let scratch_base = input.runtime_storage.wire_scratch_base;
                if scratch_base == 0 || input.runtime_storage.wire_scratch_size < 16 + staging_worst
                {
                    return false;
                }
                let staging_offset = scratch_base + 16;
                let cursor_offset = scratch_base + 8;

                // Stage the sub-message: descriptor ptr -> staging buffer,
                // staging cursor (the descriptor's len slot) = 0, then the
                // child's fields through the ordinary appends. NO era varint
                // -- the era rides only the top-level envelope (decision 10).
                push(
                    crate::selection::runtime_dispatch::write_place_address_direct(
                        RuntimeStorageRegion::RuntimeFrame,
                        staging_offset,
                        scratch_base,
                    ),
                );
                push(
                    crate::selection::runtime_dispatch::write_place_integer_direct(
                        RuntimeStorageRegion::RuntimeFrame,
                        cursor_offset,
                        0,
                        8,
                    ),
                );
                for (child_index, child) in children.iter().enumerate() {
                    let child_tag = child_plan
                        .and_then(|placements| placements.get(child_index))
                        .map(|placement| placement.tag())
                        .unwrap_or(child.number);
                    for byte in wire_varint_bytes(child_tag) {
                        push(SelectedInstructionKind::AppendWireLiteralByte {
                            out_region: RuntimeStorageRegion::RuntimeFrame,
                            out_offset: staging_offset,
                            written_region: RuntimeStorageRegion::RuntimeFrame,
                            written_offset: cursor_offset,
                            value: byte,
                        });
                    }
                    let WireFieldContent::Direct {
                        encoding: WireFieldEncoding::Scalar(scalar),
                        place,
                    } = &child.content
                    else {
                        unreachable!("collection admits only scalar children");
                    };
                    push(SelectedInstructionKind::AppendWireScalarVarint {
                        source_region: place.region,
                        source_offset: place.byte_offset,
                        byte_size: scalar.byte_size,
                        zigzag: scalar.zigzag,
                        out_region: RuntimeStorageRegion::RuntimeFrame,
                        out_offset: staging_offset,
                        written_region: RuntimeStorageRegion::RuntimeFrame,
                        written_offset: cursor_offset,
                    });
                }

                // Replay the staged sub-message into the caller's out buffer:
                // the text-bytes append reads the scratch descriptor and
                // emits LENGTH varint + length raw bytes -- exactly the
                // nested-message framing.
                push(SelectedInstructionKind::AppendWireTextBytes {
                    source_region: RuntimeStorageRegion::RuntimeFrame,
                    source_offset: scratch_base,
                    out_region: out_place.region,
                    out_offset: out_place.byte_offset,
                    out_length: out_place.byte_count,
                    written_region: written_place.region,
                    written_offset: written_place.byte_offset,
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
/// admits exactly one level, and the single staging region matches.
#[allow(clippy::too_many_arguments)]
fn collect_field_appends(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &mut ExpressionTable,
    receiver: ExpressionHandle,
    schema: &psi_checked_trees::wire::WireSchema,
    allow_nested: bool,
    text_descriptor_size: usize,
) -> Option<Vec<WireFieldAppend>> {
    let mut fields = Vec::new();
    for member in input.program.wire_members(schema.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        if field.relevance.is_erased() {
            continue;
        }
        let member_handle = expressions.insert(ExpressionNode::Member(
            psi_checked_trees::expression::TableMemberExpression {
                receiver,
                member_symbol: SymbolHandle::invalid(),
                member: field.name.clone(),
                case_variant: None,
            },
        ));

        // A repeated field resolves either the exactly-full array member or
        // a FixedVec's inline `items` plus its own `length`.
        if let Some(repeated) = input.program.wire_field_repeated_encoding(field) {
            if !allow_nested {
                return None;
            }
            let (base_handle, count_handle) = match repeated.carrier {
                psi_checked_trees::wire::WireRepeatedCarrier::FixedArray => (member_handle, None),
                psi_checked_trees::wire::WireRepeatedCarrier::FixedVec => {
                    let items = expressions.insert(ExpressionNode::Member(
                        psi_checked_trees::expression::TableMemberExpression {
                            receiver: member_handle,
                            member_symbol: SymbolHandle::invalid(),
                            member: "items".into(),
                            case_variant: None,
                        },
                    ));
                    let length = expressions.insert(ExpressionNode::Member(
                        psi_checked_trees::expression::TableMemberExpression {
                            receiver: member_handle,
                            member_symbol: SymbolHandle::invalid(),
                            member: "length".into(),
                            case_variant: None,
                        },
                    ));
                    (items, Some(length))
                }
            };
            let base = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                base_handle,
            )?;
            if base.byte_count != repeated.max_count * repeated.element.byte_size {
                return None;
            }
            let count = match count_handle {
                Some(handle) => Some(resolve_runtime_storage_place_in_table(
                    input,
                    dispatch_index,
                    source_key,
                    expressions,
                    handle,
                )?),
                None => None,
            };
            if count.as_ref().is_some_and(|count| count.byte_count != 8) {
                return None;
            }
            fields.push(WireFieldAppend {
                number: field.number,
                content: WireFieldContent::Repeated {
                    carrier: repeated.carrier,
                    element: repeated.element,
                    base,
                    count,
                    max_count: repeated.max_count,
                },
            });
            continue;
        }

        if let Some(slice) = input
            .program
            .wire_field_borrowed_scalar_slice_encoding(field)
        {
            if !allow_nested {
                return None;
            }
            let descriptor = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                member_handle,
            )?;
            if descriptor.byte_count != input.runtime_abi.slice_descriptor_size() {
                return None;
            }
            fields.push(WireFieldAppend {
                number: field.number,
                content: WireFieldContent::ScalarSlice {
                    element: slice.element,
                    descriptor,
                },
            });
            continue;
        }

        if let Some(child) = input.program.wire_field_nested_schema(field) {
            if !allow_nested {
                return None;
            }
            let children = collect_field_appends(
                input,
                dispatch_index,
                source_key,
                expressions,
                member_handle,
                child,
                false,
                text_descriptor_size,
            )?;
            // A nested child is scalar-only (validation's gate); a String or
            // doubly-nested child reaching here is a planning blocker.
            if children.iter().any(|child_field| {
                !matches!(
                    child_field.content,
                    WireFieldContent::Direct {
                        encoding: WireFieldEncoding::Scalar(_),
                        ..
                    }
                )
            }) {
                return None;
            }
            fields.push(WireFieldAppend {
                number: field.number,
                content: WireFieldContent::Nested {
                    schema: child.symbol,
                    children,
                },
            });
            continue;
        }

        // A borrowed `&[u8]` field encodes as RAW bytes (length + the bytes). It
        // is a fat `{ptr, len}` slice -- the same descriptor shape a String text
        // field uses -- so it lowers through the existing text-bytes append.
        let encoding = if input.program.is_borrowed_byte_slice(field.type_reference) {
            WireFieldEncoding::Text
        } else {
            let primitive = input
                .program
                .primitive_type_reference(field.type_reference)?;
            WireFieldEncoding::for_primitive(primitive)?
        };
        let place = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            member_handle,
        )?;
        let expected_byte_count = match encoding {
            WireFieldEncoding::Scalar(scalar) => scalar.byte_size,
            WireFieldEncoding::Text => text_descriptor_size,
        };
        if place.byte_count != expected_byte_count {
            return None;
        }

        fields.push(WireFieldAppend {
            number: field.number,
            content: WireFieldContent::Direct { encoding, place },
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
        ExpressionNode::Borrow(inner) => inner.target,
        _ => copied,
    }
}
