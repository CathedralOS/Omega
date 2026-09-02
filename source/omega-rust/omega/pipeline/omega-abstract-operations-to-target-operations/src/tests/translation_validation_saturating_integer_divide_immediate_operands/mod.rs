//! Optimizer module role: stage group. Proof-bearing constant saturating-divide validation tests.

use super::*;
use crate::{
    lower_to_target_operations, validate_abstract_to_target_translation,
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError,
};

mod fixture;
mod positive;
mod source_corruption;
mod target_corruption;

use fixture::*;
