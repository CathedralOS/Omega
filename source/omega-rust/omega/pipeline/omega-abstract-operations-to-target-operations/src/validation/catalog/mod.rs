//! Ordered inventory and exact-zero-or-one classification of replay families.
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
    dispatch::STRAIGHT_LINE_INTEGER_IMMEDIATE,
    dispatch::STRAIGHT_LINE_BOOLEAN_IMMEDIATE,
    dispatch::STRAIGHT_LINE_SCALAR_CRASH,
    dispatch::STRAIGHT_LINE_INTEGER_PARAMETER,
    dispatch::STRAIGHT_LINE_BOOLEAN_PARAMETER,
    dispatch::STRAIGHT_LINE_BOOLEAN_NOT_PARAMETER,
    dispatch::STRAIGHT_LINE_BOOLEAN_EQUAL_PARAMETERS,
    dispatch::STRAIGHT_LINE_INTEGER_EQUAL_PARAMETERS,
    dispatch::STRAIGHT_LINE_INTEGER_LESS_THAN_PARAMETERS,
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
