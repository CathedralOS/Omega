//! Independent reconstruction of proof-only float-meaning projection rows.

use psi_core::IeeeFloatFormat;
use psi_numerics::{
    float_projection::{FloatProjectionOperation, FloatProjectionRule},
    float_semantics::FloatFormat,
};
use psi_terminal::{FloatMeaningProjection, FloatMeaningProjectionOperation, ProofOnlyValueType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconstructedFloatMeaningProjection {
    pub result_type: ProofOnlyValueType,
    pub source_format: IeeeFloatFormat,
    pub operation: FloatMeaningProjectionOperation,
    pub rule: FloatProjectionRule,
}

/// Reconstruct one projection only from source-independent Terminal fields and
/// the shared closed catalog. No source declaration or name table participates.
pub fn reconstruct_float_meaning_projection(
    projection: &FloatMeaningProjection,
) -> Result<ReconstructedFloatMeaningProjection, FloatMeaningProjectionVerificationError> {
    if projection.result.value_type != ProofOnlyValueType::FloatMeaning {
        return Err(FloatMeaningProjectionVerificationError::ResultTypeMismatch);
    }
    let catalog_operation = match projection.operation {
        FloatMeaningProjectionOperation::Meaning32 => FloatProjectionOperation::Meaning32,
        FloatMeaningProjectionOperation::Meaning64 => FloatProjectionOperation::Meaning64,
    };
    let rule = catalog_operation.rule();
    let source_format = match projection.source.format {
        IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
        IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
    };
    if rule.source_format != source_format {
        return Err(FloatMeaningProjectionVerificationError::SourceFormatMismatch);
    }
    if !rule.finite_nonzero_is_exact_rational
        || !rule.preserves_signed_zero
        || !rule.preserves_signed_infinity
        || !rule.erases_nan_payload
    {
        return Err(FloatMeaningProjectionVerificationError::IncompleteProjectionLaw);
    }
    Ok(ReconstructedFloatMeaningProjection {
        result_type: projection.result.value_type,
        source_format: projection.source.format,
        operation: projection.operation,
        rule,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatMeaningProjectionVerificationError {
    ResultTypeMismatch,
    SourceFormatMismatch,
    IncompleteProjectionLaw,
}

#[cfg(test)]
mod tests {
    use psi_terminal::{
        FloatProjectionInput, FloatProjectionInputId, ProofValueDeclaration, ProofValueId,
    };

    use super::*;

    fn projection() -> FloatMeaningProjection {
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(2),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatProjectionInput {
                id: FloatProjectionInputId(6),
                format: IeeeFloatFormat::Binary32,
            },
            operation: FloatMeaningProjectionOperation::Meaning32,
        }
    }

    #[test]
    fn verifier_reconstructs_exact_catalog_row_without_names() {
        let reconstructed = reconstruct_float_meaning_projection(&projection()).unwrap();
        assert_eq!(reconstructed.result_type, ProofOnlyValueType::FloatMeaning);
        assert_eq!(reconstructed.source_format, IeeeFloatFormat::Binary32);
        assert_eq!(
            reconstructed.operation,
            FloatMeaningProjectionOperation::Meaning32
        );
        assert_eq!(
            reconstructed.rule,
            FloatProjectionOperation::Meaning32.rule()
        );
    }

    #[test]
    fn verifier_rejects_operation_and_format_substitution() {
        let mut tampered = projection();
        tampered.operation = FloatMeaningProjectionOperation::Meaning64;
        assert_eq!(
            reconstruct_float_meaning_projection(&tampered),
            Err(FloatMeaningProjectionVerificationError::SourceFormatMismatch)
        );

        tampered = projection();
        tampered.source.format = IeeeFloatFormat::Binary64;
        assert_eq!(
            reconstruct_float_meaning_projection(&tampered),
            Err(FloatMeaningProjectionVerificationError::SourceFormatMismatch)
        );
    }
}
