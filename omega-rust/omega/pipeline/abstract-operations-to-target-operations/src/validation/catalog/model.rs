use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
};
use crate::AbstractToTargetTranslationFamily;

pub(super) type TranslationFamilyClassifier = fn(&AbstractFunction) -> bool;
pub(super) type PlainTranslationFamilyValidator = fn(
    &AbstractFunction,
    NativeTarget,
    &TargetFunction,
) -> Result<
    AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamilyError,
>;

pub(super) type IeeeFloatFmaTranslationFamilyValidator = fn(
    &AbstractFunction,
    NativeTarget,
    &TargetFunction,
    &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
) -> Result<
    AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamilyError,
>;

#[derive(Clone, Copy)]
pub(super) enum TranslationFamilyValidator {
    Plain(PlainTranslationFamilyValidator),
    IeeeFloatFma(IeeeFloatFmaTranslationFamilyValidator),
}

#[derive(Clone, Copy)]
pub(super) struct TranslationFamilyDescriptor {
    pub(super) family: AbstractToTargetTranslationFamily,
    pub(super) is_candidate: TranslationFamilyClassifier,
    pub(super) validate: TranslationFamilyValidator,
}

impl TranslationFamilyDescriptor {
    pub(super) const fn new(
        family: AbstractToTargetTranslationFamily,
        is_candidate: TranslationFamilyClassifier,
        validate: PlainTranslationFamilyValidator,
    ) -> Self {
        Self {
            family,
            is_candidate,
            validate: TranslationFamilyValidator::Plain(validate),
        }
    }

    pub(super) const fn with_ieee_float_fma(
        family: AbstractToTargetTranslationFamily,
        is_candidate: TranslationFamilyClassifier,
        validate: IeeeFloatFmaTranslationFamilyValidator,
    ) -> Self {
        Self {
            family,
            is_candidate,
            validate: TranslationFamilyValidator::IeeeFloatFma(validate),
        }
    }
}
