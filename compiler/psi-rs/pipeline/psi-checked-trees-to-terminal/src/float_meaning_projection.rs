//! Exact erasure of checked float-meaning projections into Terminal Psi.

use psi_checked_trees::{
    CheckedFloatMeaningProjection, CheckedFloatMeaningProjectionError, CheckedProofOnlyValueType,
    types::PrimitiveType,
};
use psi_core::IeeeFloatFormat;
use psi_terminal::{
    FloatMeaningProjection, FloatMeaningProjectionOperation, FloatProjectionInput,
    FloatProjectionInputId, ProofOnlyValueType, ProofValueDeclaration, ProofValueId,
};

pub fn lower_float_meaning_projection(
    checked: CheckedFloatMeaningProjection,
) -> Result<FloatMeaningProjection, FloatMeaningProjectionLoweringError> {
    checked
        .validate()
        .map_err(FloatMeaningProjectionLoweringError::InvalidCheckedProjection)?;
    let format = match checked.source.primitive {
        PrimitiveType::F32 => IeeeFloatFormat::Binary32,
        PrimitiveType::F64 => IeeeFloatFormat::Binary64,
        _ => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
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
        source: FloatProjectionInput {
            id: FloatProjectionInputId(checked.source.id.0),
            format,
        },
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
            source: CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(9),
                primitive: PrimitiveType::F64,
            },
            operation: FloatProjectionOperation::Meaning64,
        }
    }

    #[test]
    fn lowering_preserves_dense_identities_and_exact_format() {
        let lowered = lower_float_meaning_projection(checked_projection()).unwrap();
        assert_eq!(lowered.result.id, ProofValueId(4));
        assert_eq!(lowered.source.id, FloatProjectionInputId(9));
        assert_eq!(lowered.source.format, IeeeFloatFormat::Binary64);
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
}
