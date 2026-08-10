#![forbid(unsafe_code)]

//! Lower verified terminal Psi into source-independent Omega realization
//! requirements.

use std::collections::BTreeMap;

use omega_terminal_abstract_operations::{
    TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractOperation,
    TerminalAbstractOperationPlan, TerminalAbstractParameter, TerminalAbstractResult,
    TerminalAbstractSuccessor, TerminalValueBinding,
};
use psi_core::{BlockId, MachineId, ScalarType};
use psi_terminal::{OperationKind, TerminalMachine, Terminator};
use psi_terminal_codec::{CodecError, terminal_psi_identity};
use psi_terminal_verifier::VerifiedTerminalModule;

/// Canonical-decode and verify terminal-Psi semantic/proof artifact sections
/// before constructing Omega's source-independent realization requirements.
/// Producer-owned modules and frontend trees cannot cross this boundary.
pub fn lower_artifact_sections(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_kernel::AdmissionProfile,
) -> Result<TerminalAbstractOperationPlan, ArtifactLoweringError> {
    let module = psi_terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let proof = psi_terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    let verified = psi_terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    lower_verified_module_inner(&verified).map_err(ArtifactLoweringError::Lowering)
}

/// In-memory bypass for integration tests that exercise individual verified
/// stages. Default production builds expose only [`lower_artifact_sections`].
#[cfg(feature = "in-memory-test-support")]
pub fn lower_verified_module(
    verified: &VerifiedTerminalModule<'_>,
) -> Result<TerminalAbstractOperationPlan, LoweringError> {
    lower_verified_module_inner(verified)
}

/// Consume the complete verified module after the artifact entry has decoded
/// and verified it. The initial terminal vocabulary has one unconditional
/// executable chain per machine, so its Omega requirement stream is flat and
/// ordered.
fn lower_verified_module_inner(
    verified: &VerifiedTerminalModule<'_>,
) -> Result<TerminalAbstractOperationPlan, LoweringError> {
    let module = verified.module();
    if !module
        .machines
        .iter()
        .any(|machine| machine.id == module.entry)
    {
        return Err(LoweringError::VerifiedEntryMachineMissing(module.entry));
    }
    let functions = module
        .machines
        .iter()
        .map(lower_machine)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TerminalAbstractOperationPlan {
        terminal_psi: terminal_psi_identity(module).map_err(LoweringError::SemanticIdentity)?,
        entry: module.entry,
        functions,
    })
}

fn lower_machine(machine: &TerminalMachine) -> Result<TerminalAbstractFunction, LoweringError> {
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut operations = Vec::new();
    let mut block_entries = Vec::with_capacity(machine.blocks.len());
    let value_types = machine
        .parameters
        .iter()
        .chain(std::iter::once(&machine.result))
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.operations.iter().map(|operation| &operation.result)),
        )
        .map(|value| (value.id, value.scalar_type))
        .collect::<BTreeMap<_, _>>();

    for block in &machine.blocks {
        block_entries.push(TerminalAbstractBlockEntry {
            block: block.id,
            operation_offset: operations.len(),
        });
        for operation in &block.operations {
            match operation.kind {
                OperationKind::IntegerConstant { value } => {
                    operations.push(TerminalAbstractOperation::IntegerConstant {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type: operation.result.scalar_type,
                        value,
                    });
                }
                OperationKind::BooleanConstant { value } => {
                    operations.push(TerminalAbstractOperation::BooleanConstant {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        value,
                    });
                }
                OperationKind::BooleanNot { operand } => {
                    operations.push(TerminalAbstractOperation::BooleanNot {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        operand,
                    });
                }
                OperationKind::BooleanEqual { left, right } => {
                    operations.push(TerminalAbstractOperation::BooleanEqual {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        left,
                        right,
                    });
                }
                OperationKind::IntegerEqual { left, right } => {
                    operations.push(TerminalAbstractOperation::IntegerEqual {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        left,
                        right,
                    });
                }
                OperationKind::IntegerLessThan { left, right } => {
                    operations.push(TerminalAbstractOperation::IntegerLessThan {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        left,
                        right,
                    });
                }
                OperationKind::IntegerLessOrEqual { left, right } => {
                    operations.push(TerminalAbstractOperation::IntegerLessOrEqual {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        left,
                        right,
                    });
                }
                OperationKind::IntegerBitwiseNot { operand } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedIntegerBitwiseMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::IntegerBitwiseNot {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        operand,
                    });
                }
                OperationKind::IntegerWiden { operand } => {
                    let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied()
                    else {
                        return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
                    };
                    let ScalarType::Integer(target_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::IntegerWiden {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        source_type,
                        target_type,
                        operand,
                    });
                }
                OperationKind::IntegerExactCast { operand, .. } => {
                    let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied()
                    else {
                        return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                            operation.id,
                        ));
                    };
                    let ScalarType::Integer(target_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::IntegerExactCast {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        source_type,
                        target_type,
                        operand,
                    });
                }
                OperationKind::IntegerBitwiseAnd { left, right }
                | OperationKind::IntegerBitwiseOr { left, right }
                | OperationKind::IntegerBitwiseXor { left, right } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedIntegerBitwiseMalformed(operation.id));
                    };
                    operations.push(match operation.kind {
                        OperationKind::IntegerBitwiseAnd { .. } => {
                            TerminalAbstractOperation::IntegerBitwiseAnd {
                                psi_operation: operation.id,
                                result: operation.result.id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        OperationKind::IntegerBitwiseOr { .. } => {
                            TerminalAbstractOperation::IntegerBitwiseOr {
                                psi_operation: operation.id,
                                result: operation.result.id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        OperationKind::IntegerBitwiseXor { .. } => {
                            TerminalAbstractOperation::IntegerBitwiseXor {
                                psi_operation: operation.id,
                                result: operation.result.id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        _ => unreachable!(),
                    });
                }
                OperationKind::WrappingIntegerShiftLeft { value, count }
                | OperationKind::WrappingIntegerShiftRight { value, count } => {
                    let ScalarType::Integer(value_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedWrappingShiftMalformed(operation.id));
                    };
                    let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied()
                    else {
                        return Err(LoweringError::VerifiedWrappingShiftMalformed(operation.id));
                    };
                    operations.push(match operation.kind {
                        OperationKind::WrappingIntegerShiftLeft { .. } => {
                            TerminalAbstractOperation::WrappingIntegerShiftLeft {
                                psi_operation: operation.id,
                                result: operation.result.id,
                                value_type,
                                count_type,
                                value,
                                count,
                            }
                        }
                        OperationKind::WrappingIntegerShiftRight { .. } => {
                            TerminalAbstractOperation::WrappingIntegerShiftRight {
                                psi_operation: operation.id,
                                result: operation.result.id,
                                value_type,
                                count_type,
                                value,
                                count,
                            }
                        }
                        _ => unreachable!(),
                    });
                }
                OperationKind::ExactIntegerShiftRight { value, count, .. } => {
                    let ScalarType::Integer(value_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
                    };
                    let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied()
                    else {
                        return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::ExactIntegerShiftRight {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        value_type,
                        count_type,
                        value,
                        count,
                    });
                }
                OperationKind::ExactIntegerShiftLeft { value, count, .. } => {
                    let ScalarType::Integer(value_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
                    };
                    let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied()
                    else {
                        return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::ExactIntegerShiftLeft {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        value_type,
                        count_type,
                        value,
                        count,
                    });
                }
                OperationKind::ExactIntegerAdd { left, right, .. }
                | OperationKind::WrappingIntegerAdd { left, right } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedWrappingAddMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::WrappingIntegerAdd {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::ExactIntegerSubtract { left, right, .. }
                | OperationKind::WrappingIntegerSubtract { left, right } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedWrappingSubtractMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::WrappingIntegerSubtract {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::ExactIntegerMultiply { left, right, .. }
                | OperationKind::WrappingIntegerMultiply { left, right } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedWrappingMultiplyMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::WrappingIntegerMultiply {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::ExactIntegerDivide { left, right, .. } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedExactDivideMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::ExactIntegerDivide {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::ExactIntegerRemainder { left, right, .. } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedExactRemainderMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::ExactIntegerRemainder {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::WrappingIntegerDivide { left, right, .. } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedWrappingDivideMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::WrappingIntegerDivide {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::WrappingIntegerRemainder { left, right, .. } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedWrappingRemainderMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::WrappingIntegerRemainder {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::SaturatingIntegerDivide { left, right, .. } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedSaturatingDivideMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::SaturatingIntegerDivide {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::SaturatingIntegerRemainder { left, right, .. } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedSaturatingRemainderMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::SaturatingIntegerRemainder {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::SaturatingIntegerAdd { left, right } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedSaturatingAddMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::SaturatingIntegerAdd {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::SaturatingIntegerSubtract { left, right } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedSaturatingSubtractMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::SaturatingIntegerSubtract {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::SaturatingIntegerMultiply { left, right } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(LoweringError::VerifiedSaturatingMultiplyMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::SaturatingIntegerMultiply {
                        psi_operation: operation.id,
                        result: operation.result.id,
                        scalar_type,
                        left,
                        right,
                    });
                }
            }
        }
        match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                arguments,
            } => {
                let target_block =
                    blocks
                        .get(target)
                        .copied()
                        .ok_or(LoweringError::VerifiedBlockMissing {
                            machine: machine.id,
                            block: *target,
                        })?;
                if target_block.parameters.len() != arguments.len() {
                    return Err(LoweringError::VerifiedJumpArityMismatch { edge: *edge });
                }
                operations.push(TerminalAbstractOperation::Jump {
                    psi_edge: *edge,
                    target: *target,
                    bindings: target_block
                        .parameters
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| TerminalValueBinding {
                            parameter: parameter.id,
                            argument: *argument,
                            scalar_type: parameter.scalar_type,
                        })
                        .collect(),
                });
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let lower_successor = |successor: &psi_terminal::SuccessorEdge| {
                    let target_block = blocks.get(&successor.target).copied().ok_or(
                        LoweringError::VerifiedBlockMissing {
                            machine: machine.id,
                            block: successor.target,
                        },
                    )?;
                    if target_block.parameters.len() != successor.arguments.len() {
                        return Err(LoweringError::VerifiedJumpArityMismatch {
                            edge: successor.edge,
                        });
                    }
                    Ok(TerminalAbstractSuccessor {
                        psi_edge: successor.edge,
                        target: successor.target,
                        bindings: target_block
                            .parameters
                            .iter()
                            .zip(&successor.arguments)
                            .map(|(parameter, argument)| TerminalValueBinding {
                                parameter: parameter.id,
                                argument: *argument,
                                scalar_type: parameter.scalar_type,
                            })
                            .collect(),
                    })
                };
                operations.push(TerminalAbstractOperation::Conditional {
                    condition: *condition,
                    when_true: lower_successor(when_true)?,
                    when_false: lower_successor(when_false)?,
                });
            }
            Terminator::Return { edge, value } => {
                operations.push(TerminalAbstractOperation::Return {
                    psi_edge: *edge,
                    result: machine.result.id,
                    value: *value,
                    scalar_type: machine.result.scalar_type,
                });
            }
            Terminator::Crash {
                edge,
                cause,
                site_guard,
                frontier_lower_bound,
            } => {
                operations.push(TerminalAbstractOperation::Crash {
                    psi_edge: *edge,
                    cause: *cause,
                    site_guard: site_guard.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                });
            }
        }
    }

    Ok(TerminalAbstractFunction {
        machine: machine.id,
        entry: machine.entry,
        parameters: machine
            .parameters
            .iter()
            .map(|parameter| TerminalAbstractParameter {
                value: parameter.id,
                scalar_type: parameter.scalar_type,
            })
            .collect(),
        result: TerminalAbstractResult {
            value: machine.result.id,
            scalar_type: machine.result.scalar_type,
        },
        block_entries,
        operations,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    SemanticIdentity(CodecError),
    VerifiedEntryMachineMissing(MachineId),
    VerifiedBlockMissing { machine: MachineId, block: BlockId },
    VerifiedControlCycle { machine: MachineId, block: BlockId },
    VerifiedJumpArityMismatch { edge: psi_core::EdgeId },
    VerifiedWrappingAddMalformed(psi_core::OperationId),
    VerifiedSaturatingAddMalformed(psi_core::OperationId),
    VerifiedWrappingSubtractMalformed(psi_core::OperationId),
    VerifiedSaturatingSubtractMalformed(psi_core::OperationId),
    VerifiedWrappingMultiplyMalformed(psi_core::OperationId),
    VerifiedExactDivideMalformed(psi_core::OperationId),
    VerifiedExactRemainderMalformed(psi_core::OperationId),
    VerifiedWrappingDivideMalformed(psi_core::OperationId),
    VerifiedWrappingRemainderMalformed(psi_core::OperationId),
    VerifiedSaturatingDivideMalformed(psi_core::OperationId),
    VerifiedSaturatingRemainderMalformed(psi_core::OperationId),
    VerifiedSaturatingMultiplyMalformed(psi_core::OperationId),
    VerifiedIntegerBitwiseMalformed(psi_core::OperationId),
    VerifiedIntegerWidenMalformed(psi_core::OperationId),
    VerifiedIntegerExactCastMalformed(psi_core::OperationId),
    VerifiedWrappingShiftMalformed(psi_core::OperationId),
    VerifiedExactShiftMalformed(psi_core::OperationId),
}

#[derive(Debug)]
pub enum ArtifactLoweringError {
    SemanticDecode(psi_terminal_codec::CodecError),
    ProofDecode(psi_terminal_codec::ProofCodecError),
    Verification(psi_terminal_verifier::VerificationError),
    Lowering(LoweringError),
}

impl std::fmt::Display for ArtifactLoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ArtifactLoweringError {}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}
