//! Exact erasure of checked float-meaning projections into Terminal Psi.

use psi_checked_trees::{
    CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
    CheckedFloatMeaningProjectionError, CheckedFloatProjectionSource, CheckedProofOnlyValueType,
    CheckedTrees, types::PrimitiveType,
};
use psi_core::{IeeeFloatFormat, MachineId, ScalarType};
use psi_terminal::{
    DirectMachineFloatParameter, DirectMachineFloatResult, FloatMeaningEqualityProposition,
    FloatMeaningProjection, FloatMeaningProjectionOperation, FloatMeaningSource,
    FloatProjectionContractIdentity, FloatProjectionInput, FloatProjectionInputId,
    ProofOnlyValueType, ProofPropositionId, ProofValueDeclaration, ProofValueId, TerminalMachine,
    TerminalMachineResult,
};

use crate::{LoweringError, terminal_scalar_type};

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
    direct_source: Option<FloatMeaningSource>,
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
        CheckedFloatProjectionSource::DirectMachineParameter(parameter) => {
            let format = match parameter.fallback.primitive {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
            };
            match direct_source {
                Some(FloatMeaningSource::DirectMachineParameter(parameter))
                    if parameter.format == format =>
                {
                    FloatMeaningSource::DirectMachineParameter(parameter)
                }
                Some(_) => {
                    return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier);
                }
                None => FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                    id: FloatProjectionInputId(parameter.fallback.id.0),
                    format,
                }),
            }
        }
        CheckedFloatProjectionSource::DirectMachineResult(result) => {
            let format = match result.fallback.primitive {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
            };
            match direct_source {
                Some(FloatMeaningSource::DirectMachineResult(result))
                    if result.format == format =>
                {
                    FloatMeaningSource::DirectMachineResult(result)
                }
                Some(_) => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
                None => FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                    id: FloatProjectionInputId(result.fallback.id.0),
                    format,
                }),
            }
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
        contract: FloatProjectionContractIdentity {
            format: checked.contract.format,
            operation: checked.contract.operation,
            declaration: checked.contract.declaration,
            catalog_version: checked.contract.catalog_version,
            commitment: checked.contract.commitment,
        },
    })
}

/// Rejoin checked source symbols to exact Terminal semantic identities while
/// both representations are available. An owner outside the emitted artifact,
/// or a route whose scalar signature does not exactly preserve the source
/// shape, retains the checked transitional fallback.
pub(crate) fn resolve_direct_float_source_binding(
    checked: &CheckedTrees,
    machine_bindings: &[(psi_symbols::SymbolHandle, MachineId)],
    terminal_machines: &[TerminalMachine],
    projection: CheckedFloatMeaningProjection,
) -> Result<Option<FloatMeaningSource>, LoweringError> {
    let owner_machine = match projection.source {
        CheckedFloatProjectionSource::DirectMachineParameter(parameter) => parameter.owner_machine,
        CheckedFloatProjectionSource::DirectMachineResult(result) => result.owner_machine,
        _ => return Ok(None),
    };
    let Some((_, terminal_owner)) = machine_bindings
        .iter()
        .find(|(source_owner, _)| *source_owner == owner_machine)
    else {
        return Ok(None);
    };
    let invalid_source = || {
        LoweringError::InvalidFloatMeaningProjection(
            FloatMeaningProjectionLoweringError::InvalidSourceCarrier,
        )
    };
    let mut terminal_owners = terminal_machines
        .iter()
        .filter(|machine| machine.id == *terminal_owner);
    let terminal_machine = terminal_owners.next().ok_or_else(invalid_source)?;
    if terminal_owners.next().is_some() {
        return Err(invalid_source());
    }
    let source_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == owner_machine)
        .ok_or_else(invalid_source)?;
    let source_entry = checked
        .typed
        .machine_states(source_machine)
        .first()
        .ok_or_else(invalid_source)?;
    match projection.source {
        CheckedFloatProjectionSource::DirectMachineParameter(parameter) => {
            resolve_direct_float_parameter(
                checked,
                source_entry,
                terminal_machine,
                *terminal_owner,
                parameter,
            )
        }
        CheckedFloatProjectionSource::DirectMachineResult(result) => {
            let format = match result.fallback.primitive {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return Err(invalid_source()),
            };
            if checked
                .typed
                .primitive_type_reference(source_entry.return_type)
                != Some(result.fallback.primitive)
            {
                return Err(invalid_source());
            }
            let TerminalMachineResult::Scalar(terminal_result) = terminal_machine.result else {
                return Err(invalid_source());
            };
            if terminal_result.scalar_type != ScalarType::IeeeFloat(format) {
                return Err(invalid_source());
            }
            Ok(Some(FloatMeaningSource::DirectMachineResult(
                DirectMachineFloatResult {
                    owner: *terminal_owner,
                    result: terminal_result.id,
                    format,
                },
            )))
        }
        _ => Ok(None),
    }
}

fn resolve_direct_float_parameter(
    checked: &CheckedTrees,
    source_entry: &psi_checked_trees::state::State,
    terminal_machine: &TerminalMachine,
    terminal_owner: MachineId,
    parameter: psi_checked_trees::CheckedDirectMachineFloatParameter,
) -> Result<Option<FloatMeaningSource>, LoweringError> {
    let invalid_source = || {
        LoweringError::InvalidFloatMeaningProjection(
            FloatMeaningProjectionLoweringError::InvalidSourceCarrier,
        )
    };
    let source_parameters = checked
        .typed
        .state_parameters(source_entry)
        .iter()
        .filter(|parameter| !parameter.is_self && !parameter.is_const)
        .filter_map(|parameter| {
            checked
                .typed
                .primitive_type_reference(parameter.type_reference)
                .map(|primitive| (parameter.symbol, primitive))
        })
        .collect::<Vec<_>>();
    let source_types = source_parameters
        .iter()
        .map(|(_, primitive)| terminal_scalar_type(*primitive))
        .collect::<Result<Vec<_>, _>>()?;
    if source_types
        != terminal_machine
            .parameters
            .iter()
            .map(|parameter| parameter.scalar_type)
            .collect::<Vec<_>>()
    {
        return Ok(None);
    }
    let position = source_parameters
        .iter()
        .position(|(symbol, _)| *symbol == parameter.parameter)
        .ok_or_else(invalid_source)?;
    let terminal_parameter = terminal_machine
        .parameters
        .get(position)
        .ok_or_else(invalid_source)?;
    let format = match parameter.fallback.primitive {
        PrimitiveType::F32 => IeeeFloatFormat::Binary32,
        PrimitiveType::F64 => IeeeFloatFormat::Binary64,
        _ => return Err(invalid_source()),
    };
    if terminal_parameter.scalar_type != ScalarType::IeeeFloat(format) {
        return Err(invalid_source());
    }
    Ok(Some(FloatMeaningSource::DirectMachineParameter(
        DirectMachineFloatParameter {
            owner: terminal_owner,
            parameter: terminal_parameter.id,
            format,
        },
    )))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatMeaningProjectionLoweringError {
    InvalidCheckedProjection(CheckedFloatMeaningProjectionError),
    InvalidSourceCarrier,
}

#[cfg(test)]
mod tests {
    use psi_checked_trees::{
        CheckedDirectMachineFloatParameter, CheckedDirectMachineFloatResult,
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
            contract: FloatProjectionOperation::Meaning64.contract_identity(),
        }
    }
    #[test]
    fn lowering_preserves_dense_identities_and_exact_format() {
        let lowered = lower_float_meaning_projection(checked_projection(), None).unwrap();
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
        let expected = FloatProjectionOperation::Meaning64.contract_identity();
        assert_eq!(lowered.contract.format, expected.format);
        assert_eq!(lowered.contract.operation, expected.operation);
        assert_eq!(lowered.contract.declaration, expected.declaration);
        assert_eq!(lowered.contract.catalog_version, expected.catalog_version);
        assert_eq!(lowered.contract.commitment, expected.commitment);
    }

    #[test]
    fn direct_machine_parameter_lowers_to_an_exact_terminal_binding_when_available() {
        let mut checked = checked_projection();
        checked.source = CheckedFloatProjectionSource::DirectMachineParameter(
            CheckedDirectMachineFloatParameter {
                owner_machine: psi_symbols::SymbolHandle::from_arena_index(3),
                parameter: psi_symbols::SymbolHandle::from_arena_index(5),
                fallback: CheckedFloatProjectionInput {
                    id: CheckedFloatProjectionInputId(9),
                    primitive: PrimitiveType::F64,
                },
            },
        );
        let direct = DirectMachineFloatParameter {
            owner: psi_core::MachineId::new(4).unwrap(),
            parameter: psi_core::ValueId::new(6).unwrap(),
            format: IeeeFloatFormat::Binary64,
        };
        let lowered = lower_float_meaning_projection(
            checked,
            Some(FloatMeaningSource::DirectMachineParameter(direct)),
        )
        .unwrap();
        assert_eq!(
            lowered.source,
            FloatMeaningSource::DirectMachineParameter(direct)
        );
    }

    #[test]
    fn direct_machine_parameter_retains_fallback_without_an_artifact_binding() {
        let mut checked = checked_projection();
        checked.source = CheckedFloatProjectionSource::DirectMachineParameter(
            CheckedDirectMachineFloatParameter {
                owner_machine: psi_symbols::SymbolHandle::from_arena_index(3),
                parameter: psi_symbols::SymbolHandle::from_arena_index(5),
                fallback: CheckedFloatProjectionInput {
                    id: CheckedFloatProjectionInputId(9),
                    primitive: PrimitiveType::F64,
                },
            },
        );
        let lowered = lower_float_meaning_projection(checked, None).unwrap();
        assert_eq!(
            lowered.source,
            FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                id: FloatProjectionInputId(9),
                format: IeeeFloatFormat::Binary64,
            })
        );
    }

    #[test]
    fn direct_machine_result_lowers_to_exact_terminal_binding_or_fallback() {
        let mut checked = checked_projection();
        checked.source =
            CheckedFloatProjectionSource::DirectMachineResult(CheckedDirectMachineFloatResult {
                owner_machine: psi_symbols::SymbolHandle::from_arena_index(3),
                fallback: CheckedFloatProjectionInput {
                    id: CheckedFloatProjectionInputId(9),
                    primitive: PrimitiveType::F64,
                },
            });
        let direct = DirectMachineFloatResult {
            owner: psi_core::MachineId::new(4).unwrap(),
            result: psi_core::ValueId::new(8).unwrap(),
            format: IeeeFloatFormat::Binary64,
        };
        let lowered = lower_float_meaning_projection(
            checked,
            Some(FloatMeaningSource::DirectMachineResult(direct)),
        )
        .unwrap();
        assert_eq!(
            lowered.source,
            FloatMeaningSource::DirectMachineResult(direct)
        );

        assert_eq!(
            lower_float_meaning_projection(checked, None)
                .unwrap()
                .source,
            FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                id: FloatProjectionInputId(9),
                format: IeeeFloatFormat::Binary64,
            })
        );
    }

    #[test]
    fn lowering_rejects_cross_format_checked_operation() {
        let mut checked = checked_projection();
        checked.operation = FloatProjectionOperation::Meaning32;
        assert_eq!(
            lower_float_meaning_projection(checked, None),
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
        let lowered = lower_float_meaning_projection(checked, None).unwrap();
        assert_eq!(
            lowered.source,
            FloatMeaningSource::ExactBinary64Literal(0x8000_0000_0000_0000)
        );
    }
}
