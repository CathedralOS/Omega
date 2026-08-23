//! Canonical format-34 codec for native fuel-attribution rows.
//!
//! The installation parent retains upfront count conversion, row ordering,
//! canonicality, and schedule validation. This child owns exact collection bytes.

use omega_terminal_machine_code::{TerminalNativeFuelAttribution, TerminalNativeFuelSite};
use psi_core::{EdgeId, FuelScheduleIdentity, MachineId, OperationId};

use super::{Reader, TerminalInstallationError, TerminalObjectFuelAttribution, push_u32, push_u64};

pub(super) fn encode_fuel_attributions(
    bytes: &mut Vec<u8>,
    count: u32,
    installed: &[TerminalObjectFuelAttribution],
) -> Result<(), TerminalInstallationError> {
    push_u32(bytes, count);
    for installed in installed {
        let attribution = &installed.attribution;
        push_u64(bytes, installed.machine.get());
        push_u32(bytes, attribution.schedule.marker());
        match attribution.site {
            TerminalNativeFuelSite::Operation(operation) => {
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(bytes, operation.get());
            }
            TerminalNativeFuelSite::Edge(edge) => {
                bytes.push(2);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(bytes, edge.get());
            }
        }
        push_u64(bytes, attribution.units);
        push_u64(
            bytes,
            u64::try_from(attribution.operation_ordinal)
                .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(installed.text_offset)
                .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(attribution.code_offset)
                .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(attribution.byte_count)
                .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
    }
    Ok(())
}

pub(super) fn decode_fuel_attributions(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalObjectFuelAttribution>, TerminalInstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyFuelAttributions)?;
    if count > reader.remaining() / 64 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut fuel_attribution = Vec::with_capacity(count);
    for _ in 0..count {
        let machine = MachineId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroFuelAttributionIdentity("MachineId"),
        )?;
        let schedule = FuelScheduleIdentity::new(reader.u32()?)
            .ok_or(TerminalInstallationError::ZeroFuelScheduleIdentity)?;
        let site_tag = reader.u8()?;
        if reader.take(3)? != [0; 3] {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        let site_identity = reader.u64()?;
        let site = match site_tag {
            1 => TerminalNativeFuelSite::Operation(OperationId::new(site_identity).ok_or(
                TerminalInstallationError::ZeroFuelAttributionIdentity("OperationId"),
            )?),
            2 => TerminalNativeFuelSite::Edge(EdgeId::new(site_identity).ok_or(
                TerminalInstallationError::ZeroFuelAttributionIdentity("EdgeId"),
            )?),
            _ => return Err(TerminalInstallationError::InvalidFuelSiteTag(site_tag)),
        };
        let units = reader.u64()?;
        let operation_ordinal = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?;
        fuel_attribution.push(TerminalObjectFuelAttribution {
            machine,
            attribution: TerminalNativeFuelAttribution {
                schedule,
                site,
                units,
                operation_ordinal,
                code_offset,
                byte_count,
            },
            text_offset,
        });
    }
    Ok(fuel_attribution)
}
