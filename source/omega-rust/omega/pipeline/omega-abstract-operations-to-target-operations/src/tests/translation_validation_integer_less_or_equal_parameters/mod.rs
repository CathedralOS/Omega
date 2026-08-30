//! Optimizer module role: stage group.
use super::parameter_translation_fixture::{
    integer_less_or_equal_parameters_plan, integer_type, uniform_integer_less_or_equal_plan,
};
use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineIntegerLessOrEqualParametersTranslationError, lower_to_target_operations,
    validate_abstract_to_target_translation,
};
use omega_target_operations::TerminalPsiProvenance;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerLessOrEqualParametersTranslationError {
    let mut source = uniform_integer_less_or_equal_plan(integer_type(IntegerSign::Signed, 32), 2);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::integer::comparison::less_or_equal::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineIntegerLessOrEqualParametersTranslationError {
    let source = uniform_integer_less_or_equal_plan(integer_type(IntegerSign::Unsigned, 64), 2);
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualParameters,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineIntegerLessOrEqualParameters(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("integer less-or-equal corruption must fail at its independent validator")
    };
    error
}

mod positive;
mod source_corruption;
mod target_corruption;
