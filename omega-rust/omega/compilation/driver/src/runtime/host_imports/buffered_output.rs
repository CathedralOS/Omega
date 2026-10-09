//! Invocation-owned stdout and stderr shared by exact installed adapters.
//! Append at dispatch, so different builtin and foreign slots cannot reorder
//! output by being collected separately. These bytes never reach OS stdout:
//! the consumer compares this private observation after execution.

use semantic_vocabulary::IntegerValue;
use std::sync::{Arc, Mutex};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalImportAdapter,
    TerminalImportSignature, TerminalScalarValue,
};

use super::{OutputKind, TerminalRuntimeError};

#[derive(Clone)]
pub(crate) struct BufferedBytes(Arc<Mutex<Vec<u8>>>);

/// Separate private streams shared by every adapter in one invocation.
#[derive(Clone)]
pub(crate) struct BufferedStreams {
    pub(crate) stdout: BufferedBytes,
    pub(crate) stderr: BufferedBytes,
}

impl BufferedStreams {
    pub(super) fn new() -> Self {
        Self {
            stdout: BufferedBytes::new(),
            stderr: BufferedBytes::new(),
        }
    }

    pub(super) fn append(
        &self,
        descriptor: i128,
        bytes: &[u8],
    ) -> Result<(), TerminalEffectRejection> {
        match descriptor {
            1 => self.stdout.append(bytes),
            2 => self.stderr.append(bytes),
            _ => Err(TerminalEffectRejection::new(
                "output descriptor is not an admitted private stream",
            )),
        }
    }
}

impl BufferedBytes {
    pub(super) fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    pub(super) fn append(&self, bytes: &[u8]) -> Result<(), TerminalEffectRejection> {
        self.0
            .lock()
            .map_err(|_| TerminalEffectRejection::new("output snapshot lock is poisoned"))?
            .extend_from_slice(bytes);
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, TerminalRuntimeError> {
        self.0
            .lock()
            .map(|bytes| bytes.clone())
            .map_err(|_| TerminalRuntimeError::InvalidProvision("output snapshot lock is poisoned"))
    }
}

pub(super) struct BufferedOutput {
    signature: TerminalImportSignature,
    kind: OutputKind,
    output: BufferedBytes,
}

impl BufferedOutput {
    pub(super) fn new(
        signature: TerminalImportSignature,
        kind: OutputKind,
        output: BufferedBytes,
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
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            byte_sequence_arguments,
            ..
        } = effect
        else {
            return Err(TerminalEffectRejection::new(
                "buffered output received a non-boundary operation",
            ));
        };
        if arguments.len() != self.signature.scalar_parameters.len()
            || !arguments
                .iter()
                .zip(&self.signature.scalar_parameters)
                .all(|(value, expected)| value.scalar_type() == *expected)
            || structural_arguments.len() != self.signature.structural_parameters.len()
            || byte_sequence_arguments.len() != structural_arguments.len()
            || byte_sequence_arguments.iter().any(Option::is_none)
        {
            return Err(TerminalEffectRejection::new(
                "buffered output operands differ from its bound contract",
            ));
        }
        match self.kind {
            OutputKind::Line => {
                let [Some(bytes)] = byte_sequence_arguments.as_slice() else {
                    return Err(TerminalEffectRejection::new(
                        "line output has no exact byte payload",
                    ));
                };
                self.output.append(bytes)?;
                self.output.append(b"\n")
            }
            OutputKind::Byte => {
                let [
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Signed(value),
                        ..
                    },
                ] = arguments.as_slice()
                else {
                    return Err(TerminalEffectRejection::new(
                        "byte output has no signed integer argument",
                    ));
                };
                let byte = u8::try_from(*value).map_err(|_| {
                    TerminalEffectRejection::new("byte output is outside its contract")
                })?;
                self.output.append(&[byte])
            }
        }
    }
}
