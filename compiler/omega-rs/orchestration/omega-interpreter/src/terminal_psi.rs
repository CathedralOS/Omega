use std::collections::BTreeMap;

use psi_core::{BlockId, ClaimId, IntegerType, IntegerValue, ScalarType, ValueId};
use psi_terminal::{Block, CrashCause, OperationKind, Terminator};
use psi_terminal_fuel::{FuelExhaustion, FuelMeterError, TerminalFuelMeter, TerminalFuelUsage};
use psi_terminal_verifier::VerifiedTerminalModule;

/// Decode, verify, and execute the canonical semantic and proof sections of one
/// terminal-Psi artifact. This is the reference-interpreter trust boundary for
/// executable artifact content: no source, checked tree, producer-owned module,
/// or prevalidated Rust object crosses it. Installation and debug sections are
/// separately bound by the artifact manifest and do not affect interpretation.
pub fn interpret_terminal_artifact_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_kernel::AdmissionProfile,
    arguments: &[TerminalScalarValue],
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let module = psi_terminal_codec::decode_module(semantic_bytes)
        .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
    let proof = psi_terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(TerminalArtifactInterpretError::ProofDecode)?;
    let verified = psi_terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(TerminalArtifactInterpretError::Verification)?;
    interpret_terminal_measured(&verified, arguments)
        .map_err(TerminalArtifactInterpretError::Execution)
}

/// Decode, verify, and execute canonical terminal-Psi semantic/proof artifact
/// sections, returning only their semantic result.
pub fn interpret_terminal_artifact(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_kernel::AdmissionProfile,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalScalarValue, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_measured(semantic_bytes, proof_bytes, profile, arguments)
        .map(MeasuredTerminalExecution::into_value)
}

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
/// The executable vocabulary is a deterministic chain of scalar constants and
/// explicit jump/return edges. Taking a
/// [`VerifiedTerminalModule`] makes verification and execution refer to the
/// same semantic module object.
pub fn interpret_terminal(
    verified: &VerifiedTerminalModule<'_>,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalScalarValue, TerminalInterpretError> {
    interpret_terminal_measured(verified, arguments).map(MeasuredTerminalExecution::into_value)
}

/// Execute terminal Psi and return deterministic logical usage under the
/// current separately versioned schedule.
pub fn interpret_terminal_measured(
    verified: &VerifiedTerminalModule<'_>,
    arguments: &[TerminalScalarValue],
) -> Result<MeasuredTerminalExecution, TerminalInterpretError> {
    let mut meter = TerminalFuelMeter::unbounded();
    let value = interpret_terminal_with_meter(verified, arguments, &mut meter)?;
    Ok(MeasuredTerminalExecution {
        value,
        usage: meter.into_usage(),
    })
}

/// Execute terminal Psi against a sponsor-owned logical-fuel meter.
///
/// A finite meter reports exhaustion through this host API before the unpaid
/// semantic operation or edge executes. Terminal Psi has no instruction for
/// observing the allowance or catching exhaustion as a machine result.
pub fn interpret_terminal_with_meter(
    verified: &VerifiedTerminalModule<'_>,
    arguments: &[TerminalScalarValue],
    meter: &mut TerminalFuelMeter,
) -> Result<TerminalScalarValue, TerminalInterpretError> {
    let mut execution = TerminalExecution::start(verified, arguments)?;
    match execution.resume(meter)? {
        TerminalExecutionStatus::Complete(value) => Ok(value),
        TerminalExecutionStatus::SponsorExhausted(exhaustion) => Err(TerminalInterpretError::Fuel(
            FuelMeterError::Exhausted(exhaustion),
        )),
        TerminalExecutionStatus::Crashed(crash) => Err(TerminalInterpretError::Crash(crash)),
    }
}

/// Resumable execution state for one already-verified terminal-Psi entry.
///
/// Fuel exhaustion never advances `next_operation` or the current terminator,
/// so a sponsor can replenish the same meter and resume without replaying
/// semantic work or charging it twice.
pub struct TerminalExecution<'module> {
    blocks: BTreeMap<BlockId, &'module Block>,
    values: BTreeMap<ValueId, TerminalScalarValue>,
    current: BlockId,
    next_operation: usize,
    result: Option<TerminalScalarValue>,
    crash: Option<TerminalCrash>,
}

impl<'module> TerminalExecution<'module> {
    pub fn start(
        verified: &VerifiedTerminalModule<'module>,
        arguments: &[TerminalScalarValue],
    ) -> Result<Self, TerminalInterpretError> {
        let module = verified.module();
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
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
            if let TerminalScalarValue::Integer { scalar_type, value } = argument
                && !scalar_type.admits(*value)
            {
                return Err(TerminalInterpretError::ArgumentIntegerOutsideType {
                    value: parameter.id,
                });
            }
            values.insert(parameter.id, *argument);
        }
        let blocks = machine
            .blocks
            .iter()
            .map(|block| (block.id, block))
            .collect::<BTreeMap<_, _>>();
        Ok(Self {
            blocks,
            values,
            current: machine.entry,
            next_operation: 0,
            result: None,
            crash: None,
        })
    }

    pub fn resume(
        &mut self,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
        if let Some(result) = self.result {
            return Ok(TerminalExecutionStatus::Complete(result));
        }
        if let Some(crash) = &self.crash {
            return Ok(TerminalExecutionStatus::Crashed(crash.clone()));
        }

        loop {
            let block = self
                .blocks
                .get(&self.current)
                .copied()
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
            while let Some(operation) = block.operations.get(self.next_operation) {
                if let Err(error) = meter.charge_operation(operation) {
                    return meter_status(error);
                }
                match operation.kind {
                    OperationKind::IntegerConstant { value } => {
                        let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::BooleanConstant { value } => {
                        if operation.result.scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.values
                            .insert(operation.result.id, TerminalScalarValue::Boolean(value));
                    }
                    OperationKind::BooleanNot { operand } => {
                        if operation.result.scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Boolean(value) = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values
                            .insert(operation.result.id, TerminalScalarValue::Boolean(!value));
                    }
                    OperationKind::BooleanEqual { left, right } => {
                        if operation.result.scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Boolean(left) = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Boolean(right) = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Boolean(left == right),
                        );
                    }
                    OperationKind::IntegerEqual { left, right } => {
                        if operation.result.scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Integer {
                            scalar_type: left_type,
                            value: left,
                        } = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: right_type,
                            value: right,
                        } = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != right_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Boolean(left == right),
                        );
                    }
                    OperationKind::IntegerLessThan { left, right }
                    | OperationKind::IntegerLessOrEqual { left, right } => {
                        if operation.result.scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Integer {
                            scalar_type: left_type,
                            value: left_value,
                        } = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: right_type,
                            value: right_value,
                        } = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != right_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let ordering = left_type
                            .compare(left_value, right_value)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let result = match operation.kind {
                            OperationKind::IntegerLessThan { .. } => ordering.is_lt(),
                            OperationKind::IntegerLessOrEqual { .. } => !ordering.is_gt(),
                            _ => unreachable!(),
                        };
                        self.values
                            .insert(operation.result.id, TerminalScalarValue::Boolean(result));
                    }
                    OperationKind::IntegerBitwiseNot { operand } => {
                        let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: operand_type,
                            value: operand,
                        } = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if operand_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .bitwise_not(operand)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::IntegerBitwiseAnd { left, right }
                    | OperationKind::IntegerBitwiseOr { left, right }
                    | OperationKind::IntegerBitwiseXor { left, right } => {
                        let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: left_type,
                            value: left_value,
                        } = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: right_type,
                            value: right_value,
                        } = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = match operation.kind {
                            OperationKind::IntegerBitwiseAnd { .. } => {
                                scalar_type.bitwise_and(left_value, right_value)
                            }
                            OperationKind::IntegerBitwiseOr { .. } => {
                                scalar_type.bitwise_or(left_value, right_value)
                            }
                            OperationKind::IntegerBitwiseXor { .. } => {
                                scalar_type.bitwise_xor(left_value, right_value)
                            }
                            _ => unreachable!(),
                        }
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::WrappingIntegerShiftLeft { value, count }
                    | OperationKind::WrappingIntegerShiftRight { value, count } => {
                        let ScalarType::Integer(value_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: actual_value_type,
                            value,
                        } = self
                            .values
                            .get(&value)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: count_type,
                            value: count,
                        } = self
                            .values
                            .get(&count)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(count))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if actual_value_type != value_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = match operation.kind {
                            OperationKind::WrappingIntegerShiftLeft { .. } => {
                                value_type.wrapping_shift_left(value, count_type, count)
                            }
                            OperationKind::WrappingIntegerShiftRight { .. } => {
                                value_type.wrapping_shift_right(value, count_type, count)
                            }
                            _ => unreachable!(),
                        }
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer {
                                scalar_type: value_type,
                                value,
                            },
                        );
                    }
                    OperationKind::WrappingIntegerAdd { left, right } => {
                        let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .wrapping_add(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::SaturatingIntegerAdd { left, right } => {
                        let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .saturating_add(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::WrappingIntegerSubtract { left, right } => {
                        let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .wrapping_sub(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::SaturatingIntegerSubtract { left, right } => {
                        let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .saturating_sub(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::WrappingIntegerMultiply { left, right } => {
                        let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .wrapping_mul(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::SaturatingIntegerMultiply { left, right } => {
                        let ScalarType::Integer(scalar_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .saturating_mul(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                }
                self.next_operation += 1;
            }
            match &block.terminator {
                Terminator::Jump {
                    target, arguments, ..
                } => {
                    if let Err(error) = meter.charge_terminator(&block.terminator) {
                        return meter_status(error);
                    }
                    let target_block = self
                        .blocks
                        .get(target)
                        .copied()
                        .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
                    let transferred = arguments
                        .iter()
                        .map(|argument| {
                            self.values
                                .get(argument)
                                .copied()
                                .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (parameter, value) in target_block.parameters.iter().zip(transferred) {
                        self.values.insert(parameter.id, value);
                    }
                    self.current = *target;
                    self.next_operation = 0;
                }
                Terminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    let condition = self
                        .values
                        .get(condition)
                        .copied()
                        .ok_or(TerminalInterpretError::VerifiedValueMissing(*condition))?;
                    let TerminalScalarValue::Boolean(condition) = condition else {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    };
                    let successor = if condition { when_true } else { when_false };
                    if let Err(error) = meter.charge_edge(successor.edge, &block.terminator) {
                        return meter_status(error);
                    }
                    let target_block = self
                        .blocks
                        .get(&successor.target)
                        .copied()
                        .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
                    let transferred = successor
                        .arguments
                        .iter()
                        .map(|argument| {
                            self.values
                                .get(argument)
                                .copied()
                                .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (parameter, value) in target_block.parameters.iter().zip(transferred) {
                        self.values.insert(parameter.id, value);
                    }
                    self.current = successor.target;
                    self.next_operation = 0;
                }
                Terminator::Return { value, .. } => {
                    if let Err(error) = meter.charge_terminator(&block.terminator) {
                        return meter_status(error);
                    }
                    let result = self
                        .values
                        .get(value)
                        .copied()
                        .ok_or(TerminalInterpretError::VerifiedValueMissing(*value))?;
                    self.result = Some(result);
                    return Ok(TerminalExecutionStatus::Complete(result));
                }
                Terminator::Crash {
                    cause,
                    damage_minimum,
                    containment_demand,
                    frontier_lower_bound,
                    ..
                } => {
                    if let Err(error) = meter.charge_terminator(&block.terminator) {
                        return meter_status(error);
                    }
                    let crash = TerminalCrash {
                        cause: *cause,
                        damage_minimum: damage_minimum.clone(),
                        containment_demand: containment_demand.clone(),
                        frontier_lower_bound: frontier_lower_bound.clone(),
                    };
                    self.crash = Some(crash.clone());
                    return Ok(TerminalExecutionStatus::Crashed(crash));
                }
            }
        }
    }
}

fn meter_status(error: FuelMeterError) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
    match error {
        FuelMeterError::Exhausted(exhaustion) => {
            Ok(TerminalExecutionStatus::SponsorExhausted(exhaustion))
        }
        other => Err(TerminalInterpretError::Fuel(other)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalExecutionStatus {
    Complete(TerminalScalarValue),
    SponsorExhausted(FuelExhaustion),
    Crashed(TerminalCrash),
}

/// The explicit terminal-Psi crash outcome reached by an execution.
///
/// `frontier_lower_bound` is the machine-local claim frontier recorded by the
/// artifact. It is not an assertion that no wider runtime state was abandoned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCrash {
    pub cause: CrashCause,
    pub damage_minimum: String,
    pub containment_demand: String,
    pub frontier_lower_bound: Vec<ClaimId>,
}

/// A successful semantic result paired with deterministic terminal-Psi fuel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredTerminalExecution {
    value: TerminalScalarValue,
    usage: TerminalFuelUsage,
}

impl MeasuredTerminalExecution {
    pub const fn value(&self) -> TerminalScalarValue {
        self.value
    }

    pub const fn usage(&self) -> &TerminalFuelUsage {
        &self.usage
    }

    pub fn into_value(self) -> TerminalScalarValue {
        self.value
    }

    pub fn into_parts(self) -> (TerminalScalarValue, TerminalFuelUsage) {
        (self.value, self.usage)
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
    ArgumentIntegerOutsideType {
        value: ValueId,
    },
    VerifiedEntryMachineMissing,
    VerifiedBlockMissing,
    VerifiedOperationMalformed,
    VerifiedValueMissing(ValueId),
    Crash(TerminalCrash),
    Fuel(FuelMeterError),
}

#[derive(Debug)]
pub enum TerminalArtifactInterpretError {
    SemanticDecode(psi_terminal_codec::CodecError),
    ProofDecode(psi_terminal_codec::ProofCodecError),
    Verification(psi_terminal_verifier::VerificationError),
    Execution(TerminalInterpretError),
}

impl std::fmt::Display for TerminalArtifactInterpretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalArtifactInterpretError {}

impl From<FuelMeterError> for TerminalInterpretError {
    fn from(error: FuelMeterError) -> Self {
        Self::Fuel(error)
    }
}

impl std::fmt::Display for TerminalInterpretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalInterpretError {}
