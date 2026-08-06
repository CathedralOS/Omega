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

/// Consume the complete verified module without consulting source or producer
/// state. The initial terminal vocabulary has one unconditional executable
/// chain per machine, so its Omega requirement stream is flat and ordered.
pub fn lower_verified_module(
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
                OperationKind::WrappingIntegerAdd { left, right } => {
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
                OperationKind::WrappingIntegerSubtract { left, right } => {
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
                OperationKind::WrappingIntegerMultiply { left, right } => {
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
    VerifiedSaturatingMultiplyMalformed(psi_core::OperationId),
    VerifiedIntegerBitwiseMalformed(psi_core::OperationId),
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}
