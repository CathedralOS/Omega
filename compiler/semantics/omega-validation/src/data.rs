use crate::symbols::TopLevelSymbols;
use crate::type_references::{
    TypeReferenceOwner, validate_type_reference_handle_with_type_parameters,
};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{DataMember, DataShapeKind};

pub(crate) fn validate_data_field_types(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for data_definition in program.data_definitions() {
        let data_members = program.data_members(data_definition);
        let type_parameters = program.data_type_parameters(data_definition);
        validate_data_member_names(data_definition, data_members, diagnostics);
        validate_data_shape(data_definition, data_members, diagnostics);
        validate_zero_case_is_payload_free(data_definition, data_members, diagnostics);

        for member in data_members {
            let payload_fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => {
                    validate_payload_field_names(data_definition, variant, program, diagnostics);
                    program.data_payload_fields(variant)
                }
            };

            for field in payload_fields {
                validate_type_reference_handle_with_type_parameters(
                    program,
                    field.type_reference,
                    symbols,
                    diagnostics,
                    TypeReferenceOwner::DataField {
                        data: data_definition.name.as_str(),
                        field: field.name.as_str(),
                        generic_depth: 0,
                    },
                    type_parameters,
                );
            }
        }
    }
}

/// The FIRST case is the zero case (tag 0): a zeroed value must be a complete,
/// valid value, so the zero case carries no payload (frozen decision 7 / the
/// tag-prefixed overlay layout keeps the zero case payload-free).
fn validate_zero_case_is_payload_free(
    data_definition: &omega_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let first_case = data_members.iter().find_map(|member| match member {
        DataMember::Variant(variant) => Some(variant),
        DataMember::Field(_) => None,
    });
    if let Some(variant) = first_case
        && variant.payload.count() > 0
    {
        diagnostics.push(Diagnostic::error(format!(
            "data `{}` zero case `{}` must be payload-free: the first case is the zero-initialized value, so it cannot require payload fields",
            data_definition.name, variant.name
        )));
    }
}

fn validate_payload_field_names(
    data_definition: &omega_typed_trees::data::DataDefinition,
    variant: &omega_typed_trees::data::DataVariant,
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let payload_fields = program.data_payload_fields(variant);
    for (field_index, field) in payload_fields.iter().enumerate() {
        if payload_fields[..field_index]
            .iter()
            .any(|previous| previous.name.as_str() == field.name.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` case `{}` has duplicate payload field `{}`",
                data_definition.name,
                variant.name,
                field.name
            )));
        }
    }
}

fn validate_data_shape(
    data_definition: &omega_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match omega_typed_trees::data::DataDefinition::shape_kind_from_members(data_members) {
        DataShapeKind::Empty => {}
        DataShapeKind::Mixed => diagnostics.push(Diagnostic::error(format!(
            "data `{}` mixes fields and case members; mixed data shapes are not implemented yet (nest a sum-shaped data field instead)",
            data_definition.name
        ))),
        DataShapeKind::Enum | DataShapeKind::Record => {}
    }
}

fn validate_data_member_names(
    data_definition: &omega_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (member_index, member) in data_members.iter().enumerate() {
        let member_name = match member {
            DataMember::Field(field) => field.name.as_str(),
            DataMember::Variant(variant) => variant.name.as_str(),
        };

        if data_members[..member_index]
            .iter()
            .any(|previous| data_member_name(previous) == member_name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` has duplicate member `{member_name}`",
                data_definition.name
            )));
        }
    }
}

fn data_member_name(member: &DataMember) -> &str {
    match member {
        DataMember::Field(field) => field.name.as_str(),
        DataMember::Variant(variant) => variant.name.as_str(),
    }
}
