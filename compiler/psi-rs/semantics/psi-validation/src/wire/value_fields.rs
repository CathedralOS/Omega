use crate::places::unwrapped_type_reference;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use psi_typed_trees::wire::{WireField, WireMember, WireSchema};

/// A nested message field's value member must be a (non-case-bearing) data
/// type that declares every CHILD schema field with the same primitive type
/// -- the matching rule the top-level value obeys, applied one level down.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_nested_value_field(
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
pub(super) fn validate_repeated_value_field(
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
pub(super) fn named_data_definition<'program>(
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
