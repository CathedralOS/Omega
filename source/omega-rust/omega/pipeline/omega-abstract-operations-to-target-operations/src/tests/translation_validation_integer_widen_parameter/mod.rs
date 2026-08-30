use super::parameter_translation_fixture::{
    integer_type, integer_widen_parameter_plan, uniform_integer_widen_plan,
};
use super::*;
use crate::{
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, StraightLineIntegerWidenParameterTranslationError,
    lower_to_target_operations, validate_abstract_to_target_translation,
};

mod positive;
mod source_corruption;
mod target_corruption;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerWidenParameterTranslationError {
    let mut source = uniform_integer_widen_plan(
        integer_type(IntegerSign::Signed, 16),
        integer_type(IntegerSign::Signed, 32),
        1,
    );
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::integer::unary::widen::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineIntegerWidenParameterTranslationError {
    let source = uniform_integer_widen_plan(
        integer_type(IntegerSign::Unsigned, 32),
        integer_type(IntegerSign::Signed, 64),
        1,
    );
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerWidenParameter,
        error: AbstractToTargetTranslationFamilyError::StraightLineIntegerWidenParameter(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("integer widen corruption must fail at its independent validator")
    };
    error
}

fn legal_native_widenings() -> Vec<(IntegerType, IntegerType)> {
    let mut pairs = Vec::new();
    for source_sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for source_bits in [8, 16, 32] {
            for target_bits in [16, 32, 64] {
                if source_bits < target_bits {
                    pairs.push((
                        integer_type(source_sign, source_bits),
                        integer_type(source_sign, target_bits),
                    ));
                }
            }
        }
    }
    for source_bits in [8, 16, 32] {
        for target_bits in [16, 32, 64] {
            if source_bits < target_bits {
                pairs.push((
                    integer_type(IntegerSign::Unsigned, source_bits),
                    integer_type(IntegerSign::Signed, target_bits),
                ));
            }
        }
    }
    assert_eq!(pairs.len(), 18);
    pairs
}
