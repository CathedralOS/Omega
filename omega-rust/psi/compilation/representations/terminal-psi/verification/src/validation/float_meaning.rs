//! Module-wide validation for proof-only float-meaning projection rows.

use terminal_psi::TerminalModule;

use super::ModuleError;

pub(super) fn validate_float_meaning_projections(
    module: &TerminalModule,
) -> Result<(), ModuleError> {
    validate_rows(&module.float_meaning_projections)?;
    validate_direct_sources(module)?;
    validate_equalities(
        &module.float_meaning_projections,
        &module.float_meaning_equalities,
    )
}

fn validate_equalities(
    projections: &[terminal_psi::FloatMeaningProjection],
    equalities: &[terminal_psi::FloatMeaningEqualityProposition],
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
        let mut reconstructed = Vec::with_capacity(2);
        for operand in [proposition.left, proposition.right] {
            let projection = usize::try_from(operand.0)
                .ok()
                .and_then(|index| projections.get(index))
                .filter(|projection| projection.result.id == operand)
                .ok_or(ModuleError::UnknownFloatMeaningEqualityOperand {
                    proposition: expected,
                    operand: operand.0,
                })?;
            reconstructed.push(
                crate::verification::reconstruct_float_meaning_projection(projections, projection)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection {
                        index: operand.0,
                        error,
                    })?,
            );
        }
        let [left, right] = reconstructed.as_slice() else {
            unreachable!("float meaning equality has exactly two operands")
        };
        if left.source_format != right.source_format
            || left.operation != right.operation
            || left.contract != right.contract
        {
            return Err(ModuleError::InvalidFloatMeaningProjection {
                index: proposition.right.0,
                error:
                    crate::verification::FloatMeaningProjectionVerificationError::EqualityCarrierMismatch,
            });
        }
    }
    Ok(())
}

fn validate_rows(projections: &[terminal_psi::FloatMeaningProjection]) -> Result<(), ModuleError> {
    let mut sources = Vec::new();
    let mut projection_keys = Vec::new();
    for (index, projection) in projections.iter().enumerate() {
        let index =
            u32::try_from(index).map_err(|_| ModuleError::NonDenseFloatMeaningProjection {
                expected: u32::MAX,
                result: projection.result.id.0,
                transitional_source: transitional_source_id(&projection.source),
            })?;
        if projection.result.id.0 != index {
            return Err(ModuleError::NonDenseFloatMeaningProjection {
                expected: index,
                result: projection.result.id.0,
                transitional_source: transitional_source_id(&projection.source),
            });
        }
        if let terminal_psi::FloatMeaningSource::TransitionalInput(source) = &projection.source {
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
        let key = (projection.source.clone(), projection.operation);
        if let Some(first) = projection_keys.iter().position(|existing| *existing == key) {
            return Err(ModuleError::DuplicateFloatMeaningProjection {
                first: u32::try_from(first).unwrap_or(u32::MAX),
                duplicate: index,
            });
        }
        projection_keys.push(key);
        crate::verification::reconstruct_float_meaning_projection(projections, projection)
            .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
    }
    Ok(())
}

fn validate_direct_sources(module: &TerminalModule) -> Result<(), ModuleError> {
    for (index, projection) in module.float_meaning_projections.iter().enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        match &projection.source {
            terminal_psi::FloatMeaningSource::DirectMachineParameter(parameter) => {
                crate::verification::verify_direct_float_parameter(module, *parameter)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            terminal_psi::FloatMeaningSource::DirectMachineResult(result) => {
                crate::verification::verify_direct_float_result(module, *result)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            terminal_psi::FloatMeaningSource::DirectBlockParameter(parameter) => {
                crate::verification::verify_direct_block_float_parameter(module, *parameter)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            terminal_psi::FloatMeaningSource::DirectOperationResult(result) => {
                crate::verification::verify_direct_operation_float_result(module, *result)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            terminal_psi::FloatMeaningSource::DirectCallResult(result) => {
                crate::verification::verify_direct_call_float_result(module, *result)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            terminal_psi::FloatMeaningSource::DirectStructuralLeaf(leaf) => {
                crate::verification::verify_direct_structural_float_leaf(module, leaf)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            // Semantic applications carry no runtime coordinates to rejoin;
            // their catalog rejoin and operand checks run in validate_rows.
            terminal_psi::FloatMeaningSource::SemanticApplication(_)
            | terminal_psi::FloatMeaningSource::TransitionalInput(_)
            | terminal_psi::FloatMeaningSource::ExactBinary32Literal(_)
            | terminal_psi::FloatMeaningSource::ExactBinary64Literal(_) => {}
        }
    }
    Ok(())
}

const fn transitional_source_id(source: &terminal_psi::FloatMeaningSource) -> Option<u32> {
    match source {
        terminal_psi::FloatMeaningSource::TransitionalInput(input) => Some(input.id.0),
        terminal_psi::FloatMeaningSource::DirectMachineParameter(_)
        | terminal_psi::FloatMeaningSource::DirectMachineResult(_)
        | terminal_psi::FloatMeaningSource::DirectBlockParameter(_)
        | terminal_psi::FloatMeaningSource::DirectOperationResult(_)
        | terminal_psi::FloatMeaningSource::DirectCallResult(_)
        | terminal_psi::FloatMeaningSource::DirectStructuralLeaf(_)
        | terminal_psi::FloatMeaningSource::ExactBinary32Literal(_)
        | terminal_psi::FloatMeaningSource::ExactBinary64Literal(_)
        | terminal_psi::FloatMeaningSource::SemanticApplication(_) => None,
    }
}
