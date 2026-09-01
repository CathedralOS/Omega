//! Shared scalar fixture projections used across lowering test families.

use super::*;

pub(crate) fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
    }
}

pub(crate) fn scalar_result(function: &AbstractFunction) -> AbstractResult {
    function.result.scalar().expect("fixture is scalar")
}

pub(crate) fn scalar_result_mut(function: &mut AbstractFunction) -> &mut AbstractResult {
    let AbstractFunctionResult::Scalar(result) = &mut function.result else {
        panic!("fixture is scalar")
    };
    result
}
