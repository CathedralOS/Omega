//! Canonical format-36 codec for privileged port-effect rows.
//!
//! The installation parent retains upfront count conversion, row ordering,
//! effect validation, and settlement association. This child owns exact bytes.

use omega_terminal_machine_code::TerminalPortEffectRecord;
use psi_core::{MachineId, OperationId, ServiceId};

use super::{
    Reader, TerminalInstallationError, TerminalObjectPortEffect, push_u16, push_u32, push_u64,
};

pub(super) fn encode_port_effects(
    bytes: &mut Vec<u8>,
    count: u32,
    installed: &[TerminalObjectPortEffect],
) -> Result<(), TerminalInstallationError> {
    push_u32(bytes, count);
    for installed in installed {
        let effect = &installed.effect;
        push_u64(bytes, installed.machine.get());
        push_u64(bytes, effect.psi_operation.get());
        push_u64(bytes, effect.service.get());
        push_u16(bytes, effect.port);
        bytes.push(effect.value);
        bytes.push(0);
        push_u64(
            bytes,
            u64::try_from(effect.operation_ordinal)
                .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(installed.text_offset)
                .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(effect.code_offset)
                .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(effect.byte_count)
                .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?,
        );
    }
    Ok(())
}

pub(super) fn decode_port_effects(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalObjectPortEffect>, TerminalInstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyPortEffects)?;
    if count > reader.remaining() / 60 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut port_effects = Vec::with_capacity(count);
    for _ in 0..count {
        let machine = MachineId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroPortEffectIdentity("MachineId"),
        )?;
        let psi_operation = OperationId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroPortEffectIdentity("OperationId"),
        )?;
        let service = ServiceId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroPortEffectIdentity("ServiceId"),
        )?;
        let port = reader.u16()?;
        let value = reader.u8()?;
        if reader.u8()? != 0 {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        let operation_ordinal = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?;
        port_effects.push(TerminalObjectPortEffect {
            machine,
            effect: TerminalPortEffectRecord {
                psi_operation,
                service,
                port,
                value,
                operation_ordinal,
                code_offset,
                byte_count,
            },
            text_offset,
        });
    }
    Ok(port_effects)
}
