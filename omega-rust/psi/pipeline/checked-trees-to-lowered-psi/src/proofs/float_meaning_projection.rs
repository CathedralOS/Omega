//! Exact erasure of checked float-meaning projections into Terminal Psi.

use checked_trees::{
    CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
    CheckedFloatMeaningProjectionError, CheckedFloatProjectionSource, CheckedProofOnlyValueType,
    CheckedTrees, types::PrimitiveType,
};
use semantic_vocabulary::{BlockId, IeeeFloatFormat, MachineId, ScalarType};
use terminal_psi::{
    DirectBlockFloatParameter, DirectCallFloatResult, DirectMachineFloatParameter,
    DirectMachineFloatResult, DirectOperationFloatResult, DirectStructuralFloatLeaf,
    FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
    FloatMeaningSource, FloatProjectionContractIdentity, FloatProjectionInput,
    FloatProjectionInputId, ProofOnlyValueType, ProofPropositionId, ProofValueDeclaration,
    ProofValueId, TerminalMachine, TerminalMachineResult,
};

use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::LoweringError;

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
        CheckedFloatProjectionSource::DirectBlockParameter(parameter) => {
            let format = match parameter.fallback.primitive {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
            };
            match direct_source {
                Some(FloatMeaningSource::DirectBlockParameter(parameter))
                    if parameter.format == format =>
                {
                    FloatMeaningSource::DirectBlockParameter(parameter)
                }
                Some(_) => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
                None => FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                    id: FloatProjectionInputId(parameter.fallback.id.0),
                    format,
                }),
            }
        }
        CheckedFloatProjectionSource::DirectStructuralLeaf(leaf) => {
            let format = match leaf.fallback.primitive {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
            };
            match direct_source {
                Some(FloatMeaningSource::DirectStructuralLeaf(direct))
                    if direct.format == format =>
                {
                    FloatMeaningSource::DirectStructuralLeaf(direct)
                }
                Some(_) => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
                None => FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                    id: FloatProjectionInputId(leaf.fallback.id.0),
                    format,
                }),
            }
        }
        CheckedFloatProjectionSource::DirectCallResult(result) => {
            let format = match result.fallback.primitive {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
            };
            match direct_source {
                Some(FloatMeaningSource::DirectCallResult(direct)) if direct.format == format => {
                    FloatMeaningSource::DirectCallResult(direct)
                }
                Some(_) => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
                None => FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                    id: FloatProjectionInputId(result.fallback.id.0),
                    format,
                }),
            }
        }
        CheckedFloatProjectionSource::DirectOperationResult(result) => {
            let format = match result.fallback.primitive {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
            };
            match direct_source {
                Some(FloatMeaningSource::DirectOperationResult(direct))
                    if direct.format == format =>
                {
                    FloatMeaningSource::DirectOperationResult(direct)
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
        numerics::float_projection::FloatProjectionOperation::Meaning32 => {
            FloatMeaningProjectionOperation::Meaning32
        }
        numerics::float_projection::FloatProjectionOperation::Meaning64 => {
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
    machine_bindings: &[(symbols::SymbolHandle, MachineId)],
    terminal_machines: &[TerminalMachine],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    source_call_occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
    fma_occurrences: &[lowered_psi::LoweredSelectedIeeeFloatFmaOccurrence],
    projection: CheckedFloatMeaningProjection,
) -> Result<Option<FloatMeaningSource>, LoweringError> {
    let owner_machine = match &projection.source {
        CheckedFloatProjectionSource::DirectMachineParameter(parameter) => parameter.owner_machine,
        CheckedFloatProjectionSource::DirectMachineResult(result) => result.owner_machine,
        CheckedFloatProjectionSource::DirectBlockParameter(parameter) => parameter.owner_machine,
        CheckedFloatProjectionSource::DirectStructuralLeaf(leaf) => leaf.owner_machine,
        CheckedFloatProjectionSource::DirectCallResult(result) => result.use_site.owner_machine,
        CheckedFloatProjectionSource::DirectOperationResult(result) => {
            result.use_site.owner_machine
        }
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
        CheckedFloatProjectionSource::DirectBlockParameter(parameter) => {
            resolve_direct_block_float_parameter(
                checked,
                source_machine,
                terminal_machine,
                *terminal_owner,
                parameter,
            )
        }
        CheckedFloatProjectionSource::DirectCallResult(result) => {
            let format = match result.fallback.primitive {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return Err(invalid_source()),
            };
            let mut occurrences = source_call_occurrences.iter().filter(|occurrence| {
                occurrence.source_state == result.use_site.owner_state
                    && occurrence.statement_index == result.use_site.statement_index
                    && occurrence.call_ordinal == result.use_site.call_ordinal
            });
            let Some(occurrence) = occurrences.next() else {
                return Ok(None);
            };
            if occurrences.next().is_some() {
                return Err(invalid_source());
            }
            resolve_float_operation_result_source(
                terminal_machine,
                *terminal_owner,
                occurrence.terminal_operation,
                format,
                true,
            )
        }
        CheckedFloatProjectionSource::DirectOperationResult(result) => {
            let format = match result.fallback.primitive {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return Err(invalid_source()),
            };
            let mut occurrences = fma_occurrences.iter().filter(|occurrence| {
                occurrence.source_state == result.use_site.owner_state
                    && occurrence.statement_index == result.use_site.statement_index
                    && occurrence.call_ordinal == result.use_site.call_ordinal
            });
            let Some(occurrence) = occurrences.next() else {
                return Ok(None);
            };
            if occurrences.next().is_some() {
                return Err(invalid_source());
            }
            if occurrence.format != format {
                return Err(invalid_source());
            }
            resolve_float_operation_result_source(
                terminal_machine,
                *terminal_owner,
                occurrence.terminal_operation,
                format,
                false,
            )
        }
        CheckedFloatProjectionSource::DirectStructuralLeaf(leaf) => {
            let format = match leaf.fallback.primitive {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return Err(invalid_source()),
            };
            let parameter = terminal_machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.position == leaf.field.parameter_position)
                .ok_or_else(invalid_source)?;
            if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return Err(invalid_source());
            }
            let (root, path, actual) = crate::proofs::crash_routes::lower_structural_member_path(
                leaf.field.parameter_position,
                &leaf.field.path,
                &terminal_machine.structural_parameters,
                structural_types,
            )?;
            if actual != terminal_psi::StructuralFieldType::IeeeFloat(format) {
                return Err(invalid_source());
            }
            Ok(Some(FloatMeaningSource::DirectStructuralLeaf(
                DirectStructuralFloatLeaf {
                    owner: *terminal_owner,
                    field: semantic_vocabulary::IeeeFloatStructuralField::new(root, path)
                        .map_err(LoweringError::InvalidCrashPredicate)?,
                    format,
                },
            )))
        }
        _ => Ok(None),
    }
}

fn resolve_direct_float_parameter(
    checked: &CheckedTrees,
    source_entry: &checked_trees::state::State,
    terminal_machine: &TerminalMachine,
    terminal_owner: MachineId,
    parameter: checked_trees::CheckedDirectMachineFloatParameter,
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

/// Rejoin a checked nested-state parameter to the exact Terminal block and
/// scalar parameter identity. Source-state blocks occupy the machine's dense
/// block namespace before every synthesized block, so the lowest block
/// identity is the checked graph entry and each state's block follows it by
/// position. The entry state itself is refused: its parameters already carry
/// the machine-parameter class, never block parameters.
fn resolve_direct_block_float_parameter(
    checked: &CheckedTrees,
    source_machine: &checked_trees::machine::Machine,
    terminal_machine: &TerminalMachine,
    terminal_owner: MachineId,
    parameter: checked_trees::CheckedDirectBlockFloatParameter,
) -> Result<Option<FloatMeaningSource>, LoweringError> {
    let invalid_source = || {
        LoweringError::InvalidFloatMeaningProjection(
            FloatMeaningProjectionLoweringError::InvalidSourceCarrier,
        )
    };
    let format = match parameter.fallback.primitive {
        PrimitiveType::F32 => IeeeFloatFormat::Binary32,
        PrimitiveType::F64 => IeeeFloatFormat::Binary64,
        _ => return Err(invalid_source()),
    };
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(source_machine.symbol)
        .ok_or_else(invalid_source)?;
    let position = graph
        .states
        .iter()
        .position(|state| state.state == parameter.owner_state)
        .ok_or_else(invalid_source)?;
    if position == 0
        || graph
            .states
            .iter()
            .skip(position + 1)
            .any(|state| state.state == parameter.owner_state)
    {
        return Err(invalid_source());
    }
    let graph_entry = terminal_machine
        .blocks
        .iter()
        .map(|block| block.id)
        .min()
        .ok_or_else(invalid_source)?;
    let expected = graph_entry
        .get()
        .checked_add(u64::try_from(position).map_err(|_| invalid_source())?)
        .and_then(BlockId::new)
        .ok_or_else(invalid_source)?;
    let mut blocks = terminal_machine
        .blocks
        .iter()
        .filter(|block| block.id == expected);
    let block = blocks.next().ok_or_else(invalid_source)?;
    if blocks.next().is_some() {
        return Err(invalid_source());
    }
    let graph_state = &graph.states[position];
    if block.parameters.len() != graph_state.scalar_parameters.len() {
        return Err(invalid_source());
    }
    let source_state = checked
        .typed
        .machine_states(source_machine)
        .iter()
        .find(|state| state.symbol == parameter.owner_state)
        .ok_or_else(invalid_source)?;
    let authored_position = checked
        .typed
        .state_parameters(source_state)
        .iter()
        .position(|source| source.symbol == parameter.parameter)
        .ok_or_else(invalid_source)?;
    let scalar_index = graph_state
        .scalar_parameters
        .iter()
        .position(|scalar| usize::try_from(scalar.source_position).ok() == Some(authored_position))
        .ok_or_else(invalid_source)?;
    if graph_state.scalar_parameters[scalar_index].primitive_type != parameter.fallback.primitive {
        return Err(invalid_source());
    }
    let declaration = &block.parameters[scalar_index];
    if declaration.scalar_type != ScalarType::IeeeFloat(format) {
        return Err(invalid_source());
    }
    Ok(Some(FloatMeaningSource::DirectBlockParameter(
        DirectBlockFloatParameter {
            owner: terminal_owner,
            block: block.id,
            parameter: declaration.id,
            format,
        },
    )))
}

/// Rejoin one checked use-site occurrence to the exact scalar result its
/// emitted Terminal operation declares. `call` selects the verifier's
/// producer-kind partition: call uses must join a call-class operation and
/// operation uses must join a non-call producer.
fn resolve_float_operation_result_source(
    terminal_machine: &TerminalMachine,
    terminal_owner: MachineId,
    terminal_operation: semantic_vocabulary::OperationId,
    format: IeeeFloatFormat,
    call: bool,
) -> Result<Option<FloatMeaningSource>, LoweringError> {
    let invalid_source = || {
        LoweringError::InvalidFloatMeaningProjection(
            FloatMeaningProjectionLoweringError::InvalidSourceCarrier,
        )
    };
    let mut producers = terminal_machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == terminal_operation);
    let producer = producers.next().ok_or_else(invalid_source)?;
    if producers.next().is_some() {
        return Err(invalid_source());
    }
    let call_kind = matches!(
        producer.kind,
        terminal_psi::OperationKind::Call { .. }
            | terminal_psi::OperationKind::CallStructuralScalar { .. }
            | terminal_psi::OperationKind::CallDynamicScalar { .. }
            | terminal_psi::OperationKind::CallDynamicParameterScalar { .. }
            | terminal_psi::OperationKind::BoundaryCall { .. }
    );
    if call_kind != call {
        return Err(invalid_source());
    }
    let terminal_psi::OperationResult::Scalar(declaration) = producer.result else {
        return Err(invalid_source());
    };
    if declaration.scalar_type != ScalarType::IeeeFloat(format) {
        return Err(invalid_source());
    }
    if call {
        return Ok(Some(FloatMeaningSource::DirectCallResult(
            DirectCallFloatResult {
                owner: terminal_owner,
                producer: terminal_operation,
                result: declaration.id,
                format,
            },
        )));
    }
    if !matches!(
        producer.kind,
        terminal_psi::OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
    ) {
        return Err(invalid_source());
    }
    Ok(Some(FloatMeaningSource::DirectOperationResult(
        DirectOperationFloatResult {
            owner: terminal_owner,
            producer: terminal_operation,
            result: declaration.id,
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
mod tests;
