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
        validate_data_shape(program, data_definition, data_members, diagnostics);

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

// The zero-case-payload-free rule moved into `[zero_init]` verification
// (crate::properties): zero-VALIDITY is unconditional (a zeroed payload is
// itself zeroed, so the value stays valid), while zero-MEANS-EMPTY is the
// opt-in property that demands a payload-free zero case (frozen decision 8).

fn validate_payload_field_names(
    data_definition: &omega_typed_trees::data::DataDefinition,
    variant: &omega_typed_trees::data::DataVariant,
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let data_members = program.data_members(data_definition);
    let payload_fields = program.data_payload_fields(variant);
    for (field_index, field) in payload_fields.iter().enumerate() {
        if payload_fields[..field_index]
            .iter()
            .any(|previous| previous.name.as_str() == field.name.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` case `{}` has duplicate payload field `{}`",
                data_definition.name, variant.name, field.name
            )));
        }

        // Mixed shapes: a payload field may not reuse a COMMON field's name.
        // Member access (`value.name`) searches common fields first and then
        // every case's payload, so a collision would silently rebind reads.
        if data_members.iter().any(|member| {
            matches!(member, DataMember::Field(common) if common.name.as_str() == field.name.as_str())
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` case `{}` payload field `{}` collides with the common field of the same name",
                data_definition.name,
                variant.name,
                field.name
            )));
        }
    }
}

/// Mixed shapes (common fields + cases) are accepted with two honest-first-cut
/// restrictions, both rejected loudly here:
///
/// - COMMON fields must be scalar primitives (bool / integers / floats).
///   Case construction zero-initializes every common field not named in the
///   literal, and a scalar zero is one storage write in both backends; zeroing
///   nested aggregates / text / slices at construction is deferred.
/// - COMMON fields may not declare default initializers. Construction of a
///   mixed value is always the case-literal form, whose rule is
///   zero-unless-named (ZII keeps that valid); a default would silently not
///   apply, so it is rejected instead of ignored.
///
/// Payload-field/common-field name collisions are rejected separately in
/// `validate_payload_field_names` (member access searches both namespaces).
fn validate_data_shape(
    program: &TypedTrees,
    data_definition: &omega_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match omega_typed_trees::data::DataDefinition::shape_kind_from_members(data_members) {
        DataShapeKind::Empty | DataShapeKind::Enum | DataShapeKind::Record => {}
        DataShapeKind::Mixed => {
            for member in data_members {
                let DataMember::Field(field) = member else {
                    continue;
                };
                let scalar = matches!(
                    program.primitive_type_reference(field.type_reference),
                    Some(primitive) if primitive != omega_typed_trees::types::PrimitiveType::String
                );
                if !scalar {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{}` common field `{}` is not a scalar primitive; mixed data shape common fields support only bool, integer, and float types for now (case construction zero-initializes unnamed common fields)",
                        data_definition.name, field.name
                    )));
                }
                if field.initial_value.is_valid() {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{}` common field `{}` declares a default value; mixed data shape common fields are zero-initialized unless named in the case literal, so a default would never apply",
                        data_definition.name, field.name
                    )));
                }
            }
        }
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
