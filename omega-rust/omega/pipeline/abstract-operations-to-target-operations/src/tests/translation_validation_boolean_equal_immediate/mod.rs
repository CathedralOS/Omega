//! Optimizer module role: stage group. Exact constant-Boolean-equality validation tests.

use super::super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, StraightLineBooleanEqualImmediateTranslationError,
    tests::support::lower_legacy_scalar_fixture as lower_to_target_operations,
    validate_abstract_to_target_translation,
};

mod fixture;
mod positive;
mod source_corruption;
mod target_corruption;

use fixture::*;
