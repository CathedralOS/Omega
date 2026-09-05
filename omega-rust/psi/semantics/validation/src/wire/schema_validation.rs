use crate::symbols::TopLevelSymbols;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use typed_trees::wire::{WireField, WireMember, WireReserved, WireSchema, WireVersion};

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
            .any(|previous| {
                previous.name == schema.name
                    && !program
                        .symbols
                        .source_scopes_separate(previous.symbol, schema.symbol)
            })
        {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate data `{}`",
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
/// Only physically relevant CURRENT-era fields participate: version blocks
/// snapshot history and erased fields retain semantic identity, but neither
/// is encoded by the current encoder.
pub(super) fn validate_nested_schema_cycles(
    program: &TypedTrees,
    schema: &WireSchema,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut visited: Vec<&str> = Vec::new();
    if nested_references_reach(program, schema, schema.name.as_str(), &mut visited) {
        diagnostics.push(Diagnostic::error(format!(
            "data `{}` contains itself through its nested message fields; a schema cycle has no finite worst-case encoding",
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
        if field.relevance.is_erased() {
            continue;
        }
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
    // The declaration form is plain `data` with identity numbers (ch20); the
    // wire-schema machinery behind it is an implementation detail.
    match scope.version {
        Some(version) => format!("data `{}` version `{version}`", schema.name),
        None => format!("data `{}`", schema.name),
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
                "data `{}` declares duplicate version `{}`",
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
                "{} field `{}` reuses retired identity number {}",
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
                "{} retires identity number {} more than once",
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
        if field.relevance.is_erased() {
            continue;
        }

        // Bounded repeated carriers (`[T; N]` and `FixedVec<T, N>`) give the
        // generated realization a finite worst-case byte/work budget. A bare
        // scalar slice has no such bound in the current encode requirement.
        if program.wire_field_is_unbounded_slice(field)
            && !program.is_borrowed_byte_slice(field.type_reference)
            && program
                .wire_field_borrowed_scalar_slice_encoding(field)
                .is_none()
        {
            // A borrowed byte slice `&[u8]` is the zero-copy RAW-bytes/text
            // field (length varint + raw bytes, runtime-bounded like the old
            // runtime-sized text content), so it is exempt from the "needs a maximum" rule
            // that bounded repeated scalar carriers obey. Any OTHER bare
            // slice still rejects -- it has no finite worst case.
            diagnostics.push(Diagnostic::error(format!(
                "{} field `{}`: compact_binary cannot encode the unbounded slice `{}`; borrowed slices require a stage 2 scalar element (i32, i64, u32, u64, bool), while repeated text and nested-message elements still need an owned/bounded carrier",
                scope_label(schema, scope),
                field.name,
                program.display_type_reference(field.type_reference),
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
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            if !wire_type_resolves(program, symbols, *base_symbol, base_name.as_str()) {
                push_unknown_field_type(schema, scope, field, base_name.as_str(), diagnostics);
            }

            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                if matches!(
                    program.type_reference_table.type_reference(*argument),
                    TypeReferenceNode::Named { name, .. }
                        if name.as_str().parse::<u64>().is_ok()
                ) {
                    continue;
                }
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
        TypeReferenceNode::DynamicTrait { symbol, name, .. }
        | TypeReferenceNode::Named { symbol, name } => {
            if !wire_type_resolves(program, symbols, *symbol, name.as_str()) {
                push_unknown_field_type(schema, scope, field, name.as_str(), diagnostics);
            }
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {}
    }
}

/// Wire fields may reference any resolvable program type plus sibling wire
/// schemas (a wire message embedding another wire message).
fn wire_type_resolves(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    symbol: symbols::SymbolHandle,
    name: &str,
) -> bool {
    symbols.has_type_symbol(symbol)
        || PrimitiveType::from_name(name).is_some()
        || program.wire_schemas().iter().any(|schema| {
            schema.symbol == symbol
                || (!program.symbols.has_source_metadata() && schema.name.as_str() == name)
        })
        || (!program.symbols.has_source_metadata() && symbols.has_type(name))
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
///
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
                    "{} drops field number {} (`{}` in version `{predecessor_name}`) without tombstoning it; add `retired {};`",
                    scope_label(schema, successor),
                    field.number,
                    field.name,
                    field.number
                )));
            }
        }
    }
}
