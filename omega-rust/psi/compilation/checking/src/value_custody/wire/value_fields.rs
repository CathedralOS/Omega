use crate::value_custody::places::unwrapped_type_reference;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use typed_trees::wire::{WireField, WireMember, WireSchema};

/// A nested message field's value member must be a (non-case-bearing) data
/// type that declares every CHILD schema field with the same primitive type
/// -- the matching rule the top-level value obeys, applied one level down.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_nested_value_field(
    program: &TypedTrees,
    schema: &WireSchema,
    machine_name: &str,
    value_data: &typed_trees::data::DataDefinition,
    field: &WireField,
    child: &WireSchema,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(value_field) = crate::program_scope::data_definitions::data_field_named(
        program,
        value_data,
        field.name.clone(),
    ) else {
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
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
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
        let Some(child_value_field) = crate::program_scope::data_definitions::data_field_named(
            program,
            child_value_data,
            child_field.name.clone(),
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::{machine_name}` nested value type `{}` has no field `{}` (wire schema `{}` field {})",
                schema.name, child_value_data.name, child_field.name, child.name, child_field.number
            )));
            continue;
        };
        // Same establishment rule one level down: every scalar read carries
        // the nested destination field's normalized interval.
        if machine_name == "decode"
            && typed_trees::wire::type_reference_carries_range(
                program,
                child_value_field.type_reference,
            )
            && crate::scalar_representation_range(program, child_value_field.type_reference)
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
pub(super) fn validate_repeated_value_field(
    program: &TypedTrees,
    schema: &WireSchema,
    machine_name: &str,
    value_data: &typed_trees::data::DataDefinition,
    field: &WireField,
    repeated: typed_trees::wire::WireRepeatedEncoding,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let find_field = |name: &str| {
        program
            .data_members(value_data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(data_field)
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
                && let Some(element_type) = typed_trees::wire::repeated_element_type(
                    program,
                    value_field.type_reference,
                    repeated.carrier,
                )
                && typed_trees::wire::type_reference_carries_range(program, element_type)
                && crate::scalar_representation_range(program, element_type).is_none()
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
pub(super) fn named_data_definition(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
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

/// Direction-specific diagnostics vocabulary shared by the codec calls.
#[derive(Clone, Copy)]
pub(super) enum CodecDirection {
    Encode,
    Decode,
}

impl CodecDirection {
    pub(super) fn verb(self) -> &'static str {
        match self {
            Self::Encode => "encode",
            Self::Decode => "decode",
        }
    }

    fn sum_phrase(self) -> &'static str {
        match self {
            Self::Encode => "encoding over",
            Self::Decode => "decoding into",
        }
    }

    fn missing_field_phrase(self) -> &'static str {
        match self {
            Self::Encode => "encode",
            Self::Decode => "decode into",
        }
    }

    fn byte_slice_note(self) -> &'static str {
        match self {
            Self::Encode => "",
            Self::Decode => " (zero-copy view)",
        }
    }
}

/// One schema field's value-data lookup, emitting the direction's
/// missing-field diagnostic on absence.
fn value_field_or_missing<'program>(
    program: &'program TypedTrees,
    schema: &WireSchema,
    value_data: &'program typed_trees::data::DataDefinition,
    field: &WireField,
    direction: CodecDirection,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'program typed_trees::data::DataField> {
    let value_field = crate::program_scope::data_definitions::data_field_named(
        program,
        value_data,
        field.name.clone(),
    );
    if value_field.is_none() {
        diagnostics.push(Diagnostic::error(format!(
            "`{}::{}` value type `{}` has no field `{}` to {} (schema field {})",
            schema.name,
            direction.verb(),
            value_data.name,
            field.name,
            direction.missing_field_phrase(),
            field.number
        )));
    }
    value_field
}

/// The codec calls' shared value-argument walk: reject a case-bearing value
/// type (the schema field set only spells scalar FIELD members), then every
/// schema field must exist in the value data with the schema's primitive
/// type, every borrowed `&[u8]` schema field must match a `&[u8]` value
/// field, and nested/repeated fields defer to their own validators.
/// `scalar_check` runs a direction-specific tail per surviving scalar pair.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_value_argument_fields(
    program: &TypedTrees,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    schema: &WireSchema,
    value_argument: typed_trees::expression::ExpressionHandle,
    current_fields: &[(&WireField, typed_trees::types::PrimitiveType)],
    byte_slice_fields: &[&WireField],
    nested_fields: &[(&WireField, &WireSchema)],
    repeated_fields: &[(&WireField, typed_trees::wire::WireRepeatedEncoding)],
    direction: CodecDirection,
    diagnostics: &mut Vec<Diagnostic>,
    mut scalar_check: impl FnMut(
        &TypedTrees,
        &WireField,
        &typed_trees::data::DataField,
        &WireSchema,
        &typed_trees::data::DataDefinition,
        &mut Vec<Diagnostic>,
    ),
) {
    if let Some(value_type) = crate::value_custody::places::declared_place_type(
        program,
        current_machine,
        current_state,
        value_argument,
    ) && let Some(value_data) = named_data_definition(program, value_type)
    {
        if program
            .data_members(value_data)
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::{}` value type `{}` is case-bearing; wire {} sums and mixed data shapes is not implemented yet (the case tag and payload have no schema spelling)",
                schema.name,
                direction.verb(),
                value_data.name,
                direction.sum_phrase()
            )));
            return;
        }
        for (field, schema_primitive) in current_fields {
            let Some(value_field) =
                value_field_or_missing(program, schema, value_data, field, direction, diagnostics)
            else {
                continue;
            };
            if program.primitive_type_reference(value_field.type_reference)
                != Some(*schema_primitive)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::{}` value field `{}.{}` is `{}`, but the schema declares field {} as `{}`",
                    schema.name,
                    direction.verb(),
                    value_data.name,
                    field.name,
                    program.display_type_reference(value_field.type_reference),
                    field.number,
                    program.display_type_reference(field.type_reference)
                )));
            }
            scalar_check(program, field, value_field, schema, value_data, diagnostics);
        }
        for field in byte_slice_fields {
            let Some(value_field) =
                value_field_or_missing(program, schema, value_data, field, direction, diagnostics)
            else {
                continue;
            };
            if !program.is_borrowed_byte_slice(value_field.type_reference) {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}::{}` value field `{}.{}` is `{}`, but the schema declares field {} as a borrowed `&[u8]`{}; the value field must also be `&[u8]`",
                    schema.name,
                    direction.verb(),
                    value_data.name,
                    field.name,
                    program.display_type_reference(value_field.type_reference),
                    field.number,
                    direction.byte_slice_note()
                )));
            }
        }
        for (field, child) in nested_fields {
            validate_nested_value_field(
                program,
                schema,
                direction.verb(),
                value_data,
                field,
                child,
                diagnostics,
            );
        }
        for (field, repeated) in repeated_fields {
            validate_repeated_value_field(
                program,
                schema,
                direction.verb(),
                value_data,
                field,
                *repeated,
                diagnostics,
            );
        }
    }
}
