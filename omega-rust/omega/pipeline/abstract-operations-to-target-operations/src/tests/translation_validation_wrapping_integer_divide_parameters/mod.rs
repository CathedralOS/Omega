//! Optimizer module role: stage group. Wrapping-divide translation custody tests by behavior.

use super::parameter_translation_fixture::{
    integer_type, uniform_wrapping_integer_divide_plan, wrapping_integer_divide_parameters_plan,
};
use super::{AbstractFunction, IntegerSign, NativeTarget};
use crate::{
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineWrappingIntegerDivideParametersTranslationError, lower_to_target_operations,
    validate_abstract_to_target_translation,
};

mod positive;
mod source_corruption;
mod target_corruption;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineWrappingIntegerDivideParametersTranslationError {
    let mut source = uniform_wrapping_integer_divide_plan(integer_type(IntegerSign::Signed, 32), 2);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::integer::arithmetic::wrapping_divide::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLineWrappingIntegerDivideParametersTranslationError {
    let source = uniform_wrapping_integer_divide_plan(integer_type(IntegerSign::Unsigned, 64), 2);
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineWrappingIntegerDivideParameters,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerDivideParameters(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("wrapping-integer-divide corruption must fail at its independent validator")
    };
    error
}
