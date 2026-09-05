//! Optimizer module role: stage group.
use super::parameter_translation_fixture::{
    integer_bitwise_not_parameter_plan, integer_type, uniform_integer_bitwise_not_plan,
};
use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineIntegerBitwiseNotParameterTranslationError, lower_to_target_operations,
    validate_abstract_to_target_translation,
};
use semantic_vocabulary::{ClaimId, ServiceId, StructuralDomainId};
use target_operations::TerminalPsiProvenance;
use terminal_psi::EntryClaim;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerBitwiseNotParameterTranslationError {
    let mut source = uniform_integer_bitwise_not_plan(integer_type(IntegerSign::Signed, 32), 1);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::integer::unary::bitwise_not::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLineIntegerBitwiseNotParameterTranslationError {
    let source = uniform_integer_bitwise_not_plan(integer_type(IntegerSign::Unsigned, 64), 1);
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotParameter,
        error: AbstractToTargetTranslationFamilyError::StraightLineIntegerBitwiseNotParameter(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("integer bitwise-not corruption must fail at its independent validator")
    };
    error
}

mod positive;
mod source_corruption;
mod target_corruption;
