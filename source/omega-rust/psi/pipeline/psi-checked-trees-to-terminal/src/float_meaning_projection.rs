//! Exact erasure of checked float-meaning projections into Terminal Psi.

use psi_checked_trees::{
    CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
    CheckedFloatMeaningProjectionError, CheckedFloatProjectionSource, CheckedProofOnlyValueType,
    types::PrimitiveType,
};
use psi_core::IeeeFloatFormat;
use psi_terminal::{
    FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
    FloatMeaningSource, FloatProjectionInput, FloatProjectionInputId, ProofOnlyValueType,
    ProofPropositionId, ProofValueDeclaration, ProofValueId,
};

pub fn lower_float_meaning_equality(
    checked: CheckedFloatMeaningEqualityProposition,
) -> FloatMeaningEqualityProposition {
    FloatMeaningEqualityProposition {
        id: ProofPropositionId(checked.id.0),
        left: ProofValueId(checked.left.0),
        right: ProofValueId(checked.right.0),
    }
}

pub fn lower_float_meaning_projection(
    checked: CheckedFloatMeaningProjection,
) -> Result<FloatMeaningProjection, FloatMeaningProjectionLoweringError> {
    checked
        .validate()
        .map_err(FloatMeaningProjectionLoweringError::InvalidCheckedProjection)?;
    let source = match checked.source {
        CheckedFloatProjectionSource::TransitionalInput(input) => {
            let format = match input.primitive {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
            };
            FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                id: FloatProjectionInputId(input.id.0),
                format,
            })
        }
        CheckedFloatProjectionSource::ExactBinary32Literal(bits) => {
            FloatMeaningSource::ExactBinary32Literal(bits)
        }
        CheckedFloatProjectionSource::ExactBinary64Literal(bits) => {
            FloatMeaningSource::ExactBinary64Literal(bits)
        }
    };
    let value_type = match checked.result.value_type {
        CheckedProofOnlyValueType::FloatMeaning => ProofOnlyValueType::FloatMeaning,
    };
    let operation = match checked.operation {
        psi_numerics::float_projection::FloatProjectionOperation::Meaning32 => {
            FloatMeaningProjectionOperation::Meaning32
        }
        psi_numerics::float_projection::FloatProjectionOperation::Meaning64 => {
            FloatMeaningProjectionOperation::Meaning64
        }
    };
    Ok(FloatMeaningProjection {
        result: ProofValueDeclaration {
            id: ProofValueId(checked.result.id.0),
            value_type,
        },
        source,
        operation,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatMeaningProjectionLoweringError {
    InvalidCheckedProjection(CheckedFloatMeaningProjectionError),
    InvalidSourceCarrier,
}

#[cfg(test)]
mod tests {
    use psi_checked_trees::{
        CheckedFloatProjectionInput, CheckedFloatProjectionInputId, CheckedProofValueDeclaration,
        CheckedProofValueId,
    };
    use psi_numerics::float_projection::FloatProjectionOperation;

    use super::*;

    fn checked_projection() -> CheckedFloatMeaningProjection {
        CheckedFloatMeaningProjection {
            result: CheckedProofValueDeclaration {
                id: CheckedProofValueId(4),
                value_type: CheckedProofOnlyValueType::FloatMeaning,
            },
            source: CheckedFloatProjectionSource::TransitionalInput(CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(9),
                primitive: PrimitiveType::F64,
            }),
            operation: FloatProjectionOperation::Meaning64,
        }
    }
    #[test]
    fn lowering_preserves_dense_identities_and_exact_format() {
        let lowered = lower_float_meaning_projection(checked_projection()).unwrap();
        assert_eq!(lowered.result.id, ProofValueId(4));
        assert_eq!(
            lowered.source,
            FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                id: FloatProjectionInputId(9),
                format: IeeeFloatFormat::Binary64,
            })
        );
        assert_eq!(
            lowered.operation,
            FloatMeaningProjectionOperation::Meaning64
        );
    }

    #[test]
    fn lowering_rejects_cross_format_checked_operation() {
        let mut checked = checked_projection();
        checked.operation = FloatProjectionOperation::Meaning32;
        assert_eq!(
            lower_float_meaning_projection(checked),
            Err(
                FloatMeaningProjectionLoweringError::InvalidCheckedProjection(
                    CheckedFloatMeaningProjectionError::SourceFormatMismatch,
                )
            )
        );
    }

    #[test]
    fn lowering_preserves_exact_literal_bits_without_a_producer_coordinate() {
        let mut checked = checked_projection();
        checked.source = CheckedFloatProjectionSource::ExactBinary64Literal(0x8000_0000_0000_0000);
        let lowered = lower_float_meaning_projection(checked).unwrap();
        assert_eq!(
            lowered.source,
            FloatMeaningSource::ExactBinary64Literal(0x8000_0000_0000_0000)
        );
    }
}
