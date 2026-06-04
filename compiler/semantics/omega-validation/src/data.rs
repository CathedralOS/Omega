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

        for member in data_members {
            let DataMember::Field(field) = member else {
                continue;
            };

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

fn validate_data_shape(
    data_definition: &omega_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match omega_typed_trees::data::DataDefinition::shape_kind_from_members(data_members) {
        DataShapeKind::Empty => {}
        DataShapeKind::Mixed => diagnostics.push(Diagnostic::error(format!(
            "data `{}` mixes fields and variants; split record data from enum-like data",
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
