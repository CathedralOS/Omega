//! Independent reconstruction of proof-only float-meaning projection rows.

use numerics::{
    float_projection::{FloatProjectionOperation, FloatProjectionRule},
    float_semantics::{FloatFormat, FloatMeaning},
    float_semantics_catalog::{
        FloatSemanticContractIdentity, FloatSemanticOperand, FloatSemanticOperation,
        FloatSemanticResult, FloatSemanticValueKind,
    },
};
use semantic_vocabulary::{BlockId, IeeeFloatFormat, MachineId, ScalarType, ValueId};
use terminal_psi::{
    FloatMeaningProjection, FloatMeaningProjectionOperation, FloatMeaningSource,
    FloatProjectionContractIdentity, FloatSemanticApplication, FloatSemanticApplicationOperand,
    ProofOnlyValueType, TerminalModule,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructedFloatMeaningProjection {
    pub result_type: ProofOnlyValueType,
    pub source: FloatMeaningSource,
    pub source_format: IeeeFloatFormat,
    /// Exact payload-erased denotation for a literal source or a semantic
    /// application every operand of which already reconstructed a literal
    /// meaning — the bound catalog kernel is replayed over those operands and
    /// the discharged meaning is retained here. Transitional and direct
    /// source coordinates cannot populate this field.
    pub literal_meaning: Option<FloatMeaning>,
    /// The shared catalog row a semantic-application source rejoined to; all
    /// other source classes leave this empty.
    pub semantic_operation: Option<&'static FloatSemanticOperation>,
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

/// Reconstruct one projection from source-independent Terminal fields and the
/// shared closed catalogs. `projections` is the module's complete projection
/// table: a semantic application names its meaning operands by row, and the
/// operand must precede the applying row, so the dense proof-value order keeps
/// every recursive rejoin well-founded.
pub fn reconstruct_float_meaning_projection(
    projections: &[FloatMeaningProjection],
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
    let mut semantic_operation = None;
    let literal_meaning = match &projection.source {
        FloatMeaningSource::TransitionalInput(_)
        | FloatMeaningSource::DirectMachineParameter(_)
        | FloatMeaningSource::DirectMachineResult(_)
        | FloatMeaningSource::DirectBlockParameter(_)
        | FloatMeaningSource::DirectOperationResult(_)
        | FloatMeaningSource::DirectCallResult(_) => None,
        FloatMeaningSource::DirectStructuralLeaf(_) => None,
        FloatMeaningSource::ExactBinary32Literal(bits) => {
            Some(FloatMeaning::from_f32(f32::from_bits(*bits)))
        }
        FloatMeaningSource::ExactBinary64Literal(bits) => {
            Some(FloatMeaning::from_f64(f64::from_bits(*bits)))
        }
        FloatMeaningSource::SemanticApplication(application) => {
            let (operation, meaning) =
                verify_semantic_application(projections, projection, application)?;
            semantic_operation = Some(operation);
            meaning
        }
    };
    Ok(ReconstructedFloatMeaningProjection {
        result_type: projection.result.value_type,
        source: projection.source.clone(),
        source_format: projection.source.format(),
        literal_meaning,
        semantic_operation,
        operation: projection.operation,
        contract: projection.contract,
        rule,
    })
}

/// Rejoin one semantic-application source: the contract identity selects the
/// catalog row, the operand list must spell that row's signature position by
/// position, and every `Meaning` operand must name a strictly earlier proof
/// row — a forward, self, or dangling reference has no well-founded
/// reconstruction. `Format` parameters admit only a sealed IEEE binary format
/// and must equal the application's declared result format; on rows without a
/// `Format` parameter (`minimum`, `maximum`) the result format exists only as
/// the shared format of the meaning operands, so a mismatched operand format
/// rejects. When every operand already reconstructs a literal meaning the
/// bound kernel is discharged and the result meaning retained; the discharge
/// still cannot outrun the named contract — a kernel result that disagrees
/// with the row's result kind or contract rejects.
fn verify_semantic_application(
    projections: &[FloatMeaningProjection],
    projection: &FloatMeaningProjection,
    application: &FloatSemanticApplication,
) -> Result<
    (&'static FloatSemanticOperation, Option<FloatMeaning>),
    FloatMeaningProjectionVerificationError,
> {
    let identity = FloatSemanticContractIdentity {
        row: application.contract.row,
        catalog_version: application.contract.catalog_version,
        commitment: application.contract.commitment,
    };
    let row = FloatSemanticOperation::for_contract_identity(&identity)
        .ok_or(FloatMeaningProjectionVerificationError::SemanticApplicationContractMismatch)?;
    if row.result != FloatSemanticValueKind::Meaning {
        return Err(FloatMeaningProjectionVerificationError::SemanticApplicationResultKindMismatch);
    }
    if application.operands.len() != row.parameters.len() {
        return Err(
            FloatMeaningProjectionVerificationError::SemanticApplicationOperandCountMismatch,
        );
    }
    let format_free = !row.parameters.contains(&FloatSemanticValueKind::Format);
    let mut discharged = Vec::with_capacity(application.operands.len());
    for (operand_index, (operand, kind)) in application
        .operands
        .iter()
        .zip(row.parameters.iter())
        .enumerate()
    {
        let operand_index = u32::try_from(operand_index).unwrap_or(u32::MAX);
        match (operand, kind) {
            (FloatSemanticApplicationOperand::Format(format), FloatSemanticValueKind::Format) => {
                if *format != application.format {
                    return Err(
                        FloatMeaningProjectionVerificationError::SemanticApplicationFormatMismatch,
                    );
                }
                discharged.push(FloatSemanticOperand::Format(match format {
                    IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
                    IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
                }));
            }
            (FloatSemanticApplicationOperand::Meaning(value), FloatSemanticValueKind::Meaning) => {
                let operand_row = if value.0 < projection.result.id.0 {
                    usize::try_from(value.0)
                        .ok()
                        .and_then(|index| projections.get(index))
                        .filter(|row| row.result.id == *value)
                } else {
                    None
                }
                .ok_or(
                    FloatMeaningProjectionVerificationError::SemanticApplicationOperandRow {
                        operand: operand_index,
                    },
                )?;
                let reconstructed = reconstruct_float_meaning_projection(projections, operand_row)?;
                if format_free && reconstructed.source_format != application.format {
                    return Err(
                        FloatMeaningProjectionVerificationError::SemanticApplicationFormatMismatch,
                    );
                }
                match reconstructed.literal_meaning {
                    Some(meaning) => {
                        discharged.push(FloatSemanticOperand::Meaning(meaning));
                    }
                    None => {
                        discharged.clear();
                        return Ok((row, None));
                    }
                }
            }
            _ => {
                return Err(
                    FloatMeaningProjectionVerificationError::SemanticApplicationOperandKindMismatch {
                        operand: operand_index,
                    },
                );
            }
        }
    }
    let Some(discharge) = row.kernel_discharge(&discharged) else {
        return Err(FloatMeaningProjectionVerificationError::SemanticApplicationDischargeMismatch);
    };
    if discharge.contract != identity {
        return Err(FloatMeaningProjectionVerificationError::SemanticApplicationDischargeMismatch);
    }
    match discharge.result {
        FloatSemanticResult::Meaning(meaning) => Ok((row, Some(meaning))),
        _ => Err(FloatMeaningProjectionVerificationError::SemanticApplicationDischargeMismatch),
    }
}

/// Rejoin one structural IEEE source to an owner's direct structural
/// parameter and complete canonical leaf path.
pub(crate) fn verify_direct_structural_float_leaf(
    module: &TerminalModule,
    leaf: &terminal_psi::DirectStructuralFloatLeaf,
) -> Result<(), FloatMeaningProjectionVerificationError> {
    let mut owners = module
        .machines
        .iter()
        .filter(|machine| machine.id == leaf.owner);
    let owner = owners.next().ok_or(
        FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeafOwner(leaf.owner),
    )?;
    if owners.next().is_some() {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeafOwner(leaf.owner),
        );
    }
    let parameter = owner
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == leaf.field.root())
        .ok_or(
            FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeaf {
                owner: leaf.owner,
            },
        )?;
    if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return Err(
            FloatMeaningProjectionVerificationError::DirectStructuralLeafWriteOnlyRoot {
                owner: leaf.owner,
            },
        );
    }
    let field_type = crate::validation::structural_leaf_type(
        module,
        owner,
        leaf.field.root(),
        leaf.field.path(),
    )
    .ok_or(
        FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeaf { owner: leaf.owner },
    )?;
    let terminal_psi::StructuralFieldType::IeeeFloat(actual) = field_type else {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeaf {
                owner: leaf.owner,
            },
        );
    };
    if *actual != leaf.format {
        return Err(FloatMeaningProjectionVerificationError::DirectStructuralLeafFormatMismatch);
    }
    Ok(())
}

/// Rejoin one direct result source to the exact scalar result declaration of
/// its owning Terminal machine. A same-numbered parameter, local, or result of
/// another machine is not interchangeable with this coordinate.
pub(crate) fn verify_direct_float_result(
    module: &TerminalModule,
    result: terminal_psi::DirectMachineFloatResult,
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
    let terminal_psi::TerminalMachineResult::Scalar(declaration) = &owner.result else {
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
    parameter: terminal_psi::DirectMachineFloatParameter,
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

/// Rejoin one artifact-relative source to the exact scalar parameter table of
/// one block in its owning Terminal machine.
pub(crate) fn verify_direct_block_float_parameter(
    module: &TerminalModule,
    parameter: terminal_psi::DirectBlockFloatParameter,
) -> Result<(), FloatMeaningProjectionVerificationError> {
    let mut owners = module
        .machines
        .iter()
        .filter(|machine| machine.id == parameter.owner);
    let owner = owners.next().ok_or(
        FloatMeaningProjectionVerificationError::InvalidDirectBlockParameterOwner(parameter.owner),
    )?;
    if owners.next().is_some() {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectBlockParameterOwner(
                parameter.owner,
            ),
        );
    }
    let mut blocks = owner
        .blocks
        .iter()
        .filter(|block| block.id == parameter.block);
    let block = blocks.next().ok_or(
        FloatMeaningProjectionVerificationError::InvalidDirectBlockParameterBlock {
            owner: parameter.owner,
            block: parameter.block,
        },
    )?;
    if blocks.next().is_some() {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectBlockParameterBlock {
                owner: parameter.owner,
                block: parameter.block,
            },
        );
    }
    let mut parameters = block
        .parameters
        .iter()
        .filter(|declaration| declaration.id == parameter.parameter);
    let declaration = parameters.next().ok_or(
        FloatMeaningProjectionVerificationError::InvalidDirectBlockParameter {
            owner: parameter.owner,
            block: parameter.block,
            parameter: parameter.parameter,
        },
    )?;
    if parameters.next().is_some() {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectBlockParameter {
                owner: parameter.owner,
                block: parameter.block,
                parameter: parameter.parameter,
            },
        );
    }
    if declaration.scalar_type != ScalarType::IeeeFloat(parameter.format) {
        return Err(FloatMeaningProjectionVerificationError::DirectBlockParameterFormatMismatch);
    }
    Ok(())
}

/// Rejoin one artifact-relative source to the exact scalar result declared by
/// one operation in its owning Terminal machine.
pub(crate) fn verify_direct_operation_float_result(
    module: &TerminalModule,
    result: terminal_psi::DirectOperationFloatResult,
) -> Result<(), FloatMeaningProjectionVerificationError> {
    let mut owners = module
        .machines
        .iter()
        .filter(|machine| machine.id == result.owner);
    let owner = owners.next().ok_or(
        FloatMeaningProjectionVerificationError::InvalidDirectOperationResultOwner(result.owner),
    )?;
    if owners.next().is_some() {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectOperationResultOwner(
                result.owner,
            ),
        );
    }
    let mut producers = owner
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == result.producer);
    let producer = producers.next().ok_or(
        FloatMeaningProjectionVerificationError::InvalidDirectOperationResultProducer {
            owner: result.owner,
            producer: result.producer,
        },
    )?;
    if producers.next().is_some() {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectOperationResultProducer {
                owner: result.owner,
                producer: result.producer,
            },
        );
    }
    if matches!(
        producer.kind,
        terminal_psi::OperationKind::Call { .. }
            | terminal_psi::OperationKind::CallUnit { .. }
            | terminal_psi::OperationKind::CallStructuralScalar { .. }
            | terminal_psi::OperationKind::CallDynamicScalar { .. }
            | terminal_psi::OperationKind::CallDynamicParameterScalar { .. }
            | terminal_psi::OperationKind::CallStructural { .. }
            | terminal_psi::OperationKind::BoundaryCall { .. }
    ) {
        return Err(
            FloatMeaningProjectionVerificationError::DirectOperationResultCallProducer {
                owner: result.owner,
                producer: result.producer,
            },
        );
    }
    let terminal_psi::OperationResult::Scalar(declaration) = producer.result else {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectOperationResult {
                owner: result.owner,
                producer: result.producer,
                result: result.result,
            },
        );
    };
    if declaration.id != result.result {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectOperationResult {
                owner: result.owner,
                producer: result.producer,
                result: result.result,
            },
        );
    }
    if declaration.scalar_type != ScalarType::IeeeFloat(result.format) {
        return Err(FloatMeaningProjectionVerificationError::DirectOperationResultFormatMismatch);
    }
    Ok(())
}

/// Rejoin one artifact-relative source to the exact scalar result declared by
/// one call operation in its owning Terminal machine.
pub(crate) fn verify_direct_call_float_result(
    module: &TerminalModule,
    result: terminal_psi::DirectCallFloatResult,
) -> Result<(), FloatMeaningProjectionVerificationError> {
    let mut owners = module
        .machines
        .iter()
        .filter(|machine| machine.id == result.owner);
    let owner = owners.next().ok_or(
        FloatMeaningProjectionVerificationError::InvalidDirectCallResultOwner(result.owner),
    )?;
    if owners.next().is_some() {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectCallResultOwner(result.owner),
        );
    }
    let mut producers = owner
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == result.producer);
    let producer = producers.next().ok_or(
        FloatMeaningProjectionVerificationError::InvalidDirectCallResultProducer {
            owner: result.owner,
            producer: result.producer,
        },
    )?;
    if producers.next().is_some() {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectCallResultProducer {
                owner: result.owner,
                producer: result.producer,
            },
        );
    }
    if !matches!(
        producer.kind,
        terminal_psi::OperationKind::Call { .. }
            | terminal_psi::OperationKind::CallStructuralScalar { .. }
            | terminal_psi::OperationKind::CallDynamicScalar { .. }
            | terminal_psi::OperationKind::CallDynamicParameterScalar { .. }
            | terminal_psi::OperationKind::BoundaryCall { .. }
    ) {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectCallResultProducerKind {
                owner: result.owner,
                producer: result.producer,
            },
        );
    }
    let terminal_psi::OperationResult::Scalar(declaration) = producer.result else {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectCallResult {
                owner: result.owner,
                producer: result.producer,
                result: result.result,
            },
        );
    };
    if declaration.id != result.result {
        return Err(
            FloatMeaningProjectionVerificationError::InvalidDirectCallResult {
                owner: result.owner,
                producer: result.producer,
                result: result.result,
            },
        );
    }
    if declaration.scalar_type != ScalarType::IeeeFloat(result.format) {
        return Err(FloatMeaningProjectionVerificationError::DirectCallResultFormatMismatch);
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
    InvalidDirectBlockParameterOwner(MachineId),
    InvalidDirectBlockParameterBlock {
        owner: MachineId,
        block: BlockId,
    },
    InvalidDirectBlockParameter {
        owner: MachineId,
        block: BlockId,
        parameter: ValueId,
    },
    DirectBlockParameterFormatMismatch,
    InvalidDirectOperationResultOwner(MachineId),
    InvalidDirectOperationResultProducer {
        owner: MachineId,
        producer: semantic_vocabulary::OperationId,
    },
    InvalidDirectOperationResult {
        owner: MachineId,
        producer: semantic_vocabulary::OperationId,
        result: ValueId,
    },
    DirectOperationResultCallProducer {
        owner: MachineId,
        producer: semantic_vocabulary::OperationId,
    },
    DirectOperationResultFormatMismatch,
    InvalidDirectCallResultOwner(MachineId),
    InvalidDirectCallResultProducer {
        owner: MachineId,
        producer: semantic_vocabulary::OperationId,
    },
    InvalidDirectCallResult {
        owner: MachineId,
        producer: semantic_vocabulary::OperationId,
        result: ValueId,
    },
    InvalidDirectCallResultProducerKind {
        owner: MachineId,
        producer: semantic_vocabulary::OperationId,
    },
    DirectCallResultFormatMismatch,
    InvalidDirectStructuralLeafOwner(MachineId),
    InvalidDirectStructuralLeaf {
        owner: MachineId,
    },
    DirectStructuralLeafWriteOnlyRoot {
        owner: MachineId,
    },
    DirectStructuralLeafFormatMismatch,
    SemanticApplicationContractMismatch,
    SemanticApplicationResultKindMismatch,
    SemanticApplicationOperandCountMismatch,
    SemanticApplicationOperandKindMismatch {
        operand: u32,
    },
    SemanticApplicationOperandRow {
        operand: u32,
    },
    SemanticApplicationFormatMismatch,
    SemanticApplicationDischargeMismatch,
    EqualityCarrierMismatch,
}

#[cfg(test)]
mod tests {
    use super::{
        FloatMeaning, FloatMeaningProjection, FloatMeaningProjectionOperation,
        FloatMeaningProjectionVerificationError, FloatMeaningSource, FloatProjectionOperation,
        IeeeFloatFormat, ProofOnlyValueType, reconstruct_float_meaning_projection,
        terminal_contract_identity,
    };
    use terminal_psi::{
        FloatProjectionInput, FloatProjectionInputId, ProofValueDeclaration, ProofValueId,
    };

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
        let reconstructed = reconstruct_float_meaning_projection(&[], &projection()).unwrap();
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
            reconstruct_float_meaning_projection(&[], &tampered),
            Err(FloatMeaningProjectionVerificationError::ContractIdentityMismatch)
        );

        tampered = projection();
        tampered.source = FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(6),
            format: IeeeFloatFormat::Binary64,
        });
        assert_eq!(
            reconstruct_float_meaning_projection(&[], &tampered),
            Err(FloatMeaningProjectionVerificationError::SourceFormatMismatch)
        );
    }

    #[test]
    fn verifier_reconstructs_literal_bits_and_payload_erased_meaning() {
        let mut exact = projection();
        exact.source = FloatMeaningSource::ExactBinary32Literal(0x8000_0000);
        let reconstructed = reconstruct_float_meaning_projection(&[], &exact).unwrap();
        assert_eq!(
            reconstructed.source,
            FloatMeaningSource::ExactBinary32Literal(0x8000_0000)
        );
        assert_eq!(
            reconstructed.literal_meaning,
            Some(FloatMeaning::Zero { negative: true })
        );

        exact.source = FloatMeaningSource::ExactBinary32Literal(0x7fc0_0001);
        let first_nan = reconstruct_float_meaning_projection(&[], &exact).unwrap();
        exact.source = FloatMeaningSource::ExactBinary32Literal(0x7fff_ffff);
        let second_nan = reconstruct_float_meaning_projection(&[], &exact).unwrap();
        assert_ne!(first_nan.source, second_nan.source);
        assert_eq!(first_nan.literal_meaning, Some(FloatMeaning::NaN));
        assert_eq!(first_nan.literal_meaning, second_nan.literal_meaning);
    }
}
