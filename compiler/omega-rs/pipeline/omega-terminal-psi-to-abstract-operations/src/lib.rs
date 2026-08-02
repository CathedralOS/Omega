#![forbid(unsafe_code)]

//! Lower verified terminal Psi into source-independent Omega realization
//! requirements.

use std::collections::{BTreeMap, BTreeSet};

use omega_terminal_abstract_operations::{
    TerminalAbstractFunction, TerminalAbstractOperation, TerminalAbstractOperationPlan,
    TerminalAbstractParameter, TerminalAbstractResult, TerminalValueBinding,
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
    let mut visited = BTreeSet::new();
    let mut current = machine.entry;

    loop {
        if !visited.insert(current) {
            return Err(LoweringError::VerifiedControlCycle {
                machine: machine.id,
                block: current,
            });
        }
        let block = blocks
            .get(&current)
            .copied()
            .ok_or(LoweringError::VerifiedBlockMissing {
                machine: machine.id,
                block: current,
            })?;
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
                current = *target;
            }
            Terminator::Return { edge, value } => {
                operations.push(TerminalAbstractOperation::Return {
                    psi_edge: *edge,
                    result: machine.result.id,
                    value: *value,
                    scalar_type: machine.result.scalar_type,
                });
                break;
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
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}
