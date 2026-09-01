//! Independent reconstruction of proof-only float-meaning projection rows.

use psi_core::{IeeeFloatFormat, MachineId, ScalarType, ValueId};
use psi_numerics::{
    float_projection::{FloatProjectionOperation, FloatProjectionRule},
    float_semantics::{FloatFormat, FloatMeaning},
};
use psi_terminal::{
    FloatMeaningProjection, FloatMeaningProjectionOperation, FloatMeaningSource,
    FloatProjectionContractIdentity, ProofOnlyValueType, TerminalModule,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructedFloatMeaningProjection {
    pub result_type: ProofOnlyValueType,
    pub source: FloatMeaningSource,
    pub source_format: IeeeFloatFormat,
    /// Exact payload-erased denotation for a literal source. Transitional
    /// source coordinates cannot populate this field.
    pub literal_meaning: Option<FloatMeaning>,
    pub operation: FloatMeaningProjectionOperation,
    pub contract: FloatProjectionContractIdentity,
    pub rule: FloatProjectionRule,
}

fn terminal_contract_identity(
    operation: FloatProjectionOperation,
) -> FloatProjectionContractIdentity {
    let contract = operation.contract_identity();
    FloatProjectionContractIdentity {
        format: contract.format,
        operation: contract.operation,
        declaration: contract.declaration,
        catalog_version: contract.catalog_version,
        commitment: contract.commitment,
    }
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
    let expected_contract = terminal_contract_identity(catalog_operation);
    if projection.contract != expected_contract {
        return Err(FloatMeaningProjectionVerificationError::ContractIdentityMismatch);
    }
    let source_format = match projection.source.format() {
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
    let literal_meaning = match projection.source {
        FloatMeaningSource::TransitionalInput(_)
        | FloatMeaningSource::DirectMachineParameter(_)
        | FloatMeaningSource::DirectMachineResult(_) => None,
        FloatMeaningSource::ExactBinary32Literal(bits) => {
            Some(FloatMeaning::from_f32(f32::from_bits(bits)))
        }
        FloatMeaningSource::ExactBinary64Literal(bits) => {
            Some(FloatMeaning::from_f64(f64::from_bits(bits)))
        }
    };
    Ok(ReconstructedFloatMeaningProjection {
        result_type: projection.result.value_type,
        source: projection.source,
        source_format: projection.source.format(),
        literal_meaning,
        operation: projection.operation,
        contract: projection.contract,
        rule,
    })
}

/// Rejoin one direct result source to the exact scalar result declaration of
/// its owning Terminal machine. A same-numbered parameter, local, or result of
/// another machine is not interchangeable with this coordinate.
pub(crate) fn verify_direct_float_result(
    module: &TerminalModule,
    result: psi_terminal::DirectMachineFloatResult,
) -> Result<(), FloatMeaningProjectionVerificationError> {
    let mut owners = module
        .machines
        .iter()
        .filter(|machine| machine.id == result.owner);
    let owner = owners
        .next()
        .ok_or(FloatMeaningProjectionVerificationError::InvalidDirectResultOwner(result.owner))?;
    if owners.next().is_some() {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectResultOwner(result.owner),
        );
    }
    let psi_terminal::TerminalMachineResult::Scalar(declaration) = &owner.result else {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectResult {
                owner: result.owner,
                result: result.result,
            },
        );
    };
    if declaration.id != result.result {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectResult {
                owner: result.owner,
                result: result.result,
            },
        );
    }
    if declaration.scalar_type != ScalarType::IeeeFloat(result.format) {
        return Err(FloatMeaningProjectionVerificationError::DirectResultFormatMismatch);
    }
    Ok(())
}

/// Rejoin one artifact-relative source to the complete Terminal machine table.
/// Catalog reconstruction above remains independent of module topology; this
/// companion check proves that a direct source is specifically an owner's
/// declared entry parameter with the exact IEEE format.
pub(crate) fn verify_direct_float_parameter(
    module: &TerminalModule,
    parameter: psi_terminal::DirectMachineFloatParameter,
) -> Result<(), FloatMeaningProjectionVerificationError> {
    let mut owners = module
        .machines
        .iter()
        .filter(|machine| machine.id == parameter.owner);
    let owner = owners.next().ok_or(
        FloatMeaningProjectionVerificationError::InvalidDirectParameterOwner(parameter.owner),
    )?;
    if owners.next().is_some() {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectParameterOwner(parameter.owner),
        );
    }
    let mut parameters = owner
        .parameters
        .iter()
        .filter(|declaration| declaration.id == parameter.parameter);
    let declaration = parameters.next().ok_or(
        FloatMeaningProjectionVerificationError::InvalidDirectParameter {
            owner: parameter.owner,
            parameter: parameter.parameter,
        },
    )?;
    if parameters.next().is_some() {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectParameter {
                owner: parameter.owner,
                parameter: parameter.parameter,
            },
        );
    }
    if declaration.scalar_type != ScalarType::IeeeFloat(parameter.format) {
        return Err(FloatMeaningProjectionVerificationError::DirectParameterFormatMismatch);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatMeaningProjectionVerificationError {
    ResultTypeMismatch,
    SourceFormatMismatch,
    IncompleteProjectionLaw,
    ContractIdentityMismatch,
    InvalidDirectParameterOwner(MachineId),
    InvalidDirectParameter {
        owner: MachineId,
        parameter: ValueId,
    },
    DirectParameterFormatMismatch,
    InvalidDirectResultOwner(MachineId),
    InvalidDirectResult {
        owner: MachineId,
        result: ValueId,
    },
    DirectResultFormatMismatch,
    EqualityCarrierMismatch,
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
            source: FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                id: FloatProjectionInputId(6),
                format: IeeeFloatFormat::Binary32,
            }),
            operation: FloatMeaningProjectionOperation::Meaning32,
            contract: terminal_contract_identity(FloatProjectionOperation::Meaning32),
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
            Err(FloatMeaningProjectionVerificationError::ContractIdentityMismatch)
        );

        tampered = projection();
        tampered.source = FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(6),
            format: IeeeFloatFormat::Binary64,
        });
        assert_eq!(
            reconstruct_float_meaning_projection(&tampered),
            Err(FloatMeaningProjectionVerificationError::SourceFormatMismatch)
        );
    }

    #[test]
    fn verifier_reconstructs_literal_bits_and_payload_erased_meaning() {
        let mut exact = projection();
        exact.source = FloatMeaningSource::ExactBinary32Literal(0x8000_0000);
        let reconstructed = reconstruct_float_meaning_projection(&exact).unwrap();
        assert_eq!(
            reconstructed.source,
            FloatMeaningSource::ExactBinary32Literal(0x8000_0000)
        );
        assert_eq!(
            reconstructed.literal_meaning,
            Some(FloatMeaning::Zero { negative: true })
        );

        exact.source = FloatMeaningSource::ExactBinary32Literal(0x7fc0_0001);
        let first_nan = reconstruct_float_meaning_projection(&exact).unwrap();
        exact.source = FloatMeaningSource::ExactBinary32Literal(0x7fff_ffff);
        let second_nan = reconstruct_float_meaning_projection(&exact).unwrap();
        assert_ne!(first_nan.source, second_nan.source);
        assert_eq!(first_nan.literal_meaning, Some(FloatMeaning::NaN));
        assert_eq!(first_nan.literal_meaning, second_nan.literal_meaning);
    }
}
