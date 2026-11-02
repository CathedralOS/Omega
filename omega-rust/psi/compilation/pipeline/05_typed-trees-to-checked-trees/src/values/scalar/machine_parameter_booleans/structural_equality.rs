//! Contextual data identity over invocation-entry subjects.
//!
//! A proof fact's `Id T left right` is not executable field equality. Retain
//! the two typed subjects instead of expanding records, sums and arrays into
//! scalar comparisons. Neither IEEE equality nor equality of projected
//! FloatMeaning values defines identity of a containing record. The entry
//! contract owns the observation frame; body readers must first establish
//! correspondence to that frame before reusing these subjects.

use super::structural_paths::path_type_reference;
use crate::values::scalar::structural_fields::structural_parameter_field_path;
use checked_trees::{CheckedBooleanExpression, CheckedStructuralParameterField};
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
pub(crate) use validation::contextual_identity_subject_type as subject_type;

/// Called only by the contract/proposition reader, never executable operator
/// selection. Primitive comparisons retain their independently selected rules.
pub(crate) fn lower(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    let (left, right, _) = subjects(program, parameters, left, right)?;
    Some(CheckedBooleanExpression::StructuralIdentity { left, right })
}

pub(crate) fn subjects(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<(
    CheckedStructuralParameterField,
    CheckedStructuralParameterField,
    typed_trees::type_identity::NormalizedTypeIdentity,
)> {
    let subjects = [left, right].map(|expression| {
        let mut path = Vec::new();
        let parameter_position =
            structural_parameter_field_path(program, parameters, expression, &mut path)?;
        let reference = path_type_reference(program, parameters, parameter_position, &path)?;
        let identity = subject_type(program, reference, path.is_empty())?;
        Some((
            CheckedStructuralParameterField {
                parameter_position,
                path,
            },
            identity,
        ))
    });
    let [Some((left, left_type)), Some((right, right_type))] = subjects else {
        return None;
    };
    (left_type == right_type).then_some((left, right, left_type))
}
