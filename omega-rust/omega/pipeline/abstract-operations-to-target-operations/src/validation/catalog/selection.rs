//! Exact-zero-or-one family selection and typed validator dispatch.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::model::{TranslationFamilyDescriptor, TranslationFamilyValidator};
use crate::validation::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetTranslationValidationError,
};

#[cfg(test)]
pub(super) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    catalog: &[TranslationFamilyDescriptor],
) -> Result<
    AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetTranslationValidationError,
> {
    validate_with_ieee_float_fma(source, expected_target, target, catalog, &[])
}

pub(super) fn validate_with_ieee_float_fma(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    catalog: &[TranslationFamilyDescriptor],
    ieee_float_fma: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
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
    match descriptor.validate {
        TranslationFamilyValidator::Plain(validate) => validate(source, expected_target, target),
        TranslationFamilyValidator::IeeeFloatFma(validate) => {
            validate(source, expected_target, target, ieee_float_fma)
        }
    }
    .map(AbstractToTargetFunctionTranslationDisposition::Validated)
    .map_err(
        |error| AbstractToTargetTranslationValidationError::FunctionFamily {
            machine: source.machine,
            family: descriptor.family,
            error,
        },
    )
}
