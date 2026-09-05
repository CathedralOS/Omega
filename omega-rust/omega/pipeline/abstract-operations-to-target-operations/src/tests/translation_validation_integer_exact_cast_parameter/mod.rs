//! Optimizer module role: stage group.
use super::parameter_translation_fixture::{
    integer_exact_cast_parameter_plan, integer_type, uniform_integer_exact_cast_plan,
};
use super::*;
use crate::{
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineIntegerExactCastParameterTranslationError, lower_to_target_operations,
    validate_abstract_to_target_translation,
};

mod positive;
mod source_corruption;
mod target_corruption;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerExactCastParameterTranslationError {
    let mut source = uniform_integer_exact_cast_plan(
        integer_type(IntegerSign::Unsigned, 64),
        integer_type(IntegerSign::Unsigned, 8),
        1,
    );
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::integer::unary::exact_cast::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLineIntegerExactCastParameterTranslationError {
    let source = uniform_integer_exact_cast_plan(
        integer_type(IntegerSign::Unsigned, 64),
        integer_type(IntegerSign::Unsigned, 8),
        1,
    );
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerExactCastParameter,
        error: AbstractToTargetTranslationFamilyError::StraightLineIntegerExactCastParameter(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("integer exact-cast corruption must fail at its independent validator")
    };
    error
}

fn legal_native_exact_casts() -> Vec<(IntegerType, IntegerType)> {
    let integers = [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| [8, 16, 32, 64].map(|bits| integer_type(sign, bits)))
        .collect::<Vec<_>>();
    let pairs = integers
        .iter()
        .flat_map(|source| integers.iter().map(move |target| (*source, *target)))
        .filter(|(source, target)| {
            source != target && !source.can_widen_to(*target) && source.can_exact_cast_to(*target)
        })
        .collect::<Vec<_>>();
    assert_eq!(pairs.len(), 38);
    pairs
}
