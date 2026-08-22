//! Module-wide validation for proof-only float-meaning projection rows.

use psi_terminal::TerminalModule;

use super::ModuleError;

pub(super) fn validate_float_meaning_projections(
    module: &TerminalModule,
) -> Result<(), ModuleError> {
    validate_rows(&module.float_meaning_projections)
}

fn validate_rows(projections: &[psi_terminal::FloatMeaningProjection]) -> Result<(), ModuleError> {
    for (index, projection) in projections.iter().enumerate() {
        let index =
            u32::try_from(index).map_err(|_| ModuleError::NonDenseFloatMeaningProjection {
                expected: u32::MAX,
                result: projection.result.id.0,
                source: projection.source.id.0,
            })?;
        if projection.result.id.0 != index || projection.source.id.0 != index {
            return Err(ModuleError::NonDenseFloatMeaningProjection {
                expected: index,
                result: projection.result.id.0,
                source: projection.source.id.0,
            });
        }
        crate::verification::reconstruct_float_meaning_projection(projection)
            .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use psi_core::IeeeFloatFormat;
    use psi_terminal::{
        FloatMeaningProjection, FloatMeaningProjectionOperation, FloatProjectionInput,
        FloatProjectionInputId, ProofOnlyValueType, ProofValueDeclaration, ProofValueId,
    };

    use super::*;

    fn projection(index: u32) -> FloatMeaningProjection {
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(index),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatProjectionInput {
                id: FloatProjectionInputId(index),
                format: IeeeFloatFormat::Binary32,
            },
            operation: FloatMeaningProjectionOperation::Meaning32,
        }
    }

    #[test]
    fn module_rows_reconstruct_in_dense_source_free_order() {
        assert_eq!(validate_rows(&[projection(0), projection(1)]), Ok(()));
    }

    #[test]
    fn module_rows_reject_identity_and_format_tampering() {
        assert!(matches!(
            validate_rows(&[projection(1)]),
            Err(ModuleError::NonDenseFloatMeaningProjection { .. })
        ));

        let mut tampered = projection(0);
        tampered.source.format = IeeeFloatFormat::Binary64;
        assert!(matches!(
            validate_rows(&[tampered]),
            Err(ModuleError::InvalidFloatMeaningProjection { .. })
        ));
    }
}
