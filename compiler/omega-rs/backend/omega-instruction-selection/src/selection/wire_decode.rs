//! Selection of the synthesized wire decoder call (chapter 20, wire stage
//! 2b): `Schema::decode(&mut value, &buffer, &mut read, &mut verdict)` lowers
//! into a straight-line sequence of wire-read operations -- zero the cursor,
//! set the sticky ok flag, expect the CURRENT era discriminator bytes, then
//! per field in field-number order the expected field-number varint bytes
//! (compile-time) and a value varint read into the field's storage. Only the
//! current era decodes; a payload carrying any other era fails on its first
//! discriminator byte.
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
//! Front-end validation (`psi-validation::wire`) has already guaranteed
//! the call shape, the field coverage, and the stage 2 field set (scalar-only
//! children, one nesting level), so an unresolvable place here is a planning
//! blocker rather than a silent skip.

use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, SelectedInstruction, SelectedInstructionKind,
};
use omega_control_flow::StateKey;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_checked_trees::statement::StatementNode;
use psi_checked_trees::types::TypeReferenceHandle;
use psi_checked_trees::wire::{WireMember, WirePlacement, WireScalarEncoding, wire_varint_bytes};
use psi_symbols::SymbolHandle;

use super::storage_places::{RuntimeStoragePlace, resolve_runtime_storage_place_in_table};

/// One field of the CURRENT era, ready to read.
struct WireFieldRead {
    number: u64,
    content: WireReadContent,
}

/// What follows a field's expected tag bytes: a scalar target place, a
/// nested sub-message's own field list (resolved one member deeper), or a
/// repeated field's packed element run.
enum WireReadContent {
    Scalar {
        encoding: WireScalarEncoding,
        place: RuntimeStoragePlace,
        range: Option<psi_language_semantics::wire::WireScalarRange>,
    },
    Nested {
        /// The child schema (its own plan supplies the expected child tags).
        schema: SymbolHandle,
        children: Vec<WireFieldRead>,
    },
    /// A borrowed `&[u8]` field: a byte-LENGTH varint then a fat `{ptr, len}`
    /// descriptor viewing the buffer content, stored zero-copy into `place`.
    ByteSlice {
        place: RuntimeStoragePlace,
        predicate_mask: u8,
    },
    /// A repeated field: a byte-LENGTH varint opens a bounded sub-region,
    /// then up to `max_count` element reads. Fixed arrays require all N;
    /// FixedVec guards on the region end and bumps its intrinsic length. The
    /// exact-end close rejects payloads whose length disagrees with the
    /// elements -- including MORE elements than the carrier capacity.
    Repeated {
        carrier: psi_checked_trees::wire::WireRepeatedCarrier,
        element: WireScalarEncoding,
        base: RuntimeStoragePlace,
        count: Option<RuntimeStoragePlace>,
        max_count: usize,
        range: Option<psi_language_semantics::wire::WireScalarRange>,
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
    let Some(value_type) = declared_expression_type(
        input,
        machine,
        state,
        &input.program.expression_table,
        *value_argument,
    ) else {
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
    // The verdict is a `WireVerdict` enum: a 4-byte tag with Invalid = 0 and
    // Sound = 1, little-endian -- so the sticky mechanics are TAG-CORRECT as
    // they stand: the initial full-width write below stores Sound (1), and
    // every wire read's failure path ANDs the LOW byte to 0, which flips the
    // whole tag to Invalid. (The remaining tag bytes start 0 and stay 0.)
    if ok_place.byte_count != omega_layout::ENUM_TAG_BYTES {
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
        value_type,
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
    push(
        crate::selection::runtime_dispatch::write_place_integer_direct(
            read_place.region,
            read_place.byte_offset,
            0,
            read_place.byte_count,
        ),
    );
    // ok = true: the flag is sticky -- every wire read ANDs its own success
    // bit in, so the first failure wins.
    push(
        crate::selection::runtime_dispatch::write_place_integer_direct(
            ok_place.region,
            ok_place.byte_offset,
            1,
            ok_place.byte_count,
        ),
    );

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

    let scalar_read_kind =
        |place: &RuntimeStoragePlace,
         encoding: &WireScalarEncoding,
         range: Option<psi_language_semantics::wire::WireScalarRange>| {
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
                range,
            }
        };

    // MINT ARC RUNG 2a: the derived wire plan drives the EXPECTED tag bytes
    // (see wire_encode.rs -- same agreement assertion, same fallback).
    let plan = input.program.wire_schema_plan(schema.symbol);
    if let Some(placements) = plan {
        let agrees = placements.len() == fields.len()
            && placements
                .iter()
                .zip(fields.iter())
                .all(|(placement, field)| {
                    let field_is_varint = matches!(field.content, WireReadContent::Scalar { .. });
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
            push(expected_byte_kind(byte));
        }
        match &field.content {
            WireReadContent::Scalar {
                encoding,
                place,
                range,
            } => {
                push(scalar_read_kind(place, encoding, *range));
            }
            WireReadContent::ByteSlice {
                place,
                predicate_mask,
            } => {
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
                    predicate_mask: *predicate_mask,
                });
            }
            WireReadContent::Nested {
                schema: child_schema,
                children,
            } => {
                // RUNG 2c: the child schema's plan supplies the EXPECTED child
                // tags (see wire_encode.rs -- same agreement discipline).
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
                    range: None,
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
                for (child_index, child) in children.iter().enumerate() {
                    let child_tag = child_plan
                        .and_then(|placements| placements.get(child_index))
                        .map(|placement| placement.tag())
                        .unwrap_or(child.number);
                    for byte in wire_varint_bytes(child_tag) {
                        push(expected_byte_kind(byte));
                    }
                    let WireReadContent::Scalar {
                        encoding,
                        place,
                        range,
                    } = &child.content
                    else {
                        unreachable!("collection admits only scalar children");
                    };
                    push(scalar_read_kind(place, encoding, *range));
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
                carrier,
                element,
                base,
                count,
                max_count,
                range,
            } => {
                // Byte-LENGTH varint into the end-bound slot, OPEN bounds it
                // against the buffer (a hostile length cannot wrap the
                // 64-bit sum or run past the buffer), then `max_count`
                // unrolled reads. Fixed arrays require every element;
                // FixedVec guards each read on the region end and records the
                // result in its intrinsic length. CLOSE rejects any byte
                // length that disagrees with the carrier.
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
                    range: None,
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
                if let Some(count) = count {
                    push(
                        crate::selection::runtime_dispatch::write_place_integer_direct(
                            count.region,
                            count.byte_offset,
                            0,
                            count.byte_count,
                        ),
                    );
                }
                for index in 0..*max_count {
                    let target_offset = base.byte_offset + index * element.byte_size;
                    match carrier {
                        psi_checked_trees::wire::WireRepeatedCarrier::FixedArray => {
                            push(SelectedInstructionKind::ReadWireScalarVarint {
                                buffer_region: buffer_place.region,
                                buffer_offset: buffer_place.byte_offset,
                                buffer_length,
                                read_region: read_place.region,
                                read_offset: read_place.byte_offset,
                                ok_region: ok_place.region,
                                ok_offset: ok_place.byte_offset,
                                target_region: base.region,
                                target_offset,
                                byte_size: element.byte_size,
                                zigzag: element.zigzag,
                                range: *range,
                            });
                        }
                        psi_checked_trees::wire::WireRepeatedCarrier::FixedVec => {
                            let count = count
                                .as_ref()
                                .expect("FixedVec repeated field carries its length place");
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
                                target_offset,
                                byte_size: element.byte_size,
                                zigzag: element.zigzag,
                                range: *range,
                            });
                        }
                    }
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
    receiver_type: TypeReferenceHandle,
    schema: &psi_checked_trees::wire::WireSchema,
    allow_nested: bool,
) -> Option<Vec<WireFieldRead>> {
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
        let member_type = psi_checked_trees::wire::data_field_type(
            input.program,
            receiver_type,
            field.name.as_str(),
        )?;

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
            fields.push(WireFieldRead {
                number: field.number,
                content: WireReadContent::Repeated {
                    carrier: repeated.carrier,
                    element: repeated.element,
                    base,
                    count,
                    max_count: repeated.max_count,
                    range: psi_checked_trees::wire::repeated_element_type(
                        input.program,
                        member_type,
                        repeated.carrier,
                    )
                    .and_then(|element| {
                        psi_checked_trees::wire::scalar_decode_range(input.program, element)
                    }),
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
                member_type,
                child,
                false,
            )?;
            fields.push(WireFieldRead {
                number: field.number,
                content: WireReadContent::Nested {
                    schema: child.symbol,
                    children,
                },
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
            // Decode-boundary byte-domain obligations: the emitted sequence
            // validates the copied bytes (interp parity). An UNRECOGNIZED
            // domain fact set refuses selection -- silently decoding
            // without validation is the pinned utf8 soundness hole; the
            // emission planner reports the unlowered decode loudly.
            let mut predicate_mask = 0u8;
            for (_, predicate) in
                psi_checked_trees::byte_predicates::type_reference_domain_predicates(
                    input.program,
                    field.type_reference,
                )
            {
                predicate_mask |= predicate?.mask_bit();
            }
            fields.push(WireFieldRead {
                number: field.number,
                content: WireReadContent::ByteSlice {
                    place,
                    predicate_mask,
                },
            });
            continue;
        }

        let primitive = input
            .program
            .primitive_type_reference(field.type_reference)?;
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
            content: WireReadContent::Scalar {
                encoding,
                place,
                range: psi_checked_trees::wire::scalar_decode_range(input.program, member_type),
            },
        });
    }
    fields.sort_by_key(|field| field.number);
    Some(fields)
}

/// Resolve the declared type of the decode value expression. This deliberately
/// follows semantic declarations rather than runtime-layout descriptors: the
/// range fact lives on the authored destination field and must survive even
/// when storage planning has flattened that field into a frame slot.
fn declared_expression_type(
    input: &InstructionSelectionInput<'_>,
    machine: &psi_checked_trees::machine::Machine,
    state: &psi_checked_trees::state::State,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            declared_expression_type(input, machine, state, expressions, *inner)
        }
        ExpressionNode::Member(member) => {
            let receiver =
                declared_expression_type(input, machine, state, expressions, member.receiver)?;
            psi_checked_trees::wire::data_field_type(
                input.program,
                receiver,
                member.member.as_str(),
            )
        }
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            let root_name = members.first()?.as_str();
            let root_symbol = if path.head_symbol.is_valid() {
                path.head_symbol
            } else {
                path.symbol
            };
            let mut current = input
                .program
                .state_parameters(state)
                .iter()
                .find(|parameter| {
                    (root_symbol.is_valid() && parameter.symbol == root_symbol)
                        || parameter.name.as_str() == root_name
                })
                .map(|parameter| parameter.type_reference)
                .or_else(|| {
                    input
                        .program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .find_map(|statement| {
                            let StatementNode::LocalData(local) = statement else {
                                return None;
                            };
                            ((root_symbol.is_valid() && local.symbol == root_symbol)
                                || local.name.as_str() == root_name)
                                .then_some(local.type_reference)
                        })
                })
                .or_else(|| {
                    input
                        .program
                        .machine_owned_data(machine)
                        .iter()
                        .find(|owned| {
                            (root_symbol.is_valid() && owned.symbol == root_symbol)
                                || owned.name.as_str() == root_name
                        })
                        .map(|owned| owned.type_reference)
                })?;
            for member in members.iter().skip(1) {
                current = psi_checked_trees::wire::data_field_type(
                    input.program,
                    current,
                    member.as_str(),
                )?;
            }
            Some(current)
        }
        _ => None,
    }
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
