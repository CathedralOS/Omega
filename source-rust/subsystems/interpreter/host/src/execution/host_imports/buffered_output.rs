// Invocation-owned `stdout` and `stderr` buffers (`BufferedStreams`,
// `BufferedOutput`) shared by all installed output adapters. Appending at
// dispatch prevents builtin and foreign slots from reordering output;
// `OutputKind::Byte` writes the low 8 bits of its signed 32-bit operand.

use semantic_vocabulary::IntegerValue;
use std::sync::{Arc, Mutex};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalImportAdapter,
    TerminalImportSignature, TerminalScalarValue,
};

use super::{OutputKind, TerminalRuntimeError};

#[derive(Clone)]
pub(crate) struct BufferedStreams {
    bytes: Arc<Mutex<StreamBytes>>,
    pub(super) limits: Option<crate::TerminalOutputLimits>,
}

#[derive(Default)]
struct StreamBytes {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl BufferedStreams {
    pub(super) fn new(limits: Option<crate::TerminalOutputLimits>) -> Self {
        Self {
            bytes: Arc::default(),
            limits,
        }
    }

    /// Check all bytes of one write before either stream can change.
    pub(super) fn append(
        &self,
        descriptor: i128,
        parts: &[&[u8]],
    ) -> Result<(), TerminalEffectRejection> {
        let mut bytes = self
            .bytes
            .lock()
            .map_err(|_| TerminalEffectRejection::new("output snapshot lock is poisoned"))?;
        let retained = bytes.stdout.len().saturating_add(bytes.stderr.len());
        let (destination, name, limit) = match descriptor {
            1 => (
                &mut bytes.stdout,
                "stdout",
                self.limits.map(|limits| limits.stdout_bytes),
            ),
            2 => (
                &mut bytes.stderr,
                "stderr",
                self.limits.map(|limits| limits.stderr_bytes),
            ),
            _ => {
                return Err(TerminalEffectRejection::new(
                    "output descriptor is not an admitted private stream",
                ));
            }
        };
        let count = parts
            .iter()
            .try_fold(0usize, |count, part| count.checked_add(part.len()))
            .ok_or_else(|| {
                TerminalEffectRejection::sponsor_exhausted("output length exceeds host storage")
            })?;
        if let Some(limit) = limit
            && count > limit.saturating_sub(destination.len())
        {
            return Err(TerminalEffectRejection::sponsor_exhausted(format!(
                "{name} exceeded its {limit}-byte capture allowance"
            )));
        }
        if let Some(limits) = self.limits
            && count > limits.combined_bytes.saturating_sub(retained)
        {
            return Err(TerminalEffectRejection::sponsor_exhausted(format!(
                "stdout and stderr exceeded their combined {}-byte capture allowance",
                limits.combined_bytes,
            )));
        }
        destination.try_reserve(count).map_err(|_| {
            TerminalEffectRejection::sponsor_exhausted(format!(
                "host cannot reserve {count} more {name} bytes"
            ))
        })?;
        for part in parts {
            destination.extend_from_slice(part);
        }
        Ok(())
    }

    pub(super) fn snapshot(&self) -> Result<(Vec<u8>, Vec<u8>), TerminalRuntimeError> {
        let bytes = self.bytes.lock().map_err(|_| {
            TerminalRuntimeError::InvalidProvision("output snapshot lock is poisoned")
        })?;
        Ok((bytes.stdout.clone(), bytes.stderr.clone()))
    }

    pub(super) fn take(&self) -> Result<(Vec<u8>, Vec<u8>), TerminalRuntimeError> {
        let bytes = std::mem::take(&mut *self.bytes.lock().map_err(|_| {
            TerminalRuntimeError::InvalidProvision("output snapshot lock is poisoned")
        })?);
        Ok((bytes.stdout, bytes.stderr))
    }
}

pub(super) struct BufferedOutput {
    signature: TerminalImportSignature,
    kind: OutputKind,
    output: BufferedStreams,
}

impl BufferedOutput {
    pub(super) fn new(
        signature: TerminalImportSignature,
        kind: OutputKind,
        output: BufferedStreams,
    ) -> Self {
        Self {
            signature,
            kind,
            output,
        }
    }
}

import_signature_via_clone!(BufferedOutput);

impl TerminalEffectHandler for BufferedOutput {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let call = super::call_operands(effect, || {
            TerminalEffectRejection::new("buffered output received a non-boundary operation")
        })?;
        if call.arguments.len() != self.signature.scalar_parameters.len()
            || !call
                .arguments
                .iter()
                .zip(&self.signature.scalar_parameters)
                .all(|(value, expected)| value.scalar_type() == *expected)
            || call.structural_arguments.len() != self.signature.structural_parameters.len()
            || call.byte_sequence_arguments.len() != call.structural_arguments.len()
            || call.byte_sequence_arguments.iter().any(Option::is_none)
        {
            return Err(TerminalEffectRejection::new(
                "buffered output operands differ from its bound contract",
            ));
        }
        match self.kind {
            OutputKind::Line => {
                let [Some(bytes)] = call.byte_sequence_arguments else {
                    return Err(TerminalEffectRejection::new(
                        "line output has no exact byte payload",
                    ));
                };
                self.output.append(1, &[bytes, b"\n"])
            }
            OutputKind::Byte => {
                let [
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Signed(value),
                        ..
                    },
                ] = call.arguments
                else {
                    return Err(TerminalEffectRejection::new(
                        "byte output has no signed integer argument",
                    ));
                };
                let value = i32::try_from(*value).map_err(|_| {
                    TerminalEffectRejection::new("byte output exceeds its signed 32-bit carrier")
                })?;
                self.output.append(1, &[&[value as u8]])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        BoundaryMachineId, IntegerSign, IntegerType, OperationId, ScalarType,
    };
    use terminal_psi::{BoundaryCompletion, BoundaryMachineResult};

    #[test]
    fn byte_output_truncates_i32_without_accepting_malformed_operands() {
        let signed = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let output = BufferedStreams::new(None);
        let mut adapter = BufferedOutput::new(
            TerminalImportSignature {
                completion: BoundaryCompletion::NormalReturn,
                scalar_parameters: vec![ScalarType::Integer(signed)],
                structural_parameters: vec![],
                result: BoundaryMachineResult::Unit,
                normal_qualifications: vec![],
                result_reference_regions: Vec::new(),
                result_lifetimes: None,
            },
            OutputKind::Byte,
            output.clone(),
        );
        let effect = |scalar_type, value| TerminalEffect::BoundaryCall {
            float_evaluation: terminal_interpreter::float_result_policy::FloatEvaluation::Runtime,
            site: terminal_interpreter::TerminalEffectWait {
                program: terminal_codec::TerminalExecutionIdentity::portable(
                    terminal_psi::TerminalPsiIdentity {
                        vocabulary_marker: terminal_psi::TerminalModule::for_entry(
                            semantic_vocabulary::MachineId::new(1).unwrap(),
                        )
                        .vocabulary_marker,
                        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0; 32]),
                    },
                ),
                machine: semantic_vocabulary::MachineId::new(1).unwrap(),
                block: semantic_vocabulary::BlockId::new(1).unwrap(),
                operation: OperationId::new(1).unwrap(),
                boundary: BoundaryMachineId::new(1).unwrap(),
            },
            arguments: vec![TerminalScalarValue::Integer { scalar_type, value }],
            structural_arguments: vec![],
            byte_sequence_arguments: vec![],
            completion_receipts: vec![],
            result: BoundaryMachineResult::Unit,
        };
        for value in [0x141, 0x100, -1, -0x100, i32::MIN, i32::MAX] {
            adapter
                .handle_effect(&effect(signed, IntegerValue::Signed(i128::from(value))))
                .unwrap();
        }
        assert_eq!(output.snapshot().unwrap().0, [0x41, 0, 0xFF, 0, 0, 0xFF]);
        for value in [i128::from(i32::MIN) - 1, i128::from(i32::MAX) + 1] {
            assert!(
                adapter
                    .handle_effect(&effect(signed, IntegerValue::Signed(value)))
                    .is_err()
            );
        }
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        assert!(
            adapter
                .handle_effect(&effect(unsigned, IntegerValue::Unsigned(0x41)))
                .is_err()
        );
        assert_eq!(output.snapshot().unwrap().0, [0x41, 0, 0xFF, 0, 0, 0xFF]);
    }

    #[test]
    fn cloned_output_adapters_share_the_invocation_ceiling() {
        let limits = crate::TerminalOutputLimits {
            stdout_bytes: 5,
            stderr_bytes: 3,
            combined_bytes: 6,
        };
        let streams = BufferedStreams::new(Some(limits));
        let child = streams.clone();
        streams.append(1, &[b"ab"]).unwrap();
        child.append(2, &[b"err"]).unwrap();
        let error = streams.append(1, &[b"x", b"\n"]).unwrap_err();
        assert_eq!(
            error.exhaustion_reason(),
            Some("stdout and stderr exceeded their combined 6-byte capture allowance")
        );
        let site = terminal_interpreter::TerminalInterpretError::ExecutionSite {
            machine: semantic_vocabulary::MachineId::new(1).unwrap(),
            block: semantic_vocabulary::BlockId::new(1).unwrap(),
            operation: Some(OperationId::new(1).unwrap()),
            operation_tag: None,
            cause: Box::new(
                terminal_interpreter::TerminalInterpretError::EffectRejected {
                    operation: OperationId::new(1).unwrap(),
                    rejection: error,
                },
            ),
        };
        let hosted = TerminalRuntimeError::Execution(
            terminal_interpreter::TerminalInstanceError::Execution(site),
        );
        assert_eq!(
            hosted.sponsor_exhaustion(),
            Some("stdout and stderr exceeded their combined 6-byte capture allowance")
        );
        let error = child.append(2, &[b"!"]).unwrap_err();
        assert_eq!(
            error.exhaustion_reason(),
            Some("stderr exceeded its 3-byte capture allowance")
        );
        streams.append(1, &[b"!"]).unwrap();
        assert_eq!(streams.take().unwrap(), (b"ab!".to_vec(), b"err".to_vec()));
        assert_eq!(
            BufferedStreams::new(Some(limits)).snapshot().unwrap(),
            (vec![], vec![])
        );
    }
}
