//! Optimizer module role: stage group.
use super::parameter_translation_fixture::{
    integer_bitwise_and_parameters_plan, integer_type, uniform_integer_bitwise_and_plan,
};
use super::*;
use crate::{
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineIntegerBitwiseAndParametersTranslationError, lower_to_target_operations,
    validate_abstract_to_target_translation,
};

mod positive;
mod source_corruption;
mod target_corruption;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerBitwiseAndParametersTranslationError {
    let mut source = uniform_integer_bitwise_and_plan(integer_type(IntegerSign::Signed, 32), 2);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::integer::bitwise::bitwise_and::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLineIntegerBitwiseAndParametersTranslationError {
    let source = uniform_integer_bitwise_and_plan(integer_type(IntegerSign::Unsigned, 64), 2);
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseAndParameters,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineIntegerBitwiseAndParameters(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("integer bitwise-AND corruption must fail at its independent validator")
    };
    error
}
