//! Optimizer module role: stage group. Exact shift-right custody tests by behavior.

use super::parameter_translation_fixture::{
    exact_integer_shift_left_parameters_plan, integer_type,
};
use super::{AbstractFunction, NativeTarget};
use crate::{
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineExactIntegerShiftLeftParametersTranslationError, lower_to_target_operations,
    validate_abstract_to_target_translation,
};

mod positive;
mod source_corruption;
mod target_corruption;

fn base_source() -> abstract_operations::AbstractOperationPlan {
    exact_integer_shift_left_parameters_plan(
        &[
            semantic_vocabulary::ScalarType::Integer(integer_type(
                semantic_vocabulary::IntegerSign::Signed,
                32,
            )),
            semantic_vocabulary::ScalarType::Integer(integer_type(
                semantic_vocabulary::IntegerSign::Unsigned,
                16,
            )),
        ],
        0,
        1,
    )
}

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineExactIntegerShiftLeftParametersTranslationError {
    let mut source = base_source();
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::integer::shift::exact_left::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLineExactIntegerShiftLeftParametersTranslationError {
    let source = base_source();
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineExactIntegerShiftLeftParameters,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineExactIntegerShiftLeftParameters(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("exact-shift-left corruption must fail at its independent validator")
    };
    error
}
