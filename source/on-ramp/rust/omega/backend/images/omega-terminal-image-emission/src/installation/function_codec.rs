//! Canonical format-36 codec for installed function rows.
//!
//! The installation parent retains upfront count conversion, cross-function
//! ordering, canonicality, and admission validation. This child composes rows.

use psi_core::{MachineId, StructuralTypeId};

use super::{
    Reader, TerminalInstallationError, TerminalInstalledFunction, decode_boolean,
    function_affine_cleanup_codec::{
        decode_scalar_control_affine_cleanups, decode_unit_affine_cleanup,
        encode_scalar_control_affine_cleanups, encode_unit_affine_cleanup,
    },
    function_parameter_codec::{
        decode_scalar_parameter_homes, decode_scalar_parameter_records,
        decode_unit_parameter_homes, decode_unit_parameter_records, encode_parameter_homes,
        encode_parameter_records,
    },
    function_stack_codec::{decode_function_stack_facts, encode_function_stack_facts},
    push_u32, push_u64,
};

pub(super) fn encode_functions(
    bytes: &mut Vec<u8>,
    count: u32,
    functions: &[TerminalInstalledFunction],
) -> Result<(), TerminalInstallationError> {
    push_u32(bytes, count);
    for function in functions {
        push_u64(bytes, function.machine.get());
        match function.attachment {
            Some(attachment) => {
                bytes.push(1);
                bytes.extend_from_slice(&[0; 7]);
                push_u64(bytes, attachment.get());
            }
            None => bytes.extend_from_slice(&[0; 16]),
        }
        push_u64(
            bytes,
            u64::try_from(function.text_offset)
                .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(function.byte_count)
                .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?,
        );
        encode_function_stack_facts(bytes, function)?;
        bytes.push(u8::from(function.unit_body));
        bytes.extend_from_slice(&[0; 3]);
        encode_parameter_records(bytes, &function.unit_parameters)?;
        encode_parameter_homes(bytes, &function.unit_parameter_homes)?;
        match &function.unit_affine_cleanup {
            Some(cleanup) => {
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                encode_unit_affine_cleanup(bytes, cleanup)?;
            }
            None => {
                bytes.push(0);
                bytes.extend_from_slice(&[0; 3]);
            }
        }
        encode_parameter_records(bytes, &function.scalar_structural_parameters)?;
        encode_parameter_homes(bytes, &function.scalar_structural_parameter_homes)?;
        match &function.scalar_affine_cleanup {
            Some(cleanup) => {
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                encode_unit_affine_cleanup(bytes, cleanup)?;
            }
            None => {
                bytes.push(0);
                bytes.extend_from_slice(&[0; 3]);
            }
        }
        encode_scalar_control_affine_cleanups(bytes, &function.scalar_control_affine_cleanups)?;
    }
    Ok(())
}

pub(super) fn decode_functions(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalInstalledFunction>, TerminalInstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyInstalledFunctions)?;
    if count > reader.remaining() / 24 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut functions = Vec::with_capacity(count);
    for _ in 0..count {
        let machine =
            MachineId::new(reader.u64()?).ok_or(TerminalInstallationError::ZeroFunctionIdentity)?;
        let attachment = match reader.u8()? {
            0 => {
                if reader.take(7)? != [0; 7] || reader.u64()? != 0 {
                    return Err(TerminalInstallationError::NonzeroReservedField);
                }
                None
            }
            1 => {
                if reader.take(7)? != [0; 7] {
                    return Err(TerminalInstallationError::NonzeroReservedField);
                }
                Some(StructuralTypeId::new(reader.u64()?).ok_or(
                    TerminalInstallationError::ZeroStructuralReturnIdentity("function attachment"),
                )?)
            }
            tag => return Err(TerminalInstallationError::InvalidPresenceFlag(tag)),
        };
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?;
        let (unit_stack, scalar_stack, unit_call_stacks, scalar_call_stacks) =
            decode_function_stack_facts(reader)?;
        let unit_body = decode_boolean(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        let unit_parameters = decode_unit_parameter_records(reader)?;
        let unit_parameter_homes = decode_unit_parameter_homes(reader)?;
        functions.push(TerminalInstalledFunction {
            machine,
            attachment,
            text_offset,
            byte_count,
            unit_stack,
            scalar_stack,
            unit_call_stacks,
            scalar_call_stacks,
            unit_body,
            unit_parameters,
            unit_parameter_homes,
            unit_affine_cleanup: match reader.u8()? {
                0 => {
                    if reader.take(3)? != [0; 3] {
                        return Err(TerminalInstallationError::NonzeroReservedField);
                    }
                    None
                }
                1 => {
                    if reader.take(3)? != [0; 3] {
                        return Err(TerminalInstallationError::NonzeroReservedField);
                    }
                    Some(decode_unit_affine_cleanup(reader)?)
                }
                tag => return Err(TerminalInstallationError::InvalidBoolean(tag)),
            },
            scalar_structural_parameters: decode_scalar_parameter_records(reader)?,
            scalar_structural_parameter_homes: decode_scalar_parameter_homes(reader)?,
            scalar_affine_cleanup: match reader.u8()? {
                0 => {
                    if reader.take(3)? != [0; 3] {
                        return Err(TerminalInstallationError::NonzeroReservedField);
                    }
                    None
                }
                1 => {
                    if reader.take(3)? != [0; 3] {
                        return Err(TerminalInstallationError::NonzeroReservedField);
                    }
                    Some(decode_unit_affine_cleanup(reader)?)
                }
                tag => return Err(TerminalInstallationError::InvalidBoolean(tag)),
            },
            scalar_control_affine_cleanups: decode_scalar_control_affine_cleanups(reader)?,
        });
    }
    Ok(functions)
}
