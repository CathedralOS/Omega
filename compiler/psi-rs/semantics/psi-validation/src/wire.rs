use crate::places::{declared_place_type, unwrapped_type_reference};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use psi_typed_trees::wire::{WireField, WireMember, WireSchema};

mod schema_validation;

#[cfg(test)]
use schema_validation::validate_nested_schema_cycles;
pub(crate) use schema_validation::validate_wire_schemas;

/// Validate a call whose receiver names a wire schema: the synthesized
/// `Schema::encode(&value, &mut out, &mut written)` encoder (wire stage
/// 2a) or `Schema::decode(&mut value, &buffer, &mut read, &mut verdict)`
/// decoder (wire stage 2b). Returns `true` when the receiver names a wire
/// schema (the call belongs to this module whether or not it validates).
pub(crate) fn validate_wire_schema_call(
    program: &TypedTrees,
    call: &psi_typed_trees::statement::TableCall,
    current_machine: &psi_typed_trees::machine::Machine,
    current_state: Option<&psi_typed_trees::state::State>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let receiver_members = program.statement_table.name_path_members(call.receiver);
    let [schema_name] = receiver_members else {
        return false;
    };
    let Some(schema) = program
        .wire_schemas()
        .iter()
        .find(|schema| schema.name.as_str() == schema_name.as_str())
    else {
        return false;
    };

    match call.target.as_str() {
        psi_typed_trees::wire::WIRE_ENCODE_MACHINE_NAME => {
            validate_wire_encode_call(
                program,
                schema,
                call,
                current_machine,
                current_state,
                diagnostics,
            );
        }
        psi_typed_trees::wire::WIRE_DECODE_MACHINE_NAME => {
            validate_wire_decode_call(
                program,
                schema,
                call,
                current_machine,
                current_state,
                diagnostics,
            );
        }
        // The pre-rename spellings (retired 2026-07-02): a guided error, not
        // a mystery "no machine" message.
        "encode_wire" | "decode_wire" => {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::{}` was renamed: the synthesized codec entries are `{}::encode(&value, \
                 &mut out, &mut written)` and `{}::decode(&mut value, &buffer, &mut read, \
                 &mut verdict)`",
                schema.name, call.target, schema.name, schema.name
            )));
        }
        _ => {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` has no machine `{}`; the compiler only synthesizes `encode(&value, &mut out, &mut written)` and `decode(&mut value, &buffer, &mut read, &mut verdict)` (wire stage 2)",
                schema.name, call.target
            )));
        }
    }
    true
}

/// Validate the synthesized wire encoder call
/// `Schema::encode(&value, &mut out, &mut written)` (chapter 20, wire
/// stage 2a).
///
/// The checks that make a bad program fail HERE, with a source-shaped
/// diagnostic, instead of in the backend:
/// - exactly three arguments;
/// - every current-era schema field is a stage 2a scalar (i32/i64/u32/u64/
///   bool), borrowed runtime-sized text, or a NESTED MESSAGE (a sibling wire schema whose body
///   is scalar-only -- runtime-text-in-child and nested-in-nested reject, one
///   honest level today); repeated fields reject;
/// - at most one runtime-sized text field, and it carries the highest field number so
///   it encodes LAST (its content is runtime-sized; see the worst-case
///   rule). The rule is PER MESSAGE SCOPE: nested fields are statically
///   bounded, so they may sit anywhere, and a child body has no runtime-sized text;
/// - the value argument's data type declares every schema field with the
///   SAME primitive type (a nested field's value member must be a data type
///   matching the CHILD schema's fields, one level down);
/// - the out buffer is `&mut [u8; N]` with room for the WORST-CASE encoding
///   (era varint + per field tag varint + max value varint; a text field
///   budgets tag + max length varint; a nested field budgets tag + length
///   varint + the child's static worst case), so every append EXCEPT the
///   trailing text byte-copy needs no runtime bounds check -- the
///   byte-copy alone bounds against N at runtime and truncates content past
///   capacity;
/// - the written argument is `&mut u64`.
///
/// Places this scope cannot type (alias-fed or computed arguments) skip the
/// value/out/written checks; instruction selection re-resolves every place
/// and an unresolved one surfaces as an emission-planning blocker.
fn validate_wire_encode_call(
    program: &TypedTrees,
    schema: &WireSchema,
    call: &psi_typed_trees::statement::TableCall,
    current_machine: &psi_typed_trees::machine::Machine,
    current_state: Option<&psi_typed_trees::state::State>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arguments = program.statement_table.expression_handles(call.arguments);
    if arguments.len() != 3 {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::encode` expects 3 arguments (&value, &mut out, &mut written), got {}",
            schema.name,
            arguments.len()
        )));
        return;
    }

    // Schema side: the stage 2a scalar set plus runtime-sized text plus nested
    // message fields, and the worst-case byte budget the out buffer must cover (era
    // varint + per-field tag varint + max value varint; a text field
    // budgets its tag + max length varint -- its CONTENT is runtime-sized, so
    // the emitted byte-copy is the one append that bounds-checks against the
    // buffer at runtime instead; a nested message field budgets its tag + the
    // sub-message's length varint + the sub-message's static worst case).
    let era = program.wire_schema_current_era(schema);
    let mut worst_case_bytes = psi_typed_trees::wire::wire_varint_bytes(era).len();
    let mut current_fields = Vec::new();
    let mut nested_fields: Vec<(&WireField, &WireSchema)> = Vec::new();
    let mut repeated_fields: Vec<(&WireField, psi_typed_trees::wire::WireRepeatedEncoding)> =
        Vec::new();
    let mut scalar_slice_fields: Vec<(
        &WireField,
        psi_typed_trees::wire::WireBorrowedScalarSliceEncoding,
    )> = Vec::new();
    let mut text_fields: Vec<&WireField> = Vec::new();
    // Borrowed byte slices `&[u8]`: the zero-copy RAW-bytes/text field. Encodes
    // as length varint + raw bytes, rides the same runtime-sized Text constraints
    // (at most one, encodes last), and matches a `&[u8]` value field.
    let mut byte_slice_fields: Vec<&WireField> = Vec::new();
    let mut max_field_number = 0u64;
    let mut schema_rejects = false;
    for member in program.wire_members(schema.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        if field.relevance.is_erased() {
            continue;
        }
        // A bounded repeated scalar carrier packs LENGTH-delimited: tag +
        // byte-length varint + back-to-back element varints (protobuf's
        // packed encoding). Its capacity bounds it statically, so it
        // joins the worst-case budget like a scalar and may sit anywhere.
        // Only scalar elements are honest today: a text element is runtime-sized
        // and a nested-message element would need per-element staging, so both
        // reject loudly.
        if let Some((_, _, max_count)) = program.wire_field_repeated_carrier(field) {
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
            let _ = max_count;
            max_field_number = max_field_number.max(field.number);
            worst_case_bytes += psi_typed_trees::wire::wire_varint_bytes(field.number).len()
                + repeated.worst_case_payload_bytes();
            repeated_fields.push((field, repeated));
            continue;
        }
        // A borrowed byte slice `&[u8]` encodes as RAW bytes (length varint +
        // the bytes), runtime-sized, so it joins
        // `text_fields` (at most one, must encode last) and matches a `&[u8]`
        // value field below. (A `[u8; N]` owned array is a repeated field,
        // handled above; only the borrowed slice reaches here.)
        if program.is_borrowed_byte_slice(field.type_reference) {
            max_field_number = max_field_number.max(field.number);
            worst_case_bytes += psi_typed_trees::wire::wire_varint_bytes(field.number).len()
                + psi_typed_trees::wire::WIRE_TEXT_LENGTH_MAX_VARINT_LENGTH;
            text_fields.push(field);
            byte_slice_fields.push(field);
            continue;
        }
        // A borrowed scalar slice is allocation-free but runtime-sized. Its
        // normalized plan retains the descriptor length, two-pass work, and
        // exact remaining-output-capacity obligation.
        if let Some(slice) = program.wire_field_borrowed_scalar_slice_encoding(field) {
            max_field_number = max_field_number.max(field.number);
            worst_case_bytes += psi_typed_trees::wire::wire_varint_bytes(field.number).len()
                + psi_typed_trees::wire::WIRE_TEXT_LENGTH_MAX_VARINT_LENGTH;
            text_fields.push(field);
            scalar_slice_fields.push((field, slice));
            continue;
        }
        let primitive = program.primitive_type_reference(field.type_reference);
        let encoding = primitive.and_then(psi_typed_trees::wire::WireFieldEncoding::for_primitive);
        let nested = program.wire_field_nested_schema(field);
        if encoding.is_none() && nested.is_none() {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` field `{}`: `{}` is not encodable by the compact_binary v0 encoder yet; wire stage 2 supports i32, i64, u32, u64, bool, runtime-sized text (`&[u8] in Utf8`), and a sibling wire schema (one nesting level)",
                schema.name,
                field.name,
                program.display_type_reference(field.type_reference)
            )));
            schema_rejects = true;
            continue;
        }
        max_field_number = max_field_number.max(field.number);
        let tag_bytes = psi_typed_trees::wire::wire_varint_bytes(field.number).len();
        if let Some(child) = nested {
            // A nested message field encodes as tag + LENGTH varint + the
            // sub-message's fields WITHOUT an era discriminator (decision 10:
            // the era rides only the top-level envelope). The whole framing
            // is statically bounded, so it joins the worst-case budget like a
            // scalar -- but only a scalar-only child body is bounded: text content
            // is runtime-sized and a doubly-nested body would need a second staging
            // region, so both reject (one honest level first).
            let Some(nested_worst) = program.wire_nested_field_worst_case(child) else {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{}` field `{}`: nested wire schema `{}` must contain only scalar fields (i32, i64, u32, u64, bool); runtime-sized text and doubly-nested message fields inside a nested message are not supported yet",
                    schema.name, field.name, child.name
                )));
                schema_rejects = true;
                continue;
            };
            worst_case_bytes += tag_bytes + nested_worst;
            nested_fields.push((field, child));
            continue;
        }
        let encoding = encoding.expect("nested handled above, so encoding is Some");
        worst_case_bytes += tag_bytes
            + match encoding {
                psi_typed_trees::wire::WireFieldEncoding::Scalar(scalar) => {
                    scalar.max_varint_length()
                }
                psi_typed_trees::wire::WireFieldEncoding::Text => {
                    text_fields.push(field);
                    psi_typed_trees::wire::WIRE_TEXT_LENGTH_MAX_VARINT_LENGTH
                }
            };
        current_fields.push((field, primitive.expect("encoding implies primitive")));
    }
    // A text field's byte count is runtime-sized, so every append AFTER it
    // would run with the compile-time capacity guarantee already spent. The
    // encoder therefore takes at most ONE runtime-sized text field, and it must
    // encode LAST (the highest field number); everything before it stays covered
    // by the worst-case budget, and the trailing byte-copy is runtime-bounded.
    if let [first_text, more_text @ ..] = text_fields.as_slice() {
        for field in more_text {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` field `{}`: the compact_binary v0 encoder supports at most one runtime-sized text field per message (text content is runtime-sized, so only the final field can be unbounded)",
                schema.name, field.name
            )));
            schema_rejects = true;
        }
        if more_text.is_empty() && first_text.number != max_field_number {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` field `{}`: a runtime-sized text field must carry the schema's highest field number so it encodes last; its byte count is runtime-sized, and any field after it would lose the compile-time out-buffer guarantee",
                schema.name, first_text.name
            )));
            schema_rejects = true;
        }
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
        // Case-bearing value types (sums and mixed shapes) have no wire
        // encoding yet: the schema field set only describes scalar FIELD
        // members, so the case tag and payload would silently drop. Reject
        // loudly until case-aware wire encoding lands.
        if program
            .data_members(value_data)
            .iter()
            .any(|member| matches!(member, psi_typed_trees::data::DataMember::Variant(_)))
        {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::encode` value type `{}` is case-bearing; wire encoding over sums and mixed data shapes is not implemented yet (the case tag and payload have no schema spelling)",
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
                        psi_typed_trees::data::DataMember::Field(data_field)
                            if data_field.name == field.name =>
                        {
                            Some(data_field)
                        }
                        _ => None,
                    })
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode` value type `{}` has no field `{}` to encode (schema field {})",
                    schema.name, value_data.name, field.name, field.number
                )));
                continue;
            };
            if program.primitive_type_reference(value_field.type_reference)
                != Some(*schema_primitive)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode` value field `{}.{}` is `{}`, but the schema declares field {} as `{}`",
                    schema.name,
                    value_data.name,
                    field.name,
                    program.display_type_reference(value_field.type_reference),
                    field.number,
                    program.display_type_reference(field.type_reference)
                )));
            }
        }
        // A `&[u8]` schema field encodes a `&[u8]` value field (its raw bytes).
        for field in &byte_slice_fields {
            let Some(value_field) =
                program
                    .data_members(value_data)
                    .iter()
                    .find_map(|member| match member {
                        psi_typed_trees::data::DataMember::Field(data_field)
                            if data_field.name == field.name =>
                        {
                            Some(data_field)
                        }
                        _ => None,
                    })
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode` value type `{}` has no field `{}` to encode (schema field {})",
                    schema.name, value_data.name, field.name, field.number
                )));
                continue;
            };
            if !program.is_borrowed_byte_slice(value_field.type_reference) {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode` value field `{}.{}` is `{}`, but the schema declares field {} as a borrowed `&[u8]`; the value field must also be `&[u8]`",
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
                "encode",
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
                "encode",
                value_data,
                field,
                *repeated,
                diagnostics,
            );
        }
        for (field, schema_slice) in &scalar_slice_fields {
            let Some(value_field) =
                program
                    .data_members(value_data)
                    .iter()
                    .find_map(|member| match member {
                        psi_typed_trees::data::DataMember::Field(data_field)
                            if data_field.name == field.name =>
                        {
                            Some(data_field)
                        }
                        _ => None,
                    })
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode` value type `{}` has no field `{}` to encode (schema field {} is a borrowed scalar slice)",
                    schema.name, value_data.name, field.name, field.number
                )));
                continue;
            };
            let mut runtime_field = (*field).clone();
            runtime_field.type_reference = value_field.type_reference;
            if program.wire_field_borrowed_scalar_slice_encoding(&runtime_field)
                != Some(*schema_slice)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode` value field `{}.{}` is `{}`, but the schema declares borrowed scalar slice field {} as `{}`; the value field must use the same borrowed slice element type",
                    schema.name,
                    value_data.name,
                    field.name,
                    program.display_type_reference(value_field.type_reference),
                    field.number,
                    program.display_type_reference(field.type_reference)
                )));
            }
        }
    }

    // Out argument: `&mut [u8; N]` with N covering the worst case.
    if let Some(out_type) =
        declared_place_type(program, current_machine, current_state, arguments[1])
    {
        match program.type_reference_table.type_reference(out_type) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: psi_typed_trees::types::FixedArrayLength::Literal(length),
            } if program.primitive_type_reference(*element_type)
                == Some(psi_typed_trees::types::PrimitiveType::U8) =>
            {
                if *length < worst_case_bytes {
                    diagnostics.push(Diagnostic::error(format!(
                        "`{}::encode` out buffer `[u8; {length}]` is too small: the worst-case encoding needs {worst_case_bytes} bytes (era varint + per-field tag and value varints)",
                        schema.name
                    )));
                }
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode` out argument must be `&mut [u8; N]`, got `{}`",
                    schema.name,
                    program.display_type_reference(out_type)
                )));
            }
        }
    }

    // Written argument: `&mut u64` (the byte count -- a size, so u64).
    if let Some(written_type) =
        declared_place_type(program, current_machine, current_state, arguments[2])
        && !matches!(
            program.primitive_type_reference(written_type),
            Some(psi_typed_trees::types::PrimitiveType::U64)
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::encode` written argument must be `&mut u64`, got `{}`",
            schema.name,
            program.display_type_reference(written_type)
        )));
    }
}

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
fn validate_wire_decode_call(
    program: &TypedTrees,
    schema: &WireSchema,
    call: &psi_typed_trees::statement::TableCall,
    current_machine: &psi_typed_trees::machine::Machine,
    current_state: Option<&psi_typed_trees::state::State>,
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
    let mut repeated_fields: Vec<(&WireField, psi_typed_trees::wire::WireRepeatedEncoding)> =
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
        let scalar = primitive.and_then(psi_typed_trees::wire::WireScalarEncoding::for_primitive);
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
            .any(|member| matches!(member, psi_typed_trees::data::DataMember::Variant(_)))
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
                        psi_typed_trees::data::DataMember::Field(data_field)
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
            if psi_typed_trees::wire::type_reference_carries_range(
                program,
                value_field.type_reference,
            ) && psi_typed_trees::wire::scalar_decode_range(program, value_field.type_reference)
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
                        psi_typed_trees::data::DataMember::Field(data_field)
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
                length: psi_typed_trees::types::FixedArrayLength::Literal(_),
            } if program.primitive_type_reference(*element_type)
                == Some(psi_typed_trees::types::PrimitiveType::U8)
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
            Some(psi_typed_trees::types::PrimitiveType::U64)
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
            psi_typed_trees::data::DataMember::Variant(variant) if variant.payload.is_empty() => {
                Some(variant.name.as_str())
            }
            _ => None,
        })
        .collect();
    members.len() == 2 && case_names == ["Invalid", "Sound"]
}

/// A nested message field's value member must be a (non-case-bearing) data
/// type that declares every CHILD schema field with the same primitive type
/// -- the matching rule the top-level value obeys, applied one level down.
#[allow(clippy::too_many_arguments)]
fn validate_nested_value_field(
    program: &TypedTrees,
    schema: &WireSchema,
    machine_name: &str,
    value_data: &psi_typed_trees::data::DataDefinition,
    field: &WireField,
    child: &WireSchema,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(value_field) =
        program
            .data_members(value_data)
            .iter()
            .find_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(data_field)
                    if data_field.name == field.name =>
                {
                    Some(data_field)
                }
                _ => None,
            })
    else {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::{machine_name}` value type `{}` has no field `{}` (schema field {} nests wire schema `{}`)",
            schema.name, value_data.name, field.name, field.number, child.name
        )));
        return;
    };

    let Some(child_value_data) = unwrapped_type_reference(program, value_field.type_reference)
        .and_then(|unwrapped| named_data_definition(program, unwrapped))
    else {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::{machine_name}` value field `{}.{}` is `{}`, but schema field {} nests wire schema `{}` and needs a data value with its fields",
            schema.name,
            value_data.name,
            field.name,
            program.display_type_reference(value_field.type_reference),
            field.number,
            child.name
        )));
        return;
    };

    if program
        .data_members(child_value_data)
        .iter()
        .any(|member| matches!(member, psi_typed_trees::data::DataMember::Variant(_)))
    {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::{machine_name}` value field `{}.{}` has case-bearing type `{}`; wire encoding over sums and mixed data shapes is not implemented yet",
            schema.name, value_data.name, field.name, child_value_data.name
        )));
        return;
    }

    for member in program.wire_members(child.members) {
        let WireMember::Field(child_field) = member else {
            continue;
        };
        if child_field.relevance.is_erased() {
            continue;
        }
        let Some(child_value_field) =
            program
                .data_members(child_value_data)
                .iter()
                .find_map(|member| match member {
                    psi_typed_trees::data::DataMember::Field(data_field)
                        if data_field.name == child_field.name =>
                    {
                        Some(data_field)
                    }
                    _ => None,
                })
        else {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::{machine_name}` nested value type `{}` has no field `{}` (wire schema `{}` field {})",
                schema.name, child_value_data.name, child_field.name, child.name, child_field.number
            )));
            continue;
        };
        // Same establishment rule one level down: every scalar read carries
        // the nested destination field's normalized interval.
        if machine_name == "decode"
            && psi_typed_trees::wire::type_reference_carries_range(
                program,
                child_value_field.type_reference,
            )
            && psi_typed_trees::wire::scalar_decode_range(program, child_value_field.type_reference)
                .is_none()
        {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::decode` nested value field `{}.{}` declares a range fact (`{}`) \
                 that cannot be normalized into a constant scalar interval",
                schema.name,
                child_value_data.name,
                child_field.name,
                program.display_type_reference(child_value_field.type_reference)
            )));
        };
        if program.primitive_type_reference(child_value_field.type_reference)
            != program.primitive_type_reference(child_field.type_reference)
        {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::{machine_name}` nested value field `{}.{}` is `{}`, but wire schema `{}` declares field {} as `{}`",
                schema.name,
                child_value_data.name,
                child_field.name,
                program.display_type_reference(child_value_field.type_reference),
                child.name,
                child_field.number,
                program.display_type_reference(child_field.type_reference)
            )));
        }
    }
}

/// A repeated wire field's runtime member must use the same carrier semantics,
/// scalar element, and capacity as the schema. Fixed arrays are exactly full;
/// `FixedVec<T, N>` owns its live length in the carrier itself.
fn validate_repeated_value_field(
    program: &TypedTrees,
    schema: &WireSchema,
    machine_name: &str,
    value_data: &psi_typed_trees::data::DataDefinition,
    field: &WireField,
    repeated: psi_typed_trees::wire::WireRepeatedEncoding,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let find_field = |name: &str| {
        program
            .data_members(value_data)
            .iter()
            .find_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(data_field)
                    if data_field.name.as_str() == name =>
                {
                    Some(data_field)
                }
                _ => None,
            })
    };

    match find_field(field.name.as_str()) {
        None => {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::{machine_name}` value type `{}` has no field `{}` (schema field {} is repeated)",
                schema.name, value_data.name, field.name, field.number
            )));
        }
        Some(value_field) => {
            let mut runtime_field = field.clone();
            runtime_field.type_reference = value_field.type_reference;
            let matches_schema =
                program.wire_field_repeated_encoding(&runtime_field) == Some(repeated);
            if !matches_schema {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::{machine_name}` value field `{}.{}` is `{}`, but the schema declares repeated field {} as `{}` -- the value carrier, element type, and capacity must match exactly",
                    schema.name,
                    value_data.name,
                    field.name,
                    program.display_type_reference(value_field.type_reference),
                    field.number,
                    program.display_type_reference(field.type_reference)
                )));
            }
            if machine_name == "decode"
                && let Some(element_type) = psi_typed_trees::wire::repeated_element_type(
                    program,
                    value_field.type_reference,
                    repeated.carrier,
                )
                && psi_typed_trees::wire::type_reference_carries_range(program, element_type)
                && psi_typed_trees::wire::scalar_decode_range(program, element_type).is_none()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::decode` repeated value field `{}.{}` declares an element range fact \
                     (`{}`) that cannot be normalized into a constant scalar interval",
                    schema.name,
                    value_data.name,
                    field.name,
                    program.display_type_reference(element_type)
                )));
            }
        }
    }
}

/// The data definition a `Named` type reference points at, if any.
fn named_data_definition<'program>(
    program: &'program TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&'program psi_typed_trees::data::DataDefinition> {
    let TypeReferenceNode::Named { name, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    program
        .data_definitions()
        .iter()
        .find(|data| data.name == *name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_language_core::BindingRelevance;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::types::TypeReferenceNode;

    fn self_referential_schema(relevance: BindingRelevance) -> TypedTrees {
        let mut program = TypedTrees::default();
        let self_reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: Default::default(),
                name: Identifier::generated("Recursive"),
            });
        let members = program.append_wire_members(vec![WireMember::Field(WireField {
            number: 0,
            name: Identifier::generated("next"),
            relevance,
            type_reference: self_reference,
        })]);
        program.push_wire_schema(WireSchema {
            name: Identifier::generated("Recursive"),
            members,
            ..WireSchema::default()
        });
        program
    }

    #[test]
    fn erased_nested_field_does_not_form_a_physical_schema_cycle() {
        let program = self_referential_schema(BindingRelevance::Erased);
        let mut diagnostics = Vec::new();

        validate_nested_schema_cycles(&program, &program.wire_schemas()[0], &mut diagnostics);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn relevant_nested_field_still_forms_a_physical_schema_cycle() {
        let program = self_referential_schema(BindingRelevance::Relevant);
        let mut diagnostics = Vec::new();

        validate_nested_schema_cycles(&program, &program.wire_schemas()[0], &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
    }
}
