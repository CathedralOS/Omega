//! Types and primitives at a structural parameter path: the field or payload
//! each `Field`, `Case`, and `FixedIndex` segment selects.

use crate::values::scalar::structural_fields;
use crate::values::scalar::structural_fields::structural_data;
use checked_trees::CheckedStructuralPredicatePathSegment;
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

fn field_type(
    program: &dyn ::validation::CheckingInput,
    receiver: TypeReferenceHandle,
    identity: &checked_trees::CheckedMemberIdentity,
) -> Option<TypeReferenceHandle> {
    let declaration = structural_data(program, receiver)?;
    program.data_members(declaration).iter().find_map(|member| {
        let field = member.as_field()?;
        let matches_identity = *identity
            == checked_trees::CheckedMemberIdentity::of_member(field.identity, field.name.clone());
        matches_identity.then_some(field.type_reference)
    })
}

pub(crate) fn path_type_reference(
    program: &dyn ::validation::CheckingInput,
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
            CheckedStructuralPredicatePathSegment::RuntimeIndex(_) => {
                if selected_case.is_some() {
                    return None;
                }
                receiver = structural_fields::runtime_index_element_type(program, receiver)?;
            }
            CheckedStructuralPredicatePathSegment::Case(case) => {
                if selected_case.is_some() {
                    return None;
                }
                let data = structural_data(program, receiver)?;
                let variant = program.data_members(data).iter().find_map(|member| {
                    let variant = member.as_variant()?;
                    let identity = checked_trees::CheckedMemberIdentity::of_member(
                        variant.identity,
                        variant.name.clone(),
                    );
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
                            let identity = checked_trees::CheckedMemberIdentity::of_member(
                                candidate.identity,
                                candidate.name.clone(),
                            );
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

pub(crate) fn path_primitive_type(
    program: &dyn ::validation::CheckingInput,
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
