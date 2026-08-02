#![forbid(unsafe_code)]

//! Lower verified terminal Psi into source-independent Omega realization
//! requirements.

use std::collections::{BTreeMap, BTreeSet};

use omega_terminal_abstract_operations::{
    TerminalAbstractFunction, TerminalAbstractOperation, TerminalAbstractOperationPlan,
    TerminalValueBinding,
};
use psi_core::{BlockId, MachineId};
use psi_terminal::{OperationKind, TerminalMachine, Terminator};
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
        operations,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    VerifiedEntryMachineMissing(MachineId),
    VerifiedBlockMissing { machine: MachineId, block: BlockId },
    VerifiedControlCycle { machine: MachineId, block: BlockId },
    VerifiedJumpArityMismatch { edge: psi_core::EdgeId },
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}
