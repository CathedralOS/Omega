//! Canonical installation codec for one installed function's stack facts.
//!
//! Function ordering and stack validation remain in the installation parent.
//! This child owns optional local envelopes and ordered unit/scalar call rows.

use psi_core::MachineId;

use super::{
    InstallationError, InstalledForeignCallStack, InstalledFunction, Reader,
    call_site_owner_codec::{decode_call_site_owner, encode_call_site_owner},
    push_u32, push_u64,
};

pub(super) fn encode_function_stack_facts(
    bytes: &mut Vec<u8>,
    function: &InstalledFunction,
) -> Result<(), InstallationError> {
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
            .map_err(|_| InstallationError::TooManyStackCallFacts)?,
    );
    for call in &function.unit_call_stacks {
        encode_call_site_owner(bytes, call.owner);
        push_u64(bytes, call.target.get());
        push_u64(
            bytes,
            u64::try_from(call.text_offset)
                .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?,
        );
        push_u32(bytes, call.active_frame_bytes);
        push_u32(bytes, call.transient_bytes);
        push_u32(bytes, call.caller_live_bytes);
    }
    push_u32(
        bytes,
        u32::try_from(function.scalar_call_stacks.len())
            .map_err(|_| InstallationError::TooManyStackCallFacts)?,
    );
    for call in &function.scalar_call_stacks {
        encode_call_site_owner(bytes, call.owner);
        push_u64(bytes, call.target.get());
        push_u64(
            bytes,
            u64::try_from(call.text_offset)
                .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?,
        );
        push_u32(bytes, call.caller_live_bytes);
    }
    push_u32(
        bytes,
        u32::try_from(function.foreign_call_stacks.len())
            .map_err(|_| InstallationError::TooManyStackCallFacts)?,
    );
    for call in &function.foreign_call_stacks {
        encode_call_site_owner(bytes, call.owner);
        push_u64(
            bytes,
            u64::try_from(call.text_offset)
                .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?,
        );
        push_u32(bytes, call.caller_live_bytes);
        push_u32(bytes, 0);
        push_u64(bytes, call.provider_plan_report_identity);
        push_u64(
            bytes,
            call.contribution_report_identity.normalized_identity(),
        );
        bytes.extend_from_slice(&call.contribution_commitment.as_bytes());
        push_u64(bytes, call.contribution_bytes);
        push_u64(bytes, call.contribution_alignment);
    }
    Ok(())
}

pub(super) fn decode_function_stack_facts(
    reader: &mut Reader<'_>,
) -> Result<
    (
        Option<crate::ObjectUnitStack>,
        Option<crate::ObjectScalarStack>,
        Vec<crate::ObjectUnitCallStack>,
        Vec<crate::ObjectScalarCallStack>,
        Vec<InstalledForeignCallStack>,
    ),
    InstallationError,
> {
    let unit_stack = match reader.u8()? {
        0 => {
            if reader.take(3)? != [0; 3]
                || reader.u32()? != 0
                || reader.u32()? != 0
                || reader.u32()? != 0
            {
                return Err(InstallationError::NonzeroReservedField);
            }
            None
        }
        1 => {
            if reader.take(3)? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            Some(crate::ObjectUnitStack {
                frame_bytes: reader.u32()?,
                local_peak_bytes: reader.u32()?,
                stack_alignment: reader.u32()?,
            })
        }
        tag => return Err(InstallationError::InvalidPresenceFlag(tag)),
    };
    let scalar_stack = match reader.u8()? {
        0 => {
            if reader.take(3)? != [0; 3] || reader.u32()? != 0 || reader.u32()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            None
        }
        1 => {
            if reader.take(3)? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            Some(crate::ObjectScalarStack {
                local_peak_bytes: reader.u32()?,
                stack_alignment: reader.u32()?,
            })
        }
        tag => return Err(InstallationError::InvalidPresenceFlag(tag)),
    };
    let unit_call_count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyStackCallFacts)?;
    if unit_call_count > reader.remaining() / 40 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut unit_call_stacks = Vec::with_capacity(unit_call_count);
    for _ in 0..unit_call_count {
        unit_call_stacks.push(crate::ObjectUnitCallStack {
            owner: decode_call_site_owner(reader)?,
            target: MachineId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?,
            text_offset: usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?,
            active_frame_bytes: reader.u32()?,
            transient_bytes: reader.u32()?,
            caller_live_bytes: reader.u32()?,
        });
    }
    let scalar_call_count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyStackCallFacts)?;
    if scalar_call_count > reader.remaining() / 32 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut scalar_call_stacks = Vec::with_capacity(scalar_call_count);
    for _ in 0..scalar_call_count {
        scalar_call_stacks.push(crate::ObjectScalarCallStack {
            owner: decode_call_site_owner(reader)?,
            target: MachineId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?,
            text_offset: usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?,
            caller_live_bytes: reader.u32()?,
        });
    }
    let foreign_call_count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyStackCallFacts)?;
    if foreign_call_count > reader.remaining() / 92 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut foreign_call_stacks = Vec::with_capacity(foreign_call_count);
    for _ in 0..foreign_call_count {
        let owner = decode_call_site_owner(reader)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::FunctionOffsetNotRepresentable)?;
        let caller_live_bytes = reader.u32()?;
        if reader.u32()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        let provider_plan_report_identity = reader.u64()?;
        let contribution_report_identity =
            omega_task_plans::AdmittedStackContributionReportId::from_normalized_identity(
                reader.u64()?,
            )
            .map_err(|_| InstallationError::InvalidForeignStackContribution)?;
        let contribution_commitment =
            omega_task_plans::SameStackContributionCommitment::from_digest(reader.array()?);
        let contribution_bytes = reader.u64()?;
        let contribution_alignment = reader.u64()?;
        foreign_call_stacks.push(InstalledForeignCallStack {
            owner,
            text_offset,
            caller_live_bytes,
            provider_plan_report_identity,
            contribution_report_identity,
            contribution_commitment,
            contribution_bytes,
            contribution_alignment,
        });
    }
    Ok((
        unit_stack,
        scalar_stack,
        unit_call_stacks,
        scalar_call_stacks,
        foreign_call_stacks,
    ))
}
