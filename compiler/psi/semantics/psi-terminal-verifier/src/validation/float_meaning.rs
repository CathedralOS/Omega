//! Module-wide validation for proof-only float-meaning projection rows.

use psi_terminal::TerminalModule;

use super::ModuleError;

pub(super) fn validate_float_meaning_projections(
    module: &TerminalModule,
) -> Result<(), ModuleError> {
    validate_rows(&module.float_meaning_projections)?;
    validate_equalities(
        &module.float_meaning_projections,
        &module.float_meaning_equalities,
    )
}

fn validate_equalities(
    projections: &[psi_terminal::FloatMeaningProjection],
    equalities: &[psi_terminal::FloatMeaningEqualityProposition],
) -> Result<(), ModuleError> {
    for (index, proposition) in equalities.iter().enumerate() {
        let expected =
            u32::try_from(index).map_err(|_| ModuleError::NonDenseFloatMeaningEquality {
                expected: u32::MAX,
                actual: proposition.id.0,
            })?;
        if proposition.id.0 != expected {
            return Err(ModuleError::NonDenseFloatMeaningEquality {
                expected,
                actual: proposition.id.0,
            });
        }
        if proposition.left > proposition.right {
            return Err(ModuleError::NonCanonicalFloatMeaningEqualityOperands {
                proposition: expected,
                left: proposition.left.0,
                right: proposition.right.0,
            });
        }
        for operand in [proposition.left, proposition.right] {
            let projection = usize::try_from(operand.0)
                .ok()
                .and_then(|index| projections.get(index))
                .filter(|projection| projection.result.id == operand)
                .ok_or(ModuleError::UnknownFloatMeaningEqualityOperand {
                    proposition: expected,
                    operand: operand.0,
                })?;
            crate::verification::reconstruct_float_meaning_projection(projection).map_err(
                |error| ModuleError::InvalidFloatMeaningProjection {
                    index: operand.0,
                    error,
                },
            )?;
        }
    }
    Ok(())
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
        FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
        FloatProjectionInput, FloatProjectionInputId, ProofOnlyValueType, ProofPropositionId,
        ProofValueDeclaration, ProofValueId,
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

    #[test]
    fn equality_rows_consume_exact_projection_results() {
        let projections = vec![projection(0), projection(1)];
        let equalities = vec![FloatMeaningEqualityProposition {
            id: ProofPropositionId(0),
            left: ProofValueId(0),
            right: ProofValueId(1),
        }];
        assert_eq!(validate_equalities(&projections, &equalities), Ok(()));
    }

    #[test]
    fn equality_rows_reject_operand_and_identity_tampering() {
        let projections = vec![projection(0), projection(1)];
        let mut equalities = vec![FloatMeaningEqualityProposition {
            id: ProofPropositionId(0),
            left: ProofValueId(0),
            right: ProofValueId(2),
        }];
        assert!(matches!(
            validate_equalities(&projections, &equalities),
            Err(ModuleError::UnknownFloatMeaningEqualityOperand { .. })
        ));

        equalities[0] = FloatMeaningEqualityProposition {
            id: ProofPropositionId(1),
            left: ProofValueId(0),
            right: ProofValueId(1),
        };
        assert!(matches!(
            validate_equalities(&projections, &equalities),
            Err(ModuleError::NonDenseFloatMeaningEquality { .. })
        ));
    }
}
