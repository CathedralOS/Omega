use std::collections::BTreeMap;

use psi_core::{IntegerType, IntegerValue, ScalarType, ValueId};
use psi_terminal::{OperationKind, TerminalMachine, Terminator};
use psi_terminal_verifier::VerifiedTerminalModule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalScalarValue {
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
}

impl TerminalScalarValue {
    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
        }
    }
}

/// Execute the verified terminal-Psi entry machine directly.
///
/// The initial executable vocabulary is a deterministic chain of integer
/// constants and explicit jump/return edges. Taking a
/// [`VerifiedTerminalModule`] makes verification and execution refer to the
/// same semantic module object.
pub fn interpret_terminal(
    verified: &VerifiedTerminalModule<'_>,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalScalarValue, TerminalInterpretError> {
    let module = verified.module();
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
    execute_machine(machine, arguments)
}

fn execute_machine(
    machine: &TerminalMachine,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalScalarValue, TerminalInterpretError> {
    if arguments.len() != machine.parameters.len() {
        return Err(TerminalInterpretError::ArgumentCount {
            expected: machine.parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut values = BTreeMap::new();
    for (parameter, argument) in machine.parameters.iter().zip(arguments) {
        if parameter.scalar_type != argument.scalar_type() {
            return Err(TerminalInterpretError::ArgumentType {
                value: parameter.id,
                expected: parameter.scalar_type,
                actual: argument.scalar_type(),
            });
        }
        values.insert(parameter.id, *argument);
    }
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut current = machine.entry;

    loop {
        let block = blocks
            .get(&current)
            .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
        for operation in &block.operations {
            match operation.kind {
                OperationKind::IntegerConstant { value } => {
                    let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    };
                    values.insert(
                        operation.result.id,
                        TerminalScalarValue::Integer { scalar_type, value },
                    );
                }
            }
        }
        match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => {
                let target_block = blocks
                    .get(target)
                    .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
                let transferred = arguments
                    .iter()
                    .map(|argument| {
                        values
                            .get(argument)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                for (parameter, value) in target_block.parameters.iter().zip(transferred) {
                    values.insert(parameter.id, value);
                }
                current = *target;
            }
            Terminator::Return { value, .. } => {
                return values
                    .get(value)
                    .copied()
                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*value));
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInterpretError {
    ArgumentCount {
        expected: usize,
        actual: usize,
    },
    ArgumentType {
        value: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    VerifiedEntryMachineMissing,
    VerifiedBlockMissing,
    VerifiedOperationMalformed,
    VerifiedValueMissing(ValueId),
}

impl std::fmt::Display for TerminalInterpretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalInterpretError {}
