//! Optimizer module role: stage group. Exact constant wrapping-shift-left validation tests.

use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineWrappingIntegerShiftLeftImmediateTranslationError,
    tests::support::lower_legacy_scalar_fixture as lower_to_target_operations,
    validate_abstract_to_target_translation,
};

mod fixture;
mod positive;
mod source_corruption;
mod target_corruption;

use fixture::*;
