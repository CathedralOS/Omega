//! Canonical installation codec for installed function rows.
//!
//! The installation parent retains upfront count conversion, cross-function
//! ordering, canonicality, and admission validation. This child composes rows.

use psi_core::{MachineId, StructuralTypeId};

use super::{
    InstallationError, InstalledFunction, Reader, decode_boolean,
    fixed_integer_scalar_abi_codec::{
        decode_fixed_integer_scalar_abi, encode_fixed_integer_scalar_abi,
    },
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
    unit_scalar_codec::{
        decode_unit_integer_constants, decode_unit_scalar_homes, encode_unit_integer_constants,
        encode_unit_scalar_homes,
    },
    unit_structural_scalar_field_store_codec::{
        decode_unit_structural_scalar_field_stores, encode_unit_structural_scalar_field_stores,
    },
};

pub(super) fn encode_functions(
    bytes: &mut Vec<u8>,
    count: u32,
    functions: &[InstalledFunction],
) -> Result<(), InstallationError> {
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
                .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(function.byte_count)
                .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?,
        );
        encode_function_stack_facts(bytes, function)?;
        bytes.push(u8::from(function.unit_body));
        bytes.push(u8::from(function.ranked_u32_countdown));
        bytes.extend_from_slice(&[0; 2]);
        encode_fixed_integer_scalar_abi(bytes, function.fixed_integer_scalar_abi.as_ref())?;
        encode_parameter_records(bytes, &function.unit_parameters)?;
        encode_parameter_homes(bytes, &function.unit_parameter_homes)?;
        encode_unit_scalar_homes(bytes, &function.unit_scalar_homes)?;
        encode_unit_integer_constants(bytes, &function.unit_integer_constants)?;
        encode_unit_structural_scalar_field_stores(
            bytes,
            &function.unit_structural_scalar_field_stores,
        )?;
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
) -> Result<Vec<InstalledFunction>, InstallationError> {
    let count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyInstalledFunctions)?;
    if count > reader.remaining() / 24 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut functions = Vec::with_capacity(count);
    for _ in 0..count {
        let machine =
            MachineId::new(reader.u64()?).ok_or(InstallationError::ZeroFunctionIdentity)?;
        let attachment = match reader.u8()? {
            0 => {
                if reader.take(7)? != [0; 7] || reader.u64()? != 0 {
                    return Err(InstallationError::NonzeroReservedField);
                }
                None
            }
            1 => {
                if reader.take(7)? != [0; 7] {
                    return Err(InstallationError::NonzeroReservedField);
                }
                Some(StructuralTypeId::new(reader.u64()?).ok_or(
                    InstallationError::ZeroStructuralReturnIdentity("function attachment"),
                )?)
            }
            tag => return Err(InstallationError::InvalidPresenceFlag(tag)),
        };
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?;
        let (unit_stack, scalar_stack, unit_call_stacks, scalar_call_stacks, foreign_call_stacks) =
            decode_function_stack_facts(reader)?;
        let unit_body = decode_boolean(reader.u8()?)?;
        let ranked_u32_countdown = decode_boolean(reader.u8()?)?;
        if reader.take(2)? != [0; 2] {
            return Err(InstallationError::NonzeroReservedField);
        }
        let fixed_integer_scalar_abi = decode_fixed_integer_scalar_abi(reader)?;
        let unit_parameters = decode_unit_parameter_records(reader)?;
        let unit_parameter_homes = decode_unit_parameter_homes(reader)?;
        let unit_scalar_homes = decode_unit_scalar_homes(reader)?;
        let unit_integer_constants = decode_unit_integer_constants(reader)?;
        let unit_structural_scalar_field_stores =
            decode_unit_structural_scalar_field_stores(reader)?;
        functions.push(InstalledFunction {
            machine,
            attachment,
            fixed_integer_scalar_abi,
            text_offset,
            byte_count,
            unit_stack,
            scalar_stack,
            unit_call_stacks,
            scalar_call_stacks,
            foreign_call_stacks,
            unit_body,
            ranked_u32_countdown,
            unit_parameters,
            unit_parameter_homes,
            unit_scalar_homes,
            unit_integer_constants,
            unit_structural_scalar_field_stores,
            unit_affine_cleanup: match reader.u8()? {
                0 => {
                    if reader.take(3)? != [0; 3] {
                        return Err(InstallationError::NonzeroReservedField);
                    }
                    None
                }
                1 => {
                    if reader.take(3)? != [0; 3] {
                        return Err(InstallationError::NonzeroReservedField);
                    }
                    Some(decode_unit_affine_cleanup(reader)?)
                }
                tag => return Err(InstallationError::InvalidBoolean(tag)),
            },
            scalar_structural_parameters: decode_scalar_parameter_records(reader)?,
            scalar_structural_parameter_homes: decode_scalar_parameter_homes(reader)?,
            scalar_affine_cleanup: match reader.u8()? {
                0 => {
                    if reader.take(3)? != [0; 3] {
                        return Err(InstallationError::NonzeroReservedField);
                    }
                    None
                }
                1 => {
                    if reader.take(3)? != [0; 3] {
                        return Err(InstallationError::NonzeroReservedField);
                    }
                    Some(decode_unit_affine_cleanup(reader)?)
                }
                tag => return Err(InstallationError::InvalidBoolean(tag)),
            },
            scalar_control_affine_cleanups: decode_scalar_control_affine_cleanups(reader)?,
        });
    }
    Ok(functions)
}
