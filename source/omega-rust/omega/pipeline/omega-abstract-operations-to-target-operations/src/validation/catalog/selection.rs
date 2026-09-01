//! Exact-zero-or-one family selection and typed validator dispatch.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::model::TranslationFamilyDescriptor;
use crate::validation::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetTranslationValidationError,
};

pub(super) fn validate(
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
