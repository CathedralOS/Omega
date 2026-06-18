use crate::places::{declared_place_type, unwrapped_type_reference};
use crate::symbols::TopLevelSymbols;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use omega_typed_trees::wire::{WireField, WireMember, WireReserved, WireSchema, WireVersion};

/// Validates `wire data` protocol schemas (chapter 20): stable field numbers,
/// reserved (retired) tags, version eras, resolvable field types, and the
/// checkable compatibility rules along the VERSION CHAIN -- each declared era
/// against its successor (v1 against v2, the newest era against the current
/// schema body).
pub(crate) fn validate_wire_schemas(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (schema_index, schema) in program.wire_schemas().iter().enumerate() {
        if program.wire_schemas()[..schema_index]
            .iter()
            .any(|previous| previous.name == schema.name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate wire data `{}`",
                schema.name
            )));
        }

        validate_schema(program, symbols, schema, diagnostics);
        validate_nested_schema_cycles(program, schema, diagnostics);
    }
}

/// A schema that reaches ITSELF through nested message fields (directly or
/// through siblings) can never have a finite worst-case encoding, so the
/// cycle is a hard error at the declaration -- not a call-site rejection.
/// Only CURRENT-era fields participate: version blocks snapshot history and
/// are never encoded by the current encoder.
fn validate_nested_schema_cycles(
    program: &TypedTrees,
    schema: &WireSchema,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut visited: Vec<&str> = Vec::new();
    if nested_references_reach(program, schema, schema.name.as_str(), &mut visited) {
        diagnostics.push(Diagnostic::error(format!(
            "wire data `{}` contains itself through its nested message fields; a schema cycle has no finite worst-case encoding",
            schema.name
        )));
    }
}

fn nested_references_reach<'program>(
    program: &'program TypedTrees,
    from: &WireSchema,
    target: &str,
    visited: &mut Vec<&'program str>,
) -> bool {
    for member in program.wire_members(from.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        let Some(child) = program.wire_field_nested_schema(field) else {
            continue;
        };
        if child.name.as_str() == target {
            return true;
        }
        if visited.contains(&child.name.as_str()) {
            continue;
        }
        visited.push(child.name.as_str());
        if nested_references_reach(program, child, target, visited) {
            return true;
        }
    }
    false
}

/// One numbering scope: the current schema body or a single version block.
/// Field numbers must be unique within a scope and must not reuse a number
/// the same scope reserves; different eras may legitimately reuse numbers.
struct WireScope<'program> {
    version: Option<&'program str>,
    fields: Vec<&'program WireField>,
    reserved: Vec<&'program WireReserved>,
    versions: Vec<&'program WireVersion>,
}

fn collect_scope<'program>(
    program: &'program TypedTrees,
    version: Option<&'program str>,
    members: HandleSpan<WireMember>,
) -> WireScope<'program> {
    let mut scope = WireScope {
        version,
        fields: Vec::new(),
        reserved: Vec::new(),
        versions: Vec::new(),
    };

    for member in program.wire_members(members) {
        match member {
            WireMember::Field(field) => scope.fields.push(field),
            WireMember::Reserved(reserved) => scope.reserved.push(reserved),
            WireMember::Version(nested) => scope.versions.push(nested),
        }
    }

    scope
}

fn scope_label(schema: &WireSchema, scope: &WireScope<'_>) -> String {
    match scope.version {
        Some(version) => format!("wire data `{}` version `{version}`", schema.name),
        None => format!("wire data `{}`", schema.name),
    }
}

fn validate_schema(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    schema: &WireSchema,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let current = collect_scope(program, None, schema.members);

    for (version_index, version) in current.versions.iter().enumerate() {
        if current.versions[..version_index]
            .iter()
            .any(|previous| previous.name == version.name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "wire data `{}` declares duplicate version `{}`",
                schema.name, version.name
            )));
        }
    }

    let version_scopes = current
        .versions
        .iter()
        .map(|version| collect_scope(program, Some(version.name.as_str()), version.members))
        .collect::<Vec<_>>();

    for scope in version_scopes.iter().chain(std::iter::once(&current)) {
        validate_scope_numbering(schema, scope, diagnostics);
        validate_scope_field_types(program, symbols, schema, scope, diagnostics);

        if scope.version.is_some() && !scope.versions.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "{} nests a version block inside a version block; protocol eras must be declared at the schema level",
                scope_label(schema, scope)
            )));
        }
    }

    // Compatibility checks run along the version chain: each declared era is
    // checked against its successor; the newest declared era is checked
    // against the current schema body. Eras are NOT all compared against
    // current -- migrations compose hop by hop.
    let mut chain: Vec<&WireScope<'_>> = version_scopes.iter().collect();
    chain.push(&current);

    for pair in chain.windows(2) {
        validate_adjacent_eras(schema, pair[0], pair[1], diagnostics);
    }
}

fn validate_scope_numbering(
    schema: &WireSchema,
    scope: &WireScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (field_index, field) in scope.fields.iter().enumerate() {
        if scope.fields[..field_index]
            .iter()
            .any(|previous| previous.number == field.number)
        {
            diagnostics.push(Diagnostic::error(format!(
                "{} declares duplicate field number {} for field `{}`",
                scope_label(schema, scope),
                field.number,
                field.name
            )));
        }

        if scope
            .reserved
            .iter()
            .any(|reserved| reserved.number == field.number)
        {
            diagnostics.push(Diagnostic::error(format!(
                "{} field `{}` reuses reserved field number {}",
                scope_label(schema, scope),
                field.name,
                field.number
            )));
        }
    }

    for (reserved_index, reserved) in scope.reserved.iter().enumerate() {
        if scope.reserved[..reserved_index]
            .iter()
            .any(|previous| previous.number == reserved.number)
        {
            diagnostics.push(Diagnostic::error(format!(
                "{} reserves field number {} more than once",
                scope_label(schema, scope),
                reserved.number
            )));
        }
    }
}

fn validate_scope_field_types(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    schema: &WireSchema,
    scope: &WireScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for field in &scope.fields {
        validate_field_type_reference(
            program,
            symbols,
            schema,
            scope,
            field,
            field.type_reference,
            diagnostics,
        );

        // A repeated field is spelled `[element; max]`. The declared maximum
        // is part of the wire contract -- it is what gives the packed payload
        // a finite worst case -- so a bare slice (`[element]`, no maximum)
        // can never be encoded and is rejected at the declaration, like a
        // schema cycle. A zero maximum declares a field that can never carry
        // an element; reject it as a contradiction rather than encode an
        // always-empty payload.
        if program.wire_field_is_unbounded_slice(field)
            && !program.is_borrowed_byte_slice(field.type_reference)
        {
            // A borrowed byte slice `&[u8]` is the zero-copy RAW-bytes/text
            // field (length varint + raw bytes, runtime-bounded like the old
            // String content), so it is exempt from the "needs a maximum" rule
            // that owned repeated `[scalar; max]` fields obey. Any OTHER bare
            // slice still rejects -- it has no finite worst case.
            diagnostics.push(Diagnostic::error(format!(
                "{} field `{}`: a repeated wire field needs a declared maximum element count (`[{}; N]`); a bare slice has no finite worst-case encoding",
                scope_label(schema, scope),
                field.name,
                program.display_type_reference(field.type_reference)
                    .trim_start_matches('[')
                    .trim_end_matches(']')
            )));
        } else if let Some((_, max_count)) = program.wire_field_fixed_array(field)
            && max_count == 0
        {
            diagnostics.push(Diagnostic::error(format!(
                "{} field `{}`: a repeated wire field's maximum element count must be at least 1, got `{}`",
                scope_label(schema, scope),
                field.name,
                program.display_type_reference(field.type_reference)
            )));
        }
    }
}

fn validate_field_type_reference(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    schema: &WireSchema,
    scope: &WireScope<'_>,
    field: &WireField,
    type_reference: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => validate_field_type_reference(
            program,
            symbols,
            schema,
            scope,
            field,
            *referee,
            diagnostics,
        ),
        TypeReferenceNode::Constrained { base_type, .. } => validate_field_type_reference(
            program,
            symbols,
            schema,
            scope,
            field,
            *base_type,
            diagnostics,
        ),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => validate_field_type_reference(
            program,
            symbols,
            schema,
            scope,
            field,
            *element_type,
            diagnostics,
        ),
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            if !wire_type_resolves(program, symbols, base_name.as_str()) {
                push_unknown_field_type(schema, scope, field, base_name.as_str(), diagnostics);
            }

            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                validate_field_type_reference(
                    program,
                    symbols,
                    schema,
                    scope,
                    field,
                    *argument,
                    diagnostics,
                );
            }
        }
        TypeReferenceNode::DynamicTrait { name, .. } | TypeReferenceNode::Named { name, .. } => {
            if !wire_type_resolves(program, symbols, name.as_str()) {
                push_unknown_field_type(schema, scope, field, name.as_str(), diagnostics);
            }
        }
        TypeReferenceNode::Unit => {}
    }
}

/// Wire fields may reference any resolvable program type plus sibling wire
/// schemas (a wire message embedding another wire message).
fn wire_type_resolves(program: &TypedTrees, symbols: &TopLevelSymbols<'_>, name: &str) -> bool {
    symbols.has_type(name)
        || program
            .wire_schemas()
            .iter()
            .any(|schema| schema.name.as_str() == name)
}

fn push_unknown_field_type(
    schema: &WireSchema,
    scope: &WireScope<'_>,
    field: &WireField,
    type_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(Diagnostic::error(format!(
        "{} field `{}` references unknown type `{type_name}`",
        scope_label(schema, scope),
        field.name
    )));
}

/// Chapter 20 compatibility rules that are checkable between a declared era
/// and its SUCCESSOR era in the version chain (frozen decision 10):
/// - retiring a documented field number without reserving it in the successor
///   era is a declared-history contradiction and stays a hard error;
/// - a field number changing type across eras is legitimate evolution -- the
///   era discriminator lets the decoder pick the old era's table -- so it is
///   reported as "requires migration" in the wire protocol compatibility
///   artifact, NOT rejected here;
/// - cross-era recycling of a retired (reserved) number is legal: `reserved`
///   is era-scoped, so a later era declaring a field on a number a prior era
///   reserved produces no diagnostic.
/// Renames (same number, same type, new name) and additive fields are
/// compatible and produce no diagnostics.
fn validate_adjacent_eras(
    schema: &WireSchema,
    predecessor: &WireScope<'_>,
    successor: &WireScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let predecessor_name = predecessor.version.unwrap_or_default();

    for field in &predecessor.fields {
        let successor_field = successor
            .fields
            .iter()
            .find(|candidate| candidate.number == field.number);

        if successor_field.is_none() {
            let is_reserved = successor
                .reserved
                .iter()
                .any(|reserved| reserved.number == field.number);

            if !is_reserved {
                diagnostics.push(Diagnostic::error(format!(
                    "{} retires field number {} (`{}` in version `{predecessor_name}`) without reserving it; add `reserved {};`",
                    scope_label(schema, successor),
                    field.number,
                    field.name,
                    field.number
                )));
            }
        }
    }
}

/// Validate a call whose receiver names a wire schema: the synthesized
/// `Schema::encode_wire(&value, &mut out, &mut written)` encoder (wire stage
/// 2a) or `Schema::decode_wire(&mut value, &buffer, &mut read, &mut ok)`
/// decoder (wire stage 2b). Returns `true` when the receiver names a wire
/// schema (the call belongs to this module whether or not it validates).
pub(crate) fn validate_wire_schema_call(
    program: &TypedTrees,
    call: &omega_typed_trees::statement::TableCall,
    current_machine: &omega_typed_trees::machine::Machine,
    current_state: Option<&omega_typed_trees::state::State>,
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
        omega_typed_trees::wire::WIRE_ENCODE_MACHINE_NAME => {
            validate_wire_encode_call(
                program,
                schema,
                call,
                current_machine,
                current_state,
                diagnostics,
            );
        }
        omega_typed_trees::wire::WIRE_DECODE_MACHINE_NAME => {
            validate_wire_decode_call(
                program,
                schema,
                call,
                current_machine,
                current_state,
                diagnostics,
            );
        }
        _ => {
            diagnostics.push(Diagnostic::error(format!(
                "wire data `{}` has no machine `{}`; the compiler only synthesizes `encode_wire(&value, &mut out, &mut written)` and `decode_wire(&mut value, &buffer, &mut read, &mut ok)` (wire stage 2)",
                schema.name, call.target
            )));
        }
    }
    true
}

/// Validate the synthesized wire encoder call
/// `Schema::encode_wire(&value, &mut out, &mut written)` (chapter 20, wire
/// stage 2a).
///
/// The checks that make a bad program fail HERE, with a source-shaped
/// diagnostic, instead of in the backend:
/// - exactly three arguments;
/// - every current-era schema field is a stage 2a scalar (i32/i64/u32/u64/
///   bool), `String`, or a NESTED MESSAGE (a sibling wire schema whose body
///   is scalar-only -- String-in-child and nested-in-nested reject, one
///   honest level today); repeated fields reject;
/// - at most one `String` field, and it carries the highest field number so
///   it encodes LAST (its content is runtime-sized; see the worst-case
///   rule). The rule is PER MESSAGE SCOPE: nested fields are statically
///   bounded, so they may sit anywhere, and a child body has no String;
/// - the value argument's data type declares every schema field with the
///   SAME primitive type (a nested field's value member must be a data type
///   matching the CHILD schema's fields, one level down);
/// - the out buffer is `&mut [u8; N]` with room for the WORST-CASE encoding
///   (era varint + per field tag varint + max value varint; a String field
///   budgets tag + max length varint; a nested field budgets tag + length
///   varint + the child's static worst case), so every append EXCEPT the
///   trailing String byte-copy needs no runtime bounds check -- the
///   byte-copy alone bounds against N at runtime and truncates content past
///   capacity;
/// - the written argument is `&mut usize`.
///
/// Places this scope cannot type (alias-fed or computed arguments) skip the
/// value/out/written checks; instruction selection re-resolves every place
/// and an unresolved one surfaces as an emission-planning blocker.
fn validate_wire_encode_call(
    program: &TypedTrees,
    schema: &WireSchema,
    call: &omega_typed_trees::statement::TableCall,
    current_machine: &omega_typed_trees::machine::Machine,
    current_state: Option<&omega_typed_trees::state::State>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arguments = program.statement_table.expression_handles(call.arguments);
    if arguments.len() != 3 {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::encode_wire` expects 3 arguments (&value, &mut out, &mut written), got {}",
            schema.name,
            arguments.len()
        )));
        return;
    }

    // Schema side: the stage 2a scalar set plus String plus nested message
    // fields, and the worst-case byte budget the out buffer must cover (era
    // varint + per-field tag varint + max value varint; a String field
    // budgets its tag + max length varint -- its CONTENT is runtime-sized, so
    // the emitted byte-copy is the one append that bounds-checks against the
    // buffer at runtime instead; a nested message field budgets its tag + the
    // sub-message's length varint + the sub-message's static worst case).
    let era = program.wire_schema_current_era(schema);
    let mut worst_case_bytes = omega_typed_trees::wire::wire_varint_bytes(era).len();
    let mut current_fields = Vec::new();
    let mut nested_fields: Vec<(&WireField, &WireSchema)> = Vec::new();
    let mut repeated_fields: Vec<(&WireField, omega_typed_trees::wire::WireRepeatedEncoding)> =
        Vec::new();
    let mut text_fields: Vec<&WireField> = Vec::new();
    // Borrowed byte slices `&[u8]`: the zero-copy RAW-bytes/text field. Encodes
    // exactly like the old String content (length varint + raw bytes) and rides
    // the same runtime-sized Text constraints (at most one, encodes last), but
    // matches a `&[u8]` value field rather than an owned `String`.
    let mut byte_slice_fields: Vec<&WireField> = Vec::new();
    let mut max_field_number = i64::MIN;
    let mut schema_rejects = false;
    for member in program.wire_members(schema.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        // A REPEATED field (`[scalar; max]`) packs LENGTH-delimited: tag +
        // byte-length varint + back-to-back element varints (protobuf's
        // packed encoding). The declared maximum bounds it statically, so it
        // joins the worst-case budget like a scalar and may sit anywhere.
        // Only scalar elements are honest today: a String element is
        // runtime-sized and a nested-message element would need per-element
        // staging, so both reject loudly.
        if let Some((_, max_count)) = program.wire_field_fixed_array(field) {
            if field.number < 0 {
                diagnostics.push(Diagnostic::error(format!(
                    "wire data `{}` field `{}`: negative field number {} cannot ride a varint tag",
                    schema.name, field.name, field.number
                )));
                schema_rejects = true;
                continue;
            }
            let Some(repeated) = program.wire_field_repeated_encoding(field) else {
                diagnostics.push(Diagnostic::error(format!(
                    "wire data `{}` field `{}`: a repeated wire field's element must be a stage 2 scalar (i32, i64, u32, u64, bool); `{}` is not supported (repeated String and repeated nested messages reject until they have an honest encoding)",
                    schema.name,
                    field.name,
                    program.display_type_reference(field.type_reference)
                )));
                schema_rejects = true;
                continue;
            };
            let _ = max_count;
            max_field_number = max_field_number.max(field.number);
            worst_case_bytes +=
                omega_typed_trees::wire::wire_varint_bytes(field.number as u64).len()
                    + repeated.worst_case_payload_bytes();
            repeated_fields.push((field, repeated));
            continue;
        }
        // A borrowed byte slice `&[u8]` encodes as RAW bytes (length varint +
        // the bytes), runtime-sized like the old String content, so it joins
        // `text_fields` (at most one, must encode last) and matches a `&[u8]`
        // value field below. (A `[u8; N]` owned array is a repeated field,
        // handled above; only the borrowed slice reaches here.)
        if program.is_borrowed_byte_slice(field.type_reference) {
            if field.number < 0 {
                diagnostics.push(Diagnostic::error(format!(
                    "wire data `{}` field `{}`: negative field number {} cannot ride a varint tag",
                    schema.name, field.name, field.number
                )));
                schema_rejects = true;
                continue;
            }
            max_field_number = max_field_number.max(field.number);
            worst_case_bytes +=
                omega_typed_trees::wire::wire_varint_bytes(field.number as u64).len()
                    + omega_typed_trees::wire::WIRE_TEXT_LENGTH_MAX_VARINT_LENGTH;
            text_fields.push(field);
            byte_slice_fields.push(field);
            continue;
        }
        let primitive = program.primitive_type_reference(field.type_reference);
        let encoding =
            primitive.and_then(omega_typed_trees::wire::WireFieldEncoding::for_primitive);
        let nested = program.wire_field_nested_schema(field);
        if encoding.is_none() && nested.is_none() {
            diagnostics.push(Diagnostic::error(format!(
                "wire data `{}` field `{}`: `{}` is not encodable by the compact_binary v0 encoder yet; wire stage 2 supports i32, i64, u32, u64, bool, String, and a sibling wire schema (one nesting level)",
                schema.name,
                field.name,
                program.display_type_reference(field.type_reference)
            )));
            schema_rejects = true;
            continue;
        }
        if field.number < 0 {
            diagnostics.push(Diagnostic::error(format!(
                "wire data `{}` field `{}`: negative field number {} cannot ride a varint tag",
                schema.name, field.name, field.number
            )));
            schema_rejects = true;
            continue;
        }
        max_field_number = max_field_number.max(field.number);
        let tag_bytes = omega_typed_trees::wire::wire_varint_bytes(field.number as u64).len();
        if let Some(child) = nested {
            // A nested message field encodes as tag + LENGTH varint + the
            // sub-message's fields WITHOUT an era discriminator (decision 10:
            // the era rides only the top-level envelope). The whole framing
            // is statically bounded, so it joins the worst-case budget like a
            // scalar -- but only a scalar-only child body is bounded: String
            // content is runtime-sized and a doubly-nested body would need a
            // second staging region, so both reject (one honest level first).
            let Some(nested_worst) = program.wire_nested_field_worst_case(child) else {
                diagnostics.push(Diagnostic::error(format!(
                    "wire data `{}` field `{}`: nested wire schema `{}` must contain only scalar fields (i32, i64, u32, u64, bool); String and doubly-nested message fields inside a nested message are not supported yet",
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
                omega_typed_trees::wire::WireFieldEncoding::Scalar(scalar) => {
                    scalar.max_varint_length()
                }
                omega_typed_trees::wire::WireFieldEncoding::Text => {
                    text_fields.push(field);
                    omega_typed_trees::wire::WIRE_TEXT_LENGTH_MAX_VARINT_LENGTH
                }
            };
        current_fields.push((field, primitive.expect("encoding implies primitive")));
    }
    // A String field's byte count is runtime-sized, so every append AFTER it
    // would run with the compile-time capacity guarantee already spent. The
    // encoder therefore takes at most ONE String field, and it must encode
    // LAST (the highest field number); everything before it stays covered by
    // the worst-case budget, and the trailing byte-copy is runtime-bounded.
    if let [first_text, more_text @ ..] = text_fields.as_slice() {
        for field in more_text {
            diagnostics.push(Diagnostic::error(format!(
                "wire data `{}` field `{}`: the compact_binary v0 encoder supports at most one String field per message (String content is runtime-sized, so only the final field can be unbounded)",
                schema.name, field.name
            )));
            schema_rejects = true;
        }
        if more_text.is_empty() && first_text.number != max_field_number {
            diagnostics.push(Diagnostic::error(format!(
                "wire data `{}` field `{}`: a String field must carry the schema's highest field number so it encodes last; its byte count is runtime-sized, and any field after it would lose the compile-time out-buffer guarantee",
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
            .any(|member| matches!(member, omega_typed_trees::data::DataMember::Variant(_)))
        {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::encode_wire` value type `{}` is case-bearing; wire encoding over sums and mixed data shapes is not implemented yet (the case tag and payload have no schema spelling)",
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
                        omega_typed_trees::data::DataMember::Field(data_field)
                            if data_field.name == field.name =>
                        {
                            Some(data_field)
                        }
                        _ => None,
                    })
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode_wire` value type `{}` has no field `{}` to encode (schema field {})",
                    schema.name, value_data.name, field.name, field.number
                )));
                continue;
            };
            if program.primitive_type_reference(value_field.type_reference)
                != Some(*schema_primitive)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode_wire` value field `{}.{}` is `{}`, but the schema declares field {} as `{}`",
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
            let Some(value_field) = program
                .data_members(value_data)
                .iter()
                .find_map(|member| match member {
                    omega_typed_trees::data::DataMember::Field(data_field)
                        if data_field.name == field.name =>
                    {
                        Some(data_field)
                    }
                    _ => None,
                })
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode_wire` value type `{}` has no field `{}` to encode (schema field {})",
                    schema.name, value_data.name, field.name, field.number
                )));
                continue;
            };
            if !program.is_borrowed_byte_slice(value_field.type_reference) {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode_wire` value field `{}.{}` is `{}`, but the schema declares field {} as a borrowed `&[u8]`; the value field must also be `&[u8]`",
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
                "encode_wire",
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
                "encode_wire",
                value_data,
                field,
                *repeated,
                diagnostics,
            );
        }
    }

    // Out argument: `&mut [u8; N]` with N covering the worst case.
    if let Some(out_type) =
        declared_place_type(program, current_machine, current_state, arguments[1])
    {
        match program.type_reference_table.type_reference(out_type) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: omega_typed_trees::types::FixedArrayLength::Literal(length),
            } if program.primitive_type_reference(*element_type)
                == Some(omega_typed_trees::types::PrimitiveType::U8) =>
            {
                if *length < worst_case_bytes {
                    diagnostics.push(Diagnostic::error(format!(
                        "`{}::encode_wire` out buffer `[u8; {length}]` is too small: the worst-case encoding needs {worst_case_bytes} bytes (era varint + per-field tag and value varints)",
                        schema.name
                    )));
                }
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::encode_wire` out argument must be `&mut [u8; N]`, got `{}`",
                    schema.name,
                    program.display_type_reference(out_type)
                )));
            }
        }
    }

    // Written argument: `&mut usize`.
    if let Some(written_type) =
        declared_place_type(program, current_machine, current_state, arguments[2])
        && program.primitive_type_reference(written_type)
            != Some(omega_typed_trees::types::PrimitiveType::Usize)
    {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::encode_wire` written argument must be `&mut usize`, got `{}`",
            schema.name,
            program.display_type_reference(written_type)
        )));
    }
}

/// Validate the synthesized wire decoder call
/// `Schema::decode_wire(&mut value, &buffer, &mut read, &mut ok)` (chapter
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
/// - `read` is `&mut usize` (receives the byte count consumed) and `ok` is
///   `&mut bool` (the success flag).
///
/// Places this scope cannot type skip their checks; instruction selection
/// re-resolves every place and an unresolved one surfaces as an
/// emission-planning blocker.
fn validate_wire_decode_call(
    program: &TypedTrees,
    schema: &WireSchema,
    call: &omega_typed_trees::statement::TableCall,
    current_machine: &omega_typed_trees::machine::Machine,
    current_state: Option<&omega_typed_trees::state::State>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arguments = program.statement_table.expression_handles(call.arguments);
    if arguments.len() != 4 {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::decode_wire` expects 4 arguments (&mut value, &buffer, &mut read, &mut ok), got {}",
            schema.name,
            arguments.len()
        )));
        return;
    }

    // Schema side: the stage 2 scalar set plus nested message fields plus
    // repeated scalar fields, same gate as the encoder (String stays
    // encode-only).
    let mut current_fields = Vec::new();
    let mut nested_fields: Vec<(&WireField, &WireSchema)> = Vec::new();
    let mut repeated_fields: Vec<(&WireField, omega_typed_trees::wire::WireRepeatedEncoding)> =
        Vec::new();
    // Borrowed byte slices `&[u8]`: decoded ZERO-COPY as a length-prefixed view
    // of the buffer (no allocator, unlike an owned `String`).
    let mut byte_slice_fields: Vec<&WireField> = Vec::new();
    let mut schema_rejects = false;
    for member in program.wire_members(schema.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        // Mirrors the encoder's repeated gate: `[scalar; max]` decodes as a
        // LENGTH-delimited packed payload (bounds-checked loop capped at the
        // declared maximum); non-scalar elements reject.
        if program.wire_field_fixed_array(field).is_some() {
            if field.number < 0 {
                diagnostics.push(Diagnostic::error(format!(
                    "wire data `{}` field `{}`: negative field number {} cannot ride a varint tag",
                    schema.name, field.name, field.number
                )));
                schema_rejects = true;
                continue;
            }
            let Some(repeated) = program.wire_field_repeated_encoding(field) else {
                diagnostics.push(Diagnostic::error(format!(
                    "wire data `{}` field `{}`: a repeated wire field's element must be a stage 2 scalar (i32, i64, u32, u64, bool); `{}` is not supported (repeated String and repeated nested messages reject until they have an honest encoding)",
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
        // `String` field still rejects below -- it would need to copy bytes out.)
        if program.is_borrowed_byte_slice(field.type_reference) {
            if field.number < 0 {
                diagnostics.push(Diagnostic::error(format!(
                    "wire data `{}` field `{}`: negative field number {} cannot ride a varint tag",
                    schema.name, field.name, field.number
                )));
                schema_rejects = true;
                continue;
            }
            byte_slice_fields.push(field);
            continue;
        }
        let primitive = program.primitive_type_reference(field.type_reference);
        let scalar =
            primitive.and_then(omega_typed_trees::wire::WireScalarEncoding::for_primitive);
        let nested = program.wire_field_nested_schema(field);
        if scalar.is_none() && nested.is_none() {
            // An owned `String` decode is gated on a missing PREREQUISITE, not a
            // missing wire feature: it must copy the bytes out of the buffer into
            // owned storage, and Omega has no runtime allocator yet. Name that
            // dependency explicitly (and the allocator-free zero-copy path) rather
            // than lumping String in with genuinely-unsupported field types.
            let message = if primitive == Some(omega_typed_trees::types::PrimitiveType::String) {
                format!(
                    "wire data `{}` field `{}`: decoding an owned `String` is not implemented yet \
                     -- it must copy the field's bytes out of the decode buffer into owned storage, \
                     which requires the runtime allocator (the reclamation substrate / arena is \
                     designed but not built). The allocator-free alternative is a borrowed \
                     `&[u8]` field that views the decode buffer in place (zero-copy). \
                     (Encoding a `String` already works.)",
                    schema.name, field.name
                )
            } else {
                format!(
                    "wire data `{}` field `{}`: `{}` is not decodable by the compact_binary v0 decoder yet; wire stage 2b supports i32, i64, u32, u64, bool, and a sibling wire schema (one nesting level)",
                    schema.name,
                    field.name,
                    program.display_type_reference(field.type_reference)
                )
            };
            diagnostics.push(Diagnostic::error(message));
            schema_rejects = true;
            continue;
        }
        if field.number < 0 {
            diagnostics.push(Diagnostic::error(format!(
                "wire data `{}` field `{}`: negative field number {} cannot ride a varint tag",
                schema.name, field.name, field.number
            )));
            schema_rejects = true;
            continue;
        }
        if let Some(child) = nested {
            // Mirrors the encoder's nested gate: a scalar-only child body is
            // the one honest level today (String content has no decode story
            // at all, and a doubly-nested body would need a second end-bound
            // slot).
            if program.wire_schema_scalar_body_worst_case(child).is_none() {
                diagnostics.push(Diagnostic::error(format!(
                    "wire data `{}` field `{}`: nested wire schema `{}` must contain only scalar fields (i32, i64, u32, u64, bool); String and doubly-nested message fields inside a nested message are not supported yet",
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
            .any(|member| matches!(member, omega_typed_trees::data::DataMember::Variant(_)))
        {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::decode_wire` value type `{}` is case-bearing; wire decoding into sums and mixed data shapes is not implemented yet (the case tag and payload have no schema spelling)",
                schema.name, value_data.name
            )));
            return;
        }
        for (field, schema_primitive) in &current_fields {
            let Some(value_field) = program
                .data_members(value_data)
                .iter()
                .find_map(|member| match member {
                    omega_typed_trees::data::DataMember::Field(data_field)
                        if data_field.name == field.name =>
                    {
                        Some(data_field)
                    }
                    _ => None,
                })
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::decode_wire` value type `{}` has no field `{}` to decode into (schema field {})",
                    schema.name, value_data.name, field.name, field.number
                )));
                continue;
            };
            if program.primitive_type_reference(value_field.type_reference)
                != Some(*schema_primitive)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::decode_wire` value field `{}.{}` is `{}`, but the schema declares field {} as `{}`",
                    schema.name,
                    value_data.name,
                    field.name,
                    program.display_type_reference(value_field.type_reference),
                    field.number,
                    program.display_type_reference(field.type_reference)
                )));
            }
        }
        // A `&[u8]` schema field decodes into a `&[u8]` value field (a zero-copy
        // view of the buffer); the value's data field must match.
        for field in &byte_slice_fields {
            let Some(value_field) = program
                .data_members(value_data)
                .iter()
                .find_map(|member| match member {
                    omega_typed_trees::data::DataMember::Field(data_field)
                        if data_field.name == field.name =>
                    {
                        Some(data_field)
                    }
                    _ => None,
                })
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::decode_wire` value type `{}` has no field `{}` to decode into (schema field {})",
                    schema.name, value_data.name, field.name, field.number
                )));
                continue;
            };
            if !program.is_borrowed_byte_slice(value_field.type_reference) {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::decode_wire` value field `{}.{}` is `{}`, but the schema declares field {} as a borrowed `&[u8]` (zero-copy view); the value field must also be `&[u8]`",
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
                "decode_wire",
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
                "decode_wire",
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
                length: omega_typed_trees::types::FixedArrayLength::Literal(_),
            } if program.primitive_type_reference(*element_type)
                == Some(omega_typed_trees::types::PrimitiveType::U8)
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::decode_wire` buffer argument must be `&[u8; N]`, got `{}`",
            schema.name,
            program.display_type_reference(buffer_type)
        )));
    }

    // Read argument: `&mut usize`.
    if let Some(read_type) =
        declared_place_type(program, current_machine, current_state, arguments[2])
        && program.primitive_type_reference(read_type)
            != Some(omega_typed_trees::types::PrimitiveType::Usize)
    {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::decode_wire` read argument must be `&mut usize`, got `{}`",
            schema.name,
            program.display_type_reference(read_type)
        )));
    }

    // Ok argument: `&mut bool`.
    if let Some(ok_type) =
        declared_place_type(program, current_machine, current_state, arguments[3])
        && program.primitive_type_reference(ok_type)
            != Some(omega_typed_trees::types::PrimitiveType::Bool)
    {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::decode_wire` ok argument must be `&mut bool`, got `{}`",
            schema.name,
            program.display_type_reference(ok_type)
        )));
    }
}

/// A nested message field's value member must be a (non-case-bearing) data
/// type that declares every CHILD schema field with the same primitive type
/// -- the matching rule the top-level value obeys, applied one level down.
#[allow(clippy::too_many_arguments)]
fn validate_nested_value_field(
    program: &TypedTrees,
    schema: &WireSchema,
    machine_name: &str,
    value_data: &omega_typed_trees::data::DataDefinition,
    field: &WireField,
    child: &WireSchema,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(value_field) = program
        .data_members(value_data)
        .iter()
        .find_map(|member| match member {
            omega_typed_trees::data::DataMember::Field(data_field)
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
        .any(|member| matches!(member, omega_typed_trees::data::DataMember::Variant(_)))
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
        let Some(child_value_field) = program
            .data_members(child_value_data)
            .iter()
            .find_map(|member| match member {
                omega_typed_trees::data::DataMember::Field(data_field)
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

/// A repeated wire field's value member must mirror the schema spelling
/// exactly -- `name: [element; max]` with the SAME element primitive and the
/// SAME maximum -- and the value type must also declare the COUNT COMPANION
/// `name_count: usize`: the wire carries the packed byte length, never the
/// count, so the companion is where the runtime element count lives (the
/// encoder reads it, the decoder writes it).
fn validate_repeated_value_field(
    program: &TypedTrees,
    schema: &WireSchema,
    machine_name: &str,
    value_data: &omega_typed_trees::data::DataDefinition,
    field: &WireField,
    repeated: omega_typed_trees::wire::WireRepeatedEncoding,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let find_field = |name: &str| {
        program
            .data_members(value_data)
            .iter()
            .find_map(|member| match member {
                omega_typed_trees::data::DataMember::Field(data_field)
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
            let matches_schema = unwrapped_type_reference(program, value_field.type_reference)
                .map(|unwrapped| program.type_reference_table.type_reference(unwrapped))
                .is_some_and(|node| {
                    matches!(
                        node,
                        TypeReferenceNode::FixedArray {
                            element_type,
                            length: omega_typed_trees::types::FixedArrayLength::Literal(length),
                        } if *length == repeated.max_count
                            && program
                                .primitive_type_reference(*element_type)
                                .and_then(
                                    omega_typed_trees::wire::WireScalarEncoding::for_primitive
                                )
                                == Some(repeated.element)
                    )
                });
            if !matches_schema {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::{machine_name}` value field `{}.{}` is `{}`, but the schema declares repeated field {} as `{}` -- the value array must match the element type and maximum exactly",
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

    let count_name = omega_typed_trees::wire::wire_repeated_count_field_name(field.name.as_str());
    match find_field(&count_name) {
        None => {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::{machine_name}` value type `{}` has no field `{count_name}`: a repeated wire field needs a `usize` count companion next to the array (`{count_name}: usize;`) -- the encoder reads it and the decoder writes the decoded element count back",
                schema.name, value_data.name
            )));
        }
        Some(count_field) => {
            if program.primitive_type_reference(count_field.type_reference)
                != Some(omega_typed_trees::types::PrimitiveType::Usize)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::{machine_name}` value field `{}.{count_name}` must be `usize` (the repeated field's count companion), got `{}`",
                    schema.name,
                    value_data.name,
                    program.display_type_reference(count_field.type_reference)
                )));
            }
        }
    }
}

/// The data definition a `Named` type reference points at, if any.
fn named_data_definition<'program>(
    program: &'program TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&'program omega_typed_trees::data::DataDefinition> {
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
