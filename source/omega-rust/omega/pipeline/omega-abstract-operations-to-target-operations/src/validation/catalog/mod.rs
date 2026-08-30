//! Optimizer module role: executable entrance. Ordered inventory and exact-zero-or-one classification of replay families.
//!
//! Adding or disabling a family happens only in `ENABLED_TRANSLATION_FAMILIES`.
//! Every row visibly joins one source classifier to one typed replay adapter.

mod dispatch;
mod model;

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
    dispatch::parameter::arithmetic::WRAPPING_INTEGER_ADD,
];

pub(super) fn validate_function(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetTranslationValidationError,
> {
    validate_function_with_catalog(
        source,
        expected_target,
        target,
        ENABLED_TRANSLATION_FAMILIES,
    )
}

fn validate_function_with_catalog(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    catalog: &[TranslationFamilyDescriptor],
) -> Result<
    AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetTranslationValidationError,
> {
    let mut selected: Option<&TranslationFamilyDescriptor> = None;
    for descriptor in catalog {
        if !(descriptor.is_candidate)(source) {
            continue;
        }
        if let Some(first) = selected {
            return Err(
                AbstractToTargetTranslationValidationError::AmbiguousFunctionFamily {
                    machine: source.machine,
                    first: first.family,
                    second: descriptor.family,
                },
            );
        }
        selected = Some(descriptor);
    }
    let Some(descriptor) = selected else {
        return Ok(AbstractToTargetFunctionTranslationDisposition::Uncovered);
    };
    (descriptor.validate)(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationDisposition::Validated)
        .map_err(
            |error| AbstractToTargetTranslationValidationError::FunctionFamily {
                machine: source.machine,
                family: descriptor.family,
                error,
            },
        )
}

#[cfg(test)]
mod tests;
