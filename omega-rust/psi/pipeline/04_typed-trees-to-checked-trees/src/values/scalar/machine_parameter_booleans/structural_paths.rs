//! Types and primitives at a structural parameter path: the field or payload
//! each `Field`, `Case`, and `FixedIndex` segment selects.

use crate::values::scalar::structural_fields;
use crate::values::scalar::structural_fields::structural_data;
use checked_trees::CheckedStructuralPredicatePathSegment;
use typed_trees::TypedTrees;
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

fn field_type(
    program: &TypedTrees,
    receiver: TypeReferenceHandle,
    identity: &str,
) -> Option<TypeReferenceHandle> {
    let declaration = structural_data(program, receiver)?;
    program.data_members(declaration).iter().find_map(|member| {
        let typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        let matches_identity = match field.identity {
            Some(field_identity) => identity == format!("#{field_identity}"),
            None => field.name.as_str() == identity,
        };
        matches_identity.then_some(field.type_reference)
    })
}

pub(super) fn path_type_reference(
    program: &TypedTrees,
    parameters: &[StateParameter],
    parameter_position: u32,
    path: &[CheckedStructuralPredicatePathSegment],
) -> Option<TypeReferenceHandle> {
    let parameter = usize::try_from(parameter_position)
        .ok()
        .and_then(|position| parameters.get(position))?;
    let mut receiver = parameter.type_reference;
    let mut selected_case = None;
    for segment in path {
        match segment {
            CheckedStructuralPredicatePathSegment::FixedIndex(element_index) => {
                if selected_case.is_some() {
                    return None;
                }
                receiver =
                    structural_fields::fixed_index_element_type(program, receiver, *element_index)?;
            }
            CheckedStructuralPredicatePathSegment::Case(case) => {
                if selected_case.is_some() {
                    return None;
                }
                let data = structural_data(program, receiver)?;
                let variant = program.data_members(data).iter().find_map(|member| {
                    let typed_trees::data::DataMember::Variant(variant) = member else {
                        return None;
                    };
                    let identity = variant.path_identity();
                    (identity == *case).then_some(variant)
                })?;
                selected_case = Some(variant);
            }
            CheckedStructuralPredicatePathSegment::Field(field) => {
                receiver = if let Some(variant) = selected_case.take() {
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .find_map(|candidate| {
                            let identity = candidate.path_identity();
                            (identity == *field).then_some(candidate.type_reference)
                        })?
                } else {
                    field_type(program, receiver, field)?
                };
            }
        }
    }
    selected_case.is_none().then_some(receiver)
}

pub(super) fn path_primitive_type(
    program: &TypedTrees,
    parameters: &[StateParameter],
    parameter_position: u32,
    path: &[CheckedStructuralPredicatePathSegment],
) -> Option<PrimitiveType> {
    program.primitive_type_reference(path_type_reference(
        program,
        parameters,
        parameter_position,
        path,
    )?)
}

pub(super) fn structural_record_fields(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<Vec<&typed_trees::data::DataField>> {
    let data = structural_data(program, type_reference)?;
    let mut fields = Vec::new();
    for member in program.data_members(data) {
        let typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        if field.relevance.is_erased() {
            return None;
        }
        fields.push(field);
    }
    Some(fields)
}

pub(super) fn payloadless_sum_cases(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<Vec<String>> {
    let data = structural_data(program, type_reference)?;
    let members = program.data_members(data);
    if !matches!(
        typed_trees::data::DataDefinition::shape_kind_from_members(members),
        typed_trees::data::DataShapeKind::Enum
    ) {
        return None;
    }
    members
        .iter()
        .map(|member| {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            program
                .data_payload_fields(variant)
                .is_empty()
                .then(|| variant.path_identity())
        })
        .collect()
}
