use crate::symbols::TopLevelSymbols;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use omega_typed_trees::wire::{WireField, WireMember, WireReserved, WireSchema, WireVersion};

/// Validates `wire data` protocol schemas (chapter 20): stable field numbers,
/// reserved (retired) tags, version eras, resolvable field types, and the
/// checkable compatibility rules between each declared version and the current
/// schema body.
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

    for scope in &version_scopes {
        validate_version_against_current(program, schema, scope, &current, diagnostics);
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

/// Chapter 20 compatibility rules that are checkable between a declared
/// version era and the current schema body:
/// - a field number keeping its number but changing type needs an explicit
///   compatibility rule (none exist yet, so it is rejected);
/// - a field number retired from an old era must reserve the old number.
/// Renames (same number, same type, new name) and additive fields are
/// compatible and produce no diagnostics.
fn validate_version_against_current(
    program: &TypedTrees,
    schema: &WireSchema,
    version: &WireScope<'_>,
    current: &WireScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let version_name = version.version.unwrap_or_default();

    for field in &version.fields {
        let current_field = current
            .fields
            .iter()
            .find(|candidate| candidate.number == field.number);

        match current_field {
            Some(current_field) => {
                let old_type = program.display_type_reference(field.type_reference);
                let new_type = program.display_type_reference(current_field.type_reference);

                if old_type != new_type {
                    diagnostics.push(Diagnostic::error(format!(
                        "wire data `{}` field number {} changes type from `{old_type}` in version `{version_name}` to `{new_type}` without an explicit compatibility rule",
                        schema.name, field.number
                    )));
                }
            }
            None => {
                let is_reserved = current
                    .reserved
                    .iter()
                    .any(|reserved| reserved.number == field.number);

                if !is_reserved {
                    diagnostics.push(Diagnostic::error(format!(
                        "wire data `{}` retires field number {} (`{}` in version `{version_name}`) without reserving it; add `reserved {};`",
                        schema.name, field.number, field.name, field.number
                    )));
                }
            }
        }
    }
}
