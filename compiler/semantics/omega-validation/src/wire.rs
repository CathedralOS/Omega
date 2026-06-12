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
    }
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
            program, symbols, schema, scope, field, *referee, diagnostics,
        ),
        TypeReferenceNode::Constrained { base_type, .. } => validate_field_type_reference(
            program, symbols, schema, scope, field, *base_type, diagnostics,
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
///   bool) -- strings, nested messages, and repeated fields reject;
/// - the value argument's data type declares every schema field with the
///   SAME primitive type;
/// - the out buffer is `&mut [u8; N]` with room for the WORST-CASE encoding
///   (era varint + per field tag varint + max value varint), so the emitted
///   appends never need a runtime bounds check;
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

    // Schema side: the stage 2a scalar set, and the worst-case byte budget
    // the out buffer must cover (era varint + per-field tag varint + max
    // value varint).
    let era = program.wire_schema_current_era(schema);
    let mut worst_case_bytes = omega_typed_trees::wire::wire_varint_bytes(era).len();
    let mut current_fields = Vec::new();
    let mut schema_rejects = false;
    for member in program.wire_members(schema.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        let primitive = program.primitive_type_reference(field.type_reference);
        let Some(encoding) =
            primitive.and_then(omega_typed_trees::wire::WireScalarEncoding::for_primitive)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "wire data `{}` field `{}`: `{}` is not encodable by the compact_binary v0 encoder yet; wire stage 2a supports i32, i64, u32, u64, and bool",
                schema.name,
                field.name,
                program.display_type_reference(field.type_reference)
            )));
            schema_rejects = true;
            continue;
        };
        if field.number < 0 {
            diagnostics.push(Diagnostic::error(format!(
                "wire data `{}` field `{}`: negative field number {} cannot ride a varint tag",
                schema.name, field.name, field.number
            )));
            schema_rejects = true;
            continue;
        }
        worst_case_bytes +=
            omega_typed_trees::wire::wire_varint_bytes(field.number as u64).len()
                + encoding.max_varint_length();
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
///   bool) -- strings, nested messages, and repeated fields reject;
/// - the value argument's data type declares every schema field with the
///   SAME primitive type;
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

    // Schema side: the stage 2 scalar set, same gate as the encoder.
    let mut current_fields = Vec::new();
    let mut schema_rejects = false;
    for member in program.wire_members(schema.members) {
        let WireMember::Field(field) = member else {
            continue;
        };
        let primitive = program.primitive_type_reference(field.type_reference);
        if primitive
            .and_then(omega_typed_trees::wire::WireScalarEncoding::for_primitive)
            .is_none()
        {
            diagnostics.push(Diagnostic::error(format!(
                "wire data `{}` field `{}`: `{}` is not decodable by the compact_binary v0 decoder yet; wire stage 2b supports i32, i64, u32, u64, and bool",
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

/// The DECLARED type of a simple place argument: a bare local/parameter name
/// (`msg`) or an attached-data field (`self.buffer`), through the `&mut`
/// marker. `None` for shapes this scope cannot type (those re-resolve during
/// instruction selection).
fn declared_place_type(
    program: &TypedTrees,
    current_machine: &omega_typed_trees::machine::Machine,
    current_state: Option<&omega_typed_trees::state::State>,
    argument: omega_typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    use omega_typed_trees::expression::ExpressionNode;

    let mut handle = argument;
    if let ExpressionNode::Mutable(inner) = program.expression_table.expression(handle) {
        handle = *inner;
    }

    let members: Vec<String> = match program.expression_table.expression(handle) {
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .iter()
            .map(|member| member.as_str().to_owned())
            .collect(),
        ExpressionNode::Member(member) => {
            let ExpressionNode::Name(receiver) =
                program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            let mut names: Vec<String> = program
                .expression_table
                .name_path_members(receiver.members)
                .iter()
                .map(|name| name.as_str().to_owned())
                .collect();
            names.push(member.member.as_str().to_owned());
            names
        }
        _ => return None,
    };

    match members.as_slice() {
        [name] => {
            if let Some(state) = current_state {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let omega_typed_trees::statement::StatementNode::LocalData(local) =
                        statement
                        && local.name.as_str() == name
                    {
                        return unwrapped_type_reference(program, local.type_reference);
                    }
                }
                for parameter in program.state_parameters(state) {
                    if parameter.name.as_str() == name {
                        return unwrapped_type_reference(program, parameter.type_reference);
                    }
                }
            }
            None
        }
        [root, field_name] if root == "self" => {
            let attached = current_machine.attached_data.as_ref()?;
            let data = program
                .data_definitions()
                .iter()
                .find(|data| data.name == *attached)?;
            let field = program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    omega_typed_trees::data::DataMember::Field(field)
                        if field.name.as_str() == field_name =>
                    {
                        Some(field)
                    }
                    _ => None,
                })?;
            unwrapped_type_reference(program, field.type_reference)
        }
        _ => None,
    }
}

/// Unwrap reference and constraint shells so the structural type underneath
/// (`[u8; N]`, `usize`, a data name) is inspectable.
fn unwrapped_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => unwrapped_type_reference(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            unwrapped_type_reference(program, *base_type)
        }
        _ => Some(type_reference),
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
