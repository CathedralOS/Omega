//! Canonical format-36 codec for privileged port-effect rows.
//!
//! The installation parent retains upfront count conversion, row ordering,
//! effect validation, and settlement association. This child owns exact bytes.

use machine_code::PortEffectRecord;
use semantic_vocabulary::{MachineId, OperationId, ServiceId};

use crate::installation_record::{
    InstallationError, ObjectPortEffect, Reader, push_u16, push_u32, push_u64,
};

pub(crate) fn encode_port_effects(
    bytes: &mut Vec<u8>,
    count: u32,
    installed: &[ObjectPortEffect],
) -> Result<(), InstallationError> {
    push_u32(bytes, count);
    for installed in installed {
        let effect = &installed.effect;
        push_u64(bytes, installed.machine.get());
        push_u64(bytes, effect.psi_operation.get());
        push_u64(bytes, effect.service.get());
        push_u16(bytes, effect.port);
        bytes.push(effect.value);
        bytes.push(0);
        for offset in [
            effect.operation_ordinal,
            installed.text_offset,
            effect.code_offset,
            effect.byte_count,
        ] {
            push_u64(
                bytes,
                u64::try_from(offset)
                    .map_err(|_| InstallationError::PortEffectOffsetNotRepresentable)?,
            );
        }
    }
    Ok(())
}

pub(crate) fn decode_port_effects(
    reader: &mut Reader<'_>,
) -> Result<Vec<ObjectPortEffect>, InstallationError> {
    let count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyPortEffects)?;
    if count > reader.remaining() / 60 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut port_effects = Vec::with_capacity(count);
    for _ in 0..count {
        let machine = MachineId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroPortEffectIdentity("MachineId"))?;
        let psi_operation = OperationId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroPortEffectIdentity("OperationId"))?;
        let service = ServiceId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroPortEffectIdentity("ServiceId"))?;
        let port = reader.u16()?;
        let value = reader.u8()?;
        if reader.u8()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        let operation_ordinal = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::PortEffectOffsetNotRepresentable)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::PortEffectOffsetNotRepresentable)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::PortEffectOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::PortEffectOffsetNotRepresentable)?;
        port_effects.push(ObjectPortEffect {
            machine,
            effect: PortEffectRecord {
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
