use super::parameter_translation_fixture::{
    integer_bitwise_xor_parameters_plan, integer_type, uniform_integer_bitwise_xor_plan,
};
use super::*;
use crate::{
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineIntegerBitwiseXorParametersTranslationError, lower_to_target_operations,
    validate_abstract_to_target_translation,
};

mod positive;
mod source_corruption;
mod target_corruption;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerBitwiseXorParametersTranslationError {
    let mut source = uniform_integer_bitwise_xor_plan(integer_type(IntegerSign::Signed, 32), 2);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::integer::bitwise::bitwise_xor::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineIntegerBitwiseXorParametersTranslationError {
    let source = uniform_integer_bitwise_xor_plan(integer_type(IntegerSign::Unsigned, 64), 2);
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseXorParameters,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineIntegerBitwiseXorParameters(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("integer bitwise-XOR corruption must fail at its independent validator")
    };
    error
}
