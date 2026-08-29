use omega_abstract_operations::AbstractFunction;
use omega_target_operations::TargetFunction;

use super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
};
use crate::AbstractToTargetTranslationFamily;

pub(super) type TranslationFamilyClassifier = fn(&AbstractFunction) -> bool;
pub(super) type TranslationFamilyValidator = fn(
    &AbstractFunction,
    &TargetFunction,
) -> Result<
    AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamilyError,
>;

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
        validate: TranslationFamilyValidator,
    ) -> Self {
        Self {
            family,
            is_candidate,
            validate,
        }
    }
}
