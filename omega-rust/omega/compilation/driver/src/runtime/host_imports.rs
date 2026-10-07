//! Explicit host bindings for exact admitted native host operation slots.
//! A slot's realization selects semantics; its complete declaration selects
//! the typed adapter. No source-name dispatch or stdout-driven exit exists.
//! Output stays buffered; input consumes only the caller's finite replay stream
//! with an instance-private cursor. Checked compound operations remain guest code.

mod numeric;
mod standard_input;

use compilation_products::TerminalRuntimeProvisions;
use semantic_vocabulary::{BoundaryMachineId, IntegerSign, IntegerType, IntegerValue, ScalarType};
use target_operations::{BoundaryRealization, BoundarySettlementRealization};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalImportAdapter,
    TerminalImportSignature, TerminalInstance, TerminalProgram, TerminalScalarValue,
};
use terminal_psi::BoundaryCompletion;

use super::TerminalRuntimeError;

pub(super) enum OutputKind {
    Line,
    Byte,
}

pub(super) fn bind<'host>(
    program: &TerminalProgram,
    instance: &mut TerminalInstance<'host>,
    provisions: Option<&TerminalRuntimeProvisions>,
    imports: &[BoundaryMachineId],
    input: Option<Box<dyn std::io::Read + Send + 'host>>,
) -> Result<Vec<(BoundaryMachineId, OutputKind)>, TerminalRuntimeError> {
    let mut output = Vec::new();
    // Every selected stdin leaf in this instance consumes the same logical
    // stream. Independent adapter cursors would replay bytes after a slot switch.
    let input = input.map(|reader| std::sync::Arc::new(std::sync::Mutex::new(reader)));
    for slot in imports {
        let provision = provisions
            .and_then(|provisions| {
                provisions
                    .host()
                    .iter()
                    .find(|provision| provision.boundary == *slot)
            })
            .ok_or_else(|| {
                TerminalRuntimeError::missing(
                    "runtime-host-import-bindings",
                    format!("slot {slot:?} has no admitted host realization"),
                )
            })?;
        let kind = match provision.realization {
            BoundarySettlementRealization::Builtin(BoundaryRealization::HostedReadByte(_)) => {
                let declaration =
                    instance
                        .import(*slot)
                        .ok_or(TerminalRuntimeError::InvalidProvision(
                            "stdin host provision names no demanded import",
                        ))?;
                let reader = input.as_ref().ok_or_else(|| {
                    TerminalRuntimeError::missing(
                        "runtime-standard-input",
                        "no finite replay stream was supplied",
                    )
                })?;
                let adapter = standard_input::ReadByteImport::for_import(
                    program,
                    declaration,
                    std::sync::Arc::clone(reader),
                )?;
                instance
                    .bind_import(*slot, adapter)
                    .map_err(TerminalRuntimeError::Binding)?;
                continue;
            }
            BoundarySettlementRealization::Builtin(BoundaryRealization::LinuxWriteLine(_)) => {
                OutputKind::Line
            }
            BoundarySettlementRealization::Builtin(BoundaryRealization::HostedWriteByteI32(_)) => {
                OutputKind::Byte
            }
            BoundarySettlementRealization::Builtin(
                BoundaryRealization::NearestIeeeFloatArithmetic(realization),
            ) => {
                let declaration =
                    instance
                        .import(*slot)
                        .ok_or(TerminalRuntimeError::InvalidProvision(
                            "numeric host provision names no demanded import",
                        ))?;
                let adapter = numeric::NumericImport::for_import(declaration, realization)?;
                instance
                    .bind_import(*slot, adapter)
                    .map_err(TerminalRuntimeError::Binding)?;
                continue;
            }
            _ => {
                return Err(TerminalRuntimeError::missing(
                    "runtime-host-import-bindings",
                    format!(
                        "slot {slot:?} needs a runtime adapter for {:?}",
                        provision.realization
                    ),
                ));
            }
        };
        let declaration = instance
            .import(*slot)
            .ok_or(TerminalRuntimeError::InvalidProvision(
                "host provision names no demanded import declaration",
            ))?;
        let signature = TerminalImportSignature::from(declaration);
        let valid = signature.completion == BoundaryCompletion::NormalReturn
            && signature.result.is_unit()
            && match kind {
                OutputKind::Line => {
                    signature.scalar_parameters.is_empty()
                        && signature.structural_parameters.len() == 1
                }
                OutputKind::Byte => {
                    signature.structural_parameters.is_empty()
                        && matches!(signature.scalar_parameters.as_slice(),
                [ScalarType::Integer(integer)] if integer.sign() == IntegerSign::Signed && integer.bits() == 32)
                }
            };
        if !valid {
            return Err(TerminalRuntimeError::InvalidProvision(
                "buffered host operation differs from its admitted declaration",
            ));
        }
        instance
            .bind_import(*slot, BufferedOutput { signature })
            .map_err(TerminalRuntimeError::Binding)?;
        output.push((*slot, kind));
    }
    Ok(output)
}

struct BufferedOutput {
    signature: TerminalImportSignature,
}

impl TerminalImportAdapter for BufferedOutput {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

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
            || byte_sequence_arguments.iter().any(Option::is_none)
        {
            return Err(TerminalEffectRejection::new(
                "buffered output operands differ from its bound contract",
            ));
        }
        Ok(())
    }
}

pub(super) fn collect_stdout(
    effects: &[TerminalEffect],
    output: &[(BoundaryMachineId, OutputKind)],
) -> Result<Vec<u8>, TerminalRuntimeError> {
    let mut stdout = Vec::new();
    for effect in effects {
        let TerminalEffect::BoundaryCall {
            boundary,
            arguments,
            byte_sequence_arguments,
            ..
        } = effect
        else {
            continue;
        };
        let Some((_, kind)) = output.iter().find(|(slot, _)| slot == boundary) else {
            continue;
        };
        match kind {
            OutputKind::Line => {
                let [Some(bytes)] = byte_sequence_arguments.as_slice() else {
                    return Err(TerminalRuntimeError::InvalidProvision(
                        "committed line output has no exact byte payload",
                    ));
                };
                stdout.extend_from_slice(bytes);
                stdout.push(b'\n');
            }
            OutputKind::Byte => {
                let [TerminalScalarValue::Integer { scalar_type, value }] = arguments.as_slice()
                else {
                    return Err(TerminalRuntimeError::InvalidProvision(
                        "committed byte output has no exact integer argument",
                    ));
                };
                let expected = IntegerType::new(IntegerSign::Signed, 32)
                    .map_err(|_| TerminalRuntimeError::InvalidProvision("i32 unavailable"))?;
                if *scalar_type != expected {
                    return Err(TerminalRuntimeError::InvalidProvision(
                        "byte output argument is not i32",
                    ));
                }
                let IntegerValue::Signed(value) = value else {
                    return Err(TerminalRuntimeError::InvalidProvision(
                        "byte output argument is not signed",
                    ));
                };
                stdout.push(u8::try_from(*value).map_err(|_| {
                    TerminalRuntimeError::InvalidProvision(
                        "byte output argument is outside its contract",
                    )
                })?);
            }
        }
    }
    Ok(stdout)
}
