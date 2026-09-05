use super::value_fields::{
    named_data_definition, validate_nested_value_field, validate_repeated_value_field,
};
use crate::places::declared_place_type;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use typed_trees::wire::{WireField, WireMember, WireSchema};

/// Validate the synthesized wire decoder call
/// `Schema::decode(&mut value, &buffer, &mut read, &mut verdict)` (chapter
/// 20, wire stage 2b).
///
/// The checks, mirroring the encoder's:
/// - exactly four arguments;
/// - every current-era schema field is a stage 2 scalar (i32/i64/u32/u64/
///   bool) or a NESTED MESSAGE with a scalar-only body -- strings (encode-
///   only) and repeated fields reject;
/// - the value argument's data type declares every schema field with the
///   SAME primitive type (a nested field's value member must be a data type
///   matching the CHILD schema's fields, one level down);
/// - the buffer is a fixed `[u8; N]` byte array (its compile-time length
///   bounds every runtime read -- the decoder never reads past it);
/// - `read` is `&mut u64` (receives the byte count consumed) and `ok` is
///   `&mut bool` (the success flag).
///
/// Places this scope cannot type skip their checks; instruction selection
/// re-resolves every place and an unresolved one surfaces as an
/// emission-planning blocker.
pub(super) fn validate_wire_decode_call(
    program: &TypedTrees,
    schema: &WireSchema,
    call: &typed_trees::statement::TableCall,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arguments = program.statement_table.expression_handles(call.arguments);
    if arguments.len() != 4 {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::decode` expects 4 arguments (&mut value, &buffer, &mut read, &mut verdict), got {}",
            schema.name,
            arguments.len()
        )));
        return;
    }

    // Schema side: the stage 2 scalar set plus nested message fields plus
    // repeated scalar fields and borrowed runtime-sized text, matching the
    // encoder's field set.
    let mut current_fields = Vec::new();
    let mut nested_fields: Vec<(&WireField, &WireSchema)> = Vec::new();
    let mut repeated_fields: Vec<(&WireField, typed_trees::wire::WireRepeatedEncoding)> =
        Vec::new();
    // Borrowed byte slices `&[u8]`: decoded ZERO-COPY as a length-prefixed view
    // of the buffer (no allocator or owned-copy policy required).
    let mut byte_slice_fields: Vec<&WireField> = Vec::new();
    let mut schema_rejects = false;
    for member in program.wire_members(schema.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        if field.relevance.is_erased() {
            continue;
        }
        // Mirrors the encoder's repeated gate: a bounded scalar carrier decodes as a
        // LENGTH-delimited packed payload (bounds-checked loop capped at the
        // declared capacity); non-scalar elements reject.
        if program.wire_field_repeated_carrier(field).is_some() {
            let Some(repeated) = program.wire_field_repeated_encoding(field) else {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{}` field `{}`: a repeated wire field's element must be a stage 2 scalar (i32, i64, u32, u64, bool); `{}` is not supported (repeated runtime-sized text and repeated nested messages reject until they have an honest encoding)",
                    schema.name,
                    field.name,
                    program.display_type_reference(field.type_reference)
                )));
                schema_rejects = true;
                continue;
            };
            repeated_fields.push((field, repeated));
            continue;
        }
        // A borrowed byte slice `&[u8]` decodes ZERO-COPY: a length-prefixed
        // view into the buffer, no owned copy and so no allocator. (An owned
        // An owned-copy destination would instead need allocator/package policy.)
        if program.is_borrowed_byte_slice(field.type_reference) {
            byte_slice_fields.push(field);
            continue;
        }
        if program
            .wire_field_borrowed_scalar_slice_encoding(field)
            .is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` field `{}` is a borrowed scalar slice: compact_binary can encode it from its runtime descriptor, but cannot decode packed varints into a borrowed view without owned/preallocated destination storage",
                schema.name, field.name
            )));
            schema_rejects = true;
            continue;
        }
        let primitive = program.primitive_type_reference(field.type_reference);
        let scalar = primitive.and_then(typed_trees::wire::WireScalarEncoding::for_primitive);
        let nested = program.wire_field_nested_schema(field);
        if scalar.is_none() && nested.is_none() {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` field `{}`: `{}` is not decodable by the compact_binary v0 decoder yet; wire stage 2b supports i32, i64, u32, u64, bool, borrowed runtime-sized text, and a sibling wire schema (one nesting level)",
                schema.name,
                field.name,
                program.display_type_reference(field.type_reference)
            )));
            schema_rejects = true;
            continue;
        }
        if let Some(child) = nested {
            // Mirrors the encoder's nested gate: a scalar-only child body is
            // the one honest level today (runtime-sized text in the child and
            // a doubly-nested body would each need another staging/end-bound
            // story).
            if program.wire_schema_scalar_body_worst_case(child).is_none() {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{}` field `{}`: nested wire schema `{}` must contain only scalar fields (i32, i64, u32, u64, bool); runtime-sized text and doubly-nested message fields inside a nested message are not supported yet",
                    schema.name, field.name, child.name
                )));
                schema_rejects = true;
                continue;
            }
            nested_fields.push((field, child));
            continue;
        }
        current_fields.push((field, primitive.expect("encoding implies primitive")));
    }
    if schema_rejects {
        return;
    }

    // Value argument: its data type must declare every schema field with the
    // same primitive type.
    if let Some(value_type) =
        declared_place_type(program, current_machine, current_state, arguments[0])
        && let Some(value_data) = named_data_definition(program, value_type)
    {
        // Mirrors the encoder's case-bearing rejection: the schema field set
        // only describes scalar FIELD members, so a decoded sum or mixed
        // value's tag and payload would stay silently unwritten.
        if program
            .data_members(value_data)
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::decode` value type `{}` is case-bearing; wire decoding into sums and mixed data shapes is not implemented yet (the case tag and payload have no schema spelling)",
                schema.name, value_data.name
            )));
            return;
        }
        for (field, schema_primitive) in &current_fields {
            let Some(value_field) =
                program
                    .data_members(value_data)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Field(data_field)
                            if data_field.name == field.name =>
                        {
                            Some(data_field)
                        }
                        _ => None,
                    })
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::decode` value type `{}` has no field `{}` to decode into (schema field {})",
                    schema.name, value_data.name, field.name, field.number
                )));
                continue;
            };
            if program.primitive_type_reference(value_field.type_reference)
                != Some(*schema_primitive)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::decode` value field `{}.{}` is `{}`, but the schema declares field {} as `{}`",
                    schema.name,
                    value_data.name,
                    field.name,
                    program.display_type_reference(value_field.type_reference),
                    field.number,
                    program.display_type_reference(field.type_reference)
                )));
            }
            // Decode ESTABLISHES a declared range from hostile bytes. The
            // native and interpreter reads carry the normalized interval and
            // leave the prior field value untouched while clearing the
            // verdict when the decoded scalar falls outside it. Refuse here
            // only if a declared range somehow survived the ordinary range
            // validator without a constant normalized interval.
            if typed_trees::wire::type_reference_carries_range(program, value_field.type_reference)
                && typed_trees::wire::scalar_decode_range(program, value_field.type_reference)
                    .is_none()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::decode` value field `{}.{}` declares a range fact (`{}`) \
                     that cannot be normalized into a constant scalar interval",
                    schema.name,
                    value_data.name,
                    field.name,
                    program.display_type_reference(value_field.type_reference)
                )));
            }
        }
        // A `&[u8]` schema field decodes into a `&[u8]` value field (a zero-copy
        // view of the buffer); the value's data field must match.
        for field in &byte_slice_fields {
            let Some(value_field) =
                program
                    .data_members(value_data)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Field(data_field)
                            if data_field.name == field.name =>
                        {
                            Some(data_field)
                        }
                        _ => None,
                    })
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::decode` value type `{}` has no field `{}` to decode into (schema field {})",
                    schema.name, value_data.name, field.name, field.number
                )));
                continue;
            };
            if !program.is_borrowed_byte_slice(value_field.type_reference) {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::decode` value field `{}.{}` is `{}`, but the schema declares field {} as a borrowed `&[u8]` (zero-copy view); the value field must also be `&[u8]`",
                    schema.name,
                    value_data.name,
                    field.name,
                    program.display_type_reference(value_field.type_reference),
                    field.number
                )));
            }
        }
        for (field, child) in &nested_fields {
            validate_nested_value_field(
                program,
                schema,
                "decode",
                value_data,
                field,
                child,
                diagnostics,
            );
        }
        for (field, repeated) in &repeated_fields {
            validate_repeated_value_field(
                program,
                schema,
                "decode",
                value_data,
                field,
                *repeated,
                diagnostics,
            );
        }
    }

    // Buffer argument: a fixed `[u8; N]` byte array (any length -- the
    // decoder bounds-checks every read against N at runtime).
    if let Some(buffer_type) =
        declared_place_type(program, current_machine, current_state, arguments[1])
        && !matches!(
            program.type_reference_table.type_reference(buffer_type),
            TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(_),
            } if program.primitive_type_reference(*element_type)
                == Some(typed_trees::types::PrimitiveType::U8)
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::decode` buffer argument must be `&[u8; N]`, got `{}`",
            schema.name,
            program.display_type_reference(buffer_type)
        )));
    }

    // Read argument: `&mut u64` (the consumed byte count).
    if let Some(read_type) =
        declared_place_type(program, current_machine, current_state, arguments[2])
        && !matches!(
            program.primitive_type_reference(read_type),
            Some(typed_trees::types::PrimitiveType::U64)
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::decode` read argument must be `&mut u64`, got `{}`",
            schema.name,
            program.display_type_reference(read_type)
        )));
    }

    // Verdict argument: `&mut WireVerdict` -- the dispatchable result enum
    // (case Invalid = tag 0 = the ZII default, so an unexamined verdict reads
    // as failure; case Sound = tag 1). Replaced the sticky `ok: bool` flag
    // 2026-07-02: a flag can be forgotten, an enum is dispatched.
    if let Some(verdict_type) =
        declared_place_type(program, current_machine, current_state, arguments[3])
        && !type_is_wire_verdict(program, verdict_type)
    {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::decode` verdict argument must be `&mut WireVerdict` (declare \
             `data WireVerdict {{ case Invalid; case Sound; }}` -- Invalid is the zero \
             case, so an untouched verdict reads as failure), got `{}`",
            schema.name,
            program.display_type_reference(verdict_type)
        )));
    }
}

/// The decode verdict contract: a data definition NAMED `WireVerdict` whose
/// members are exactly the cases `Invalid` then `Sound` (declaration order is
/// the tag order -- Invalid must be the ZII zero case).
fn type_is_wire_verdict(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    let TypeReferenceNode::Named { name, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    if name.as_str() != "WireVerdict" {
        return false;
    }
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "WireVerdict")
    else {
        return false;
    };
    let members = program.data_members(data);
    let case_names: Vec<&str> = members
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Variant(variant) if variant.payload.is_empty() => {
                Some(variant.name.as_str())
            }
            _ => None,
        })
        .collect();
    members.len() == 2 && case_names == ["Invalid", "Sound"]
}
