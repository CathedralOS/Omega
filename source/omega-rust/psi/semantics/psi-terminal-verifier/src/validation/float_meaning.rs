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
    let mut sources = Vec::new();
    let mut projection_keys = Vec::new();
    for (index, projection) in projections.iter().enumerate() {
        let index =
            u32::try_from(index).map_err(|_| ModuleError::NonDenseFloatMeaningProjection {
                expected: u32::MAX,
                result: projection.result.id.0,
                transitional_source: transitional_source_id(projection.source),
            })?;
        if projection.result.id.0 != index {
            return Err(ModuleError::NonDenseFloatMeaningProjection {
                expected: index,
                result: projection.result.id.0,
                transitional_source: transitional_source_id(projection.source),
            });
        }
        if let psi_terminal::FloatMeaningSource::TransitionalInput(source) = projection.source {
            if let Some((_, format)) = sources.iter().find(|(id, _)| *id == source.id) {
                if *format != source.format {
                    return Err(
                        ModuleError::InconsistentFloatMeaningProjectionSourceFormat {
                            source: source.id.0,
                        },
                    );
                }
            } else {
                let expected = u32::try_from(sources.len()).map_err(|_| {
                    ModuleError::NonDenseFloatMeaningProjection {
                        expected: u32::MAX,
                        result: projection.result.id.0,
                        transitional_source: Some(source.id.0),
                    }
                })?;
                if source.id.0 != expected {
                    return Err(ModuleError::NonDenseFloatMeaningProjection {
                        expected,
                        result: projection.result.id.0,
                        transitional_source: Some(source.id.0),
                    });
                }
                sources.push((source.id, source.format));
            }
        }
        let key = (projection.source, projection.operation);
        if let Some(first) = projection_keys.iter().position(|existing| *existing == key) {
            return Err(ModuleError::DuplicateFloatMeaningProjection {
                first: u32::try_from(first).unwrap_or(u32::MAX),
                duplicate: index,
            });
        }
        projection_keys.push(key);
        crate::verification::reconstruct_float_meaning_projection(projection)
            .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
    }
    Ok(())
}

const fn transitional_source_id(source: psi_terminal::FloatMeaningSource) -> Option<u32> {
    match source {
        psi_terminal::FloatMeaningSource::TransitionalInput(input) => Some(input.id.0),
        psi_terminal::FloatMeaningSource::ExactBinary32Literal(_)
        | psi_terminal::FloatMeaningSource::ExactBinary64Literal(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use psi_core::IeeeFloatFormat;
    use psi_terminal::{
        FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
        FloatMeaningSource, FloatProjectionInput, FloatProjectionInputId, ProofOnlyValueType,
        ProofPropositionId, ProofValueDeclaration, ProofValueId,
    };

    use super::*;

    fn projection(index: u32) -> FloatMeaningProjection {
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(index),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                id: FloatProjectionInputId(index),
                format: IeeeFloatFormat::Binary32,
            }),
            operation: FloatMeaningProjectionOperation::Meaning32,
        }
    }

    fn exact_binary32_projection(index: u32, bits: u32) -> FloatMeaningProjection {
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(index),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatMeaningSource::ExactBinary32Literal(bits),
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
        tampered.source = FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(0),
            format: IeeeFloatFormat::Binary64,
        });
        assert!(matches!(
            validate_rows(&[tampered]),
            Err(ModuleError::InvalidFloatMeaningProjection { .. })
        ));
    }

    #[test]
    fn module_rows_reject_duplicate_values_and_inconsistent_source_formats() {
        let mut duplicate = projection(1);
        duplicate.source = FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(0),
            format: IeeeFloatFormat::Binary32,
        });
        assert!(matches!(
            validate_rows(&[projection(0), duplicate]),
            Err(ModuleError::DuplicateFloatMeaningProjection {
                first: 0,
                duplicate: 1,
            })
        ));

        duplicate.source = FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(0),
            format: IeeeFloatFormat::Binary64,
        });
        duplicate.operation = FloatMeaningProjectionOperation::Meaning64;
        assert!(matches!(
            validate_rows(&[projection(0), duplicate]),
            Err(ModuleError::InconsistentFloatMeaningProjectionSourceFormat { source: 0 })
        ));
    }

    #[test]
    fn exact_literal_identity_ignores_producer_numbering_and_retains_raw_bits() {
        let positive_zero = exact_binary32_projection(0, 0x0000_0000);
        let mut duplicate_with_fresh_ids = exact_binary32_projection(1, 0x0000_0000);
        assert!(matches!(
            validate_rows(&[positive_zero, duplicate_with_fresh_ids]),
            Err(ModuleError::DuplicateFloatMeaningProjection {
                first: 0,
                duplicate: 1,
            })
        ));

        duplicate_with_fresh_ids.source = FloatMeaningSource::ExactBinary32Literal(0x8000_0000);
        assert_eq!(
            validate_rows(&[positive_zero, duplicate_with_fresh_ids]),
            Ok(())
        );
    }

    #[test]
    fn exact_literal_rejects_cross_format_operation_substitution() {
        let mut projection = exact_binary32_projection(0, 0x3f80_0000);
        projection.operation = FloatMeaningProjectionOperation::Meaning64;
        assert!(matches!(
            validate_rows(&[projection]),
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
