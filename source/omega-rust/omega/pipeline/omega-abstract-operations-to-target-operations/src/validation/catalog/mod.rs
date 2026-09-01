//! Optimizer module role: executable entrance. Ordered inventory and exact-zero-or-one classification of replay families.
//! Adding or disabling a family happens only in `ENABLED_TRANSLATION_FAMILIES`.
//! Every row visibly joins one source classifier to one typed replay adapter.

mod dispatch;
mod model;
mod selection;

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetTranslationValidationError,
};
use model::TranslationFamilyDescriptor;

const ENABLED_TRANSLATION_FAMILIES: &[TranslationFamilyDescriptor] = &[
    dispatch::immediate::INTEGER,
    dispatch::immediate::BOOLEAN,
    dispatch::terminal::SCALAR_CRASH,
    dispatch::parameter::direct::INTEGER,
    dispatch::parameter::direct::BOOLEAN,
    dispatch::parameter::unary::BOOLEAN_NOT,
    dispatch::parameter::comparison::BOOLEAN_EQUAL,
    dispatch::parameter::comparison::INTEGER_EQUAL,
    dispatch::parameter::comparison::INTEGER_LESS_THAN,
    dispatch::parameter::comparison::INTEGER_LESS_OR_EQUAL,
    dispatch::parameter::unary::INTEGER_BITWISE_NOT,
    dispatch::parameter::unary::INTEGER_WIDEN,
    dispatch::parameter::unary::INTEGER_EXACT_CAST,
    dispatch::parameter::bitwise::INTEGER_AND,
    dispatch::parameter::bitwise::INTEGER_OR,
    dispatch::parameter::bitwise::INTEGER_XOR,
    dispatch::parameter::arithmetic::EXACT_INTEGER_ADD,
    dispatch::parameter::arithmetic::EXACT_INTEGER_SUBTRACT,
    dispatch::parameter::arithmetic::EXACT_INTEGER_MULTIPLY,
    dispatch::parameter::arithmetic::EXACT_INTEGER_DIVIDE,
    dispatch::parameter::arithmetic::EXACT_INTEGER_REMAINDER,
    dispatch::parameter::arithmetic::WRAPPING_INTEGER_DIVIDE,
    dispatch::parameter::arithmetic::WRAPPING_INTEGER_REMAINDER,
    dispatch::parameter::arithmetic::SATURATING_INTEGER_ADD,
    dispatch::parameter::arithmetic::WRAPPING_INTEGER_ADD,
    dispatch::parameter::arithmetic::SATURATING_INTEGER_SUBTRACT,
    dispatch::parameter::arithmetic::WRAPPING_INTEGER_SUBTRACT,
    dispatch::parameter::arithmetic::WRAPPING_INTEGER_MULTIPLY,
    dispatch::parameter::arithmetic::SATURATING_INTEGER_MULTIPLY,
];

pub(super) fn validate_function(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetTranslationValidationError,
> {
    selection::validate(
        source,
        expected_target,
        target,
        ENABLED_TRANSLATION_FAMILIES,
    )
}

#[cfg(test)]
mod tests;
