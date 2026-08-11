//! Fuel-bounded reference execution for verified terminal-Psi artifacts.
//!
//! The public entry accepts only canonical semantic/proof bytes and an
//! admission profile. It decodes and verifies those bytes before constructing
//! execution state; no source or checked-tree representation crosses this
//! boundary.

use std::collections::BTreeMap;

use psi_core::{BlockId, ClaimId, IntegerType, IntegerValue, MachineId, ScalarType, ValueId};
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
    let mut execution =
        TerminalExecution::start_artifact(semantic_bytes, proof_bytes, profile, arguments)?;
    let mut meter = TerminalFuelMeter::unbounded();
    let value = match execution
        .resume(&mut meter)
        .map_err(TerminalArtifactInterpretError::Execution)?
    {
        TerminalExecutionStatus::Complete(value) => value,
        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::Fuel(FuelMeterError::Exhausted(exhaustion)),
            ));
        }
        TerminalExecutionStatus::Crashed(crash) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::Crash(crash),
            ));
        }
    };
    Ok(MeasuredTerminalExecution {
        value,
        usage: meter.into_usage(),
    })
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

/// Resumable execution state created from canonical terminal-Psi artifact
/// sections.
///
/// Fuel exhaustion never advances `next_operation` or the current terminator,
/// so a sponsor can replenish the same meter and resume without replaying
/// semantic work or charging it twice.
pub struct TerminalExecution {
    machines: BTreeMap<MachineId, ExecutableMachine>,
    blocks: BTreeMap<BlockId, Block>,
    values: BTreeMap<ValueId, TerminalScalarValue>,
    current: BlockId,
    next_operation: usize,
    call_stack: Vec<SuspendedCall>,
    result: Option<TerminalScalarValue>,
    crash: Option<TerminalCrash>,
}

#[derive(Clone)]
struct ExecutableMachine {
    parameters: Vec<psi_terminal::ValueDeclaration>,
    entry: BlockId,
    blocks: BTreeMap<BlockId, Block>,
}

struct SuspendedCall {
    blocks: BTreeMap<BlockId, Block>,
    values: BTreeMap<ValueId, TerminalScalarValue>,
    current: BlockId,
    next_operation: usize,
    result: ValueId,
}

impl TerminalExecution {
    /// Canonical-decode, verify, and begin one resumable artifact execution.
    /// The resulting state owns its verified entry block graph, so no decoded
    /// producer object or self-referential verifier borrow escapes this entry.
    pub fn start_artifact(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &psi_proof_kernel::AdmissionProfile,
        arguments: &[TerminalScalarValue],
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let module = psi_terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        let proof = psi_terminal_codec::decode_proof_bundle(proof_bytes)
            .map_err(TerminalArtifactInterpretError::ProofDecode)?;
        let verified = psi_terminal_verifier::verify_module(&module, &proof, profile)
            .map_err(TerminalArtifactInterpretError::Verification)?;
        Self::start(&verified, arguments).map_err(TerminalArtifactInterpretError::Execution)
    }

    fn start(
        verified: &VerifiedTerminalModule<'_>,
        arguments: &[TerminalScalarValue],
    ) -> Result<Self, TerminalInterpretError> {
        let module = verified.module();
        let machines = module
            .machines
            .iter()
            .map(|machine| {
                (
                    machine.id,
                    ExecutableMachine {
                        parameters: machine.parameters.clone(),
                        entry: machine.entry,
                        blocks: machine
                            .blocks
                            .iter()
                            .map(|block| (block.id, block.clone()))
                            .collect(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let machine = machines
            .get(&module.entry)
            .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
        let values = bind_arguments(&machine.parameters, arguments)?;
        let blocks = machine.blocks.clone();
        let current = machine.entry;
        Ok(Self {
            machines,
            blocks,
            values,
            current,
            next_operation: 0,
            call_stack: Vec::new(),
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
            while let Some(operation) = self
                .blocks
                .get(&self.current)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                .operations
                .get(self.next_operation)
                .cloned()
            {
                if let Err(error) = meter.charge_operation(&operation) {
                    return meter_status(error);
                }
                match operation.kind.clone() {
                    OperationKind::Call {
                        callee, arguments, ..
                    } => {
                        let arguments = arguments
                            .iter()
                            .map(|argument| {
                                self.values
                                    .get(argument)
                                    .copied()
                                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let callee = self
                            .machines
                            .get(&callee)
                            .cloned()
                            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee))?;
                        let values = bind_arguments(&callee.parameters, &arguments)?;
                        self.next_operation += 1;
                        self.call_stack.push(SuspendedCall {
                            blocks: std::mem::take(&mut self.blocks),
                            values: std::mem::take(&mut self.values),
                            current: self.current,
                            next_operation: self.next_operation,
                            result: operation.result.id,
                        });
                        self.blocks = callee.blocks;
                        self.values = values;
                        self.current = callee.entry;
                        self.next_operation = 0;
                        continue;
                    }
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
                        let result = match operation.kind.clone() {
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
                    OperationKind::IntegerWiden { operand } => {
                        let ScalarType::Integer(target_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: source_type,
                            value,
                        } = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let value = source_type
                            .widen_value_to(target_type, value)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer {
                                scalar_type: target_type,
                                value,
                            },
                        );
                    }
                    OperationKind::IntegerExactCast { operand, .. } => {
                        let ScalarType::Integer(target_type) = operation.result.scalar_type else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: source_type,
                            value,
                        } = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let value = source_type
                            .exact_cast_value_to(target_type, value)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.id,
                            TerminalScalarValue::Integer {
                                scalar_type: target_type,
                                value,
                            },
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
                        let value = match operation.kind.clone() {
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
                    | OperationKind::WrappingIntegerShiftRight { value, count }
                    | OperationKind::ExactIntegerShiftLeft { value, count, .. }
                    | OperationKind::ExactIntegerShiftRight { value, count, .. } => {
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
                        let value = match operation.kind.clone() {
                            OperationKind::WrappingIntegerShiftLeft { .. } => {
                                value_type.wrapping_shift_left(value, count_type, count)
                            }
                            OperationKind::WrappingIntegerShiftRight { .. } => {
                                value_type.wrapping_shift_right(value, count_type, count)
                            }
                            OperationKind::ExactIntegerShiftLeft { .. } => {
                                value_type.exact_shift_left(value, count_type, count)
                            }
                            OperationKind::ExactIntegerShiftRight { .. } => {
                                value_type.exact_shift_right(value, count_type, count)
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
                    OperationKind::ExactIntegerAdd { left, right, .. }
                    | OperationKind::WrappingIntegerAdd { left, right }
                    | OperationKind::ExactIntegerSubtract { left, right, .. }
                    | OperationKind::WrappingIntegerSubtract { left, right }
                    | OperationKind::ExactIntegerMultiply { left, right, .. }
                    | OperationKind::ExactIntegerDivide { left, right, .. }
                    | OperationKind::ExactIntegerRemainder { left, right, .. }
                    | OperationKind::WrappingIntegerDivide { left, right, .. }
                    | OperationKind::WrappingIntegerRemainder { left, right, .. }
                    | OperationKind::SaturatingIntegerDivide { left, right, .. }
                    | OperationKind::SaturatingIntegerRemainder { left, right, .. }
                    | OperationKind::WrappingIntegerMultiply { left, right } => {
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
                        let value = match operation.kind.clone() {
                            OperationKind::ExactIntegerAdd { .. } => {
                                scalar_type.exact_add(left, right)
                            }
                            OperationKind::WrappingIntegerAdd { .. } => {
                                scalar_type.wrapping_add(left, right)
                            }
                            OperationKind::ExactIntegerSubtract { .. } => {
                                scalar_type.exact_sub(left, right)
                            }
                            OperationKind::WrappingIntegerSubtract { .. } => {
                                scalar_type.wrapping_sub(left, right)
                            }
                            OperationKind::ExactIntegerMultiply { .. } => {
                                scalar_type.exact_mul(left, right)
                            }
                            OperationKind::ExactIntegerDivide { .. } => {
                                scalar_type.exact_div(left, right)
                            }
                            OperationKind::ExactIntegerRemainder { .. } => {
                                scalar_type.exact_rem(left, right)
                            }
                            OperationKind::WrappingIntegerDivide { .. } => {
                                scalar_type.wrapping_div(left, right)
                            }
                            OperationKind::WrappingIntegerRemainder { .. } => {
                                scalar_type.wrapping_rem(left, right)
                            }
                            OperationKind::SaturatingIntegerDivide { .. } => {
                                scalar_type.saturating_div(left, right)
                            }
                            OperationKind::SaturatingIntegerRemainder { .. } => {
                                scalar_type.saturating_rem(left, right)
                            }
                            OperationKind::WrappingIntegerMultiply { .. } => {
                                scalar_type.wrapping_mul(left, right)
                            }
                            _ => unreachable!(),
                        }
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
            let terminator = self
                .blocks
                .get(&self.current)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                .terminator
                .clone();
            match &terminator {
                Terminator::Jump {
                    target, arguments, ..
                } => {
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    let target_block = self
                        .blocks
                        .get(target)
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
                    if let Err(error) = meter.charge_edge(successor.edge, &terminator) {
                        return meter_status(error);
                    }
                    let target_block = self
                        .blocks
                        .get(&successor.target)
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
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    let result = self
                        .values
                        .get(value)
                        .copied()
                        .ok_or(TerminalInterpretError::VerifiedValueMissing(*value))?;
                    if let Some(caller) = self.call_stack.pop() {
                        self.blocks = caller.blocks;
                        self.values = caller.values;
                        self.values.insert(caller.result, result);
                        self.current = caller.current;
                        self.next_operation = caller.next_operation;
                        continue;
                    }
                    self.result = Some(result);
                    return Ok(TerminalExecutionStatus::Complete(result));
                }
                Terminator::Crash {
                    edge,
                    cause,
                    site_guard,
                    frontier_lower_bound,
                    ..
                } => {
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    let crash = TerminalCrash {
                        edge: *edge,
                        cause: *cause,
                        site_guard: site_guard.clone(),
                        frontier_lower_bound: frontier_lower_bound.clone(),
                    };
                    self.crash = Some(crash.clone());
                    return Ok(TerminalExecutionStatus::Crashed(crash));
                }
            }
        }
    }
}

fn bind_arguments(
    parameters: &[psi_terminal::ValueDeclaration],
    arguments: &[TerminalScalarValue],
) -> Result<BTreeMap<ValueId, TerminalScalarValue>, TerminalInterpretError> {
    if arguments.len() != parameters.len() {
        return Err(TerminalInterpretError::ArgumentCount {
            expected: parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut values = BTreeMap::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
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
    Ok(values)
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
    pub edge: psi_core::EdgeId,
    pub cause: CrashCause,
    pub site_guard: Vec<psi_terminal::CrashPredicateTerm>,
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
    VerifiedCallTargetMissing(MachineId),
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
