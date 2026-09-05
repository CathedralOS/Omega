//! Optimizer module role: executable entrance. Source-only grammar map for direct and expression parameter use.

pub(super) mod boolean_equal;
pub(super) mod boolean_not;
pub(super) mod direct;
mod envelope;
pub(super) mod integer;

use abstract_operations::AbstractFunction;
use semantic_vocabulary::ScalarType;

use super::super::StraightLineBooleanEqualParametersTranslationError;
use super::super::StraightLineBooleanNotParameterTranslationError;
use super::{
    StraightLineParameterReconstructionError,
    model::{
        BooleanEqualParametersSource, BooleanNotParameterSource, ParameterResultKind,
        ParameterReturnSource,
    },
};

pub(super) fn has_candidate_envelope(
    function: &AbstractFunction,
    result_kind: ParameterResultKind,
) -> bool {
    envelope::is_candidate(function, result_kind)
}

pub(super) fn reconstruct_direct(
    function: &AbstractFunction,
    expected_result: ScalarType,
) -> Result<ParameterReturnSource, StraightLineParameterReconstructionError> {
    let envelope = envelope::reconstruct(function, expected_result)?;
    direct::reconstruct(function, &envelope, expected_result)
}

pub(super) fn reconstruct_boolean_not(
    function: &AbstractFunction,
) -> Result<BooleanNotParameterSource, StraightLineBooleanNotParameterTranslationError> {
    let envelope = envelope::reconstruct(function, ScalarType::Boolean)?;
    boolean_not::reconstruct(function, &envelope)
}

pub(super) fn reconstruct_boolean_equal(
    function: &AbstractFunction,
) -> Result<BooleanEqualParametersSource, StraightLineBooleanEqualParametersTranslationError> {
    let envelope = envelope::reconstruct(function, ScalarType::Boolean)?;
    boolean_equal::reconstruct(function, &envelope)
}
