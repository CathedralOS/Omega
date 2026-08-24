//! Canonical format-35 codec for one installed function's stack facts.
//!
//! Function ordering and stack validation remain in the installation parent.
//! This child owns optional local envelopes and ordered unit/scalar call rows.

use psi_core::MachineId;

use super::{
    Reader, TerminalInstallationError, TerminalInstalledFunction,
    call_site_owner_codec::{decode_call_site_owner, encode_call_site_owner},
    push_u32, push_u64,
};

pub(super) fn encode_function_stack_facts(
    bytes: &mut Vec<u8>,
    function: &TerminalInstalledFunction,
) -> Result<(), TerminalInstallationError> {
    match function.unit_stack {
        Some(stack) => {
            bytes.push(1);
            bytes.extend_from_slice(&[0; 3]);
            push_u32(bytes, stack.frame_bytes);
            push_u32(bytes, stack.local_peak_bytes);
            push_u32(bytes, stack.stack_alignment);
        }
        None => bytes.extend_from_slice(&[0; 16]),
    }
    match function.scalar_stack {
        Some(stack) => {
            bytes.push(1);
            bytes.extend_from_slice(&[0; 3]);
            push_u32(bytes, stack.local_peak_bytes);
            push_u32(bytes, stack.stack_alignment);
        }
        None => bytes.extend_from_slice(&[0; 12]),
    }
    push_u32(
        bytes,
        u32::try_from(function.unit_call_stacks.len())
            .map_err(|_| TerminalInstallationError::TooManyStackCallFacts)?,
    );
    for call in &function.unit_call_stacks {
        encode_call_site_owner(bytes, call.owner);
        push_u64(bytes, call.target.get());
        push_u64(
            bytes,
            u64::try_from(call.text_offset)
                .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?,
        );
        push_u32(bytes, call.active_frame_bytes);
        push_u32(bytes, call.transient_bytes);
        push_u32(bytes, call.caller_live_bytes);
    }
    push_u32(
        bytes,
        u32::try_from(function.scalar_call_stacks.len())
            .map_err(|_| TerminalInstallationError::TooManyStackCallFacts)?,
    );
    for call in &function.scalar_call_stacks {
        encode_call_site_owner(bytes, call.owner);
        push_u64(bytes, call.target.get());
        push_u64(
            bytes,
            u64::try_from(call.text_offset)
                .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?,
        );
        push_u32(bytes, call.caller_live_bytes);
    }
    Ok(())
}

pub(super) fn decode_function_stack_facts(
    reader: &mut Reader<'_>,
) -> Result<
    (
        Option<crate::TerminalObjectUnitStack>,
        Option<crate::TerminalObjectScalarStack>,
        Vec<crate::TerminalObjectUnitCallStack>,
        Vec<crate::TerminalObjectScalarCallStack>,
    ),
    TerminalInstallationError,
> {
    let unit_stack = match reader.u8()? {
        0 => {
            if reader.take(3)? != [0; 3]
                || reader.u32()? != 0
                || reader.u32()? != 0
                || reader.u32()? != 0
            {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            None
        }
        1 => {
            if reader.take(3)? != [0; 3] {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            Some(crate::TerminalObjectUnitStack {
                frame_bytes: reader.u32()?,
                local_peak_bytes: reader.u32()?,
                stack_alignment: reader.u32()?,
            })
        }
        tag => return Err(TerminalInstallationError::InvalidPresenceFlag(tag)),
    };
    let scalar_stack = match reader.u8()? {
        0 => {
            if reader.take(3)? != [0; 3] || reader.u32()? != 0 || reader.u32()? != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            None
        }
        1 => {
            if reader.take(3)? != [0; 3] {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            Some(crate::TerminalObjectScalarStack {
                local_peak_bytes: reader.u32()?,
                stack_alignment: reader.u32()?,
            })
        }
        tag => return Err(TerminalInstallationError::InvalidPresenceFlag(tag)),
    };
    let unit_call_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStackCallFacts)?;
    if unit_call_count > reader.remaining() / 40 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut unit_call_stacks = Vec::with_capacity(unit_call_count);
    for _ in 0..unit_call_count {
        unit_call_stacks.push(crate::TerminalObjectUnitCallStack {
            owner: decode_call_site_owner(reader)?,
            target: MachineId::new(reader.u64()?)
                .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?,
            text_offset: usize::try_from(reader.u64()?)
                .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?,
            active_frame_bytes: reader.u32()?,
            transient_bytes: reader.u32()?,
            caller_live_bytes: reader.u32()?,
        });
    }
    let scalar_call_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStackCallFacts)?;
    if scalar_call_count > reader.remaining() / 32 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut scalar_call_stacks = Vec::with_capacity(scalar_call_count);
    for _ in 0..scalar_call_count {
        scalar_call_stacks.push(crate::TerminalObjectScalarCallStack {
            owner: decode_call_site_owner(reader)?,
            target: MachineId::new(reader.u64()?)
                .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?,
            text_offset: usize::try_from(reader.u64()?)
                .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?,
            caller_live_bytes: reader.u32()?,
        });
    }
    Ok((
        unit_stack,
        scalar_stack,
        unit_call_stacks,
        scalar_call_stacks,
    ))
}
