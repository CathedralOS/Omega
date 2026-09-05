//! Optimizer module role: stage group. Exact-remainder translation custody tests by behavior.

use super::parameter_translation_fixture::{
    exact_integer_remainder_parameters_plan, integer_type, uniform_exact_integer_remainder_plan,
};
use super::{AbstractFunction, IntegerSign, NativeTarget};
use crate::{
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineExactIntegerRemainderParametersTranslationError, lower_to_target_operations,
    validate_abstract_to_target_translation,
};

mod positive;
mod source_corruption;
mod target_corruption;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineExactIntegerRemainderParametersTranslationError {
    let mut source = uniform_exact_integer_remainder_plan(integer_type(IntegerSign::Signed, 32), 2);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::integer::arithmetic::exact_remainder::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLineExactIntegerRemainderParametersTranslationError {
    let source = uniform_exact_integer_remainder_plan(integer_type(IntegerSign::Unsigned, 64), 2);
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineExactIntegerRemainderParameters,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineExactIntegerRemainderParameters(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("exact-integer-remainder corruption must fail at its independent validator")
    };
    error
}
