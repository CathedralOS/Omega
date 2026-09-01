//! Optimizer module role: stage group. Exact constant-integer-less-or-equal validation tests.

use super::super::*;
use crate::{
    lower_to_target_operations, validate_abstract_to_target_translation,
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineIntegerLessOrEqualImmediateTranslationError,
};

mod fixture;
mod positive;
mod source_corruption;
mod target_corruption;

use fixture::*;
