//! Canonical format-36 codec for native fuel-attribution rows.
//!
//! The installation parent retains upfront count conversion, row ordering,
//! canonicality, and schedule validation. This child owns exact collection bytes.

use omega_machine_code::{NativeFuelAttribution, NativeFuelSite};
use psi_core::{EdgeId, FuelScheduleIdentity, MachineId, OperationId};

use super::{InstallationError, ObjectFuelAttribution, Reader, push_u32, push_u64};

pub(super) fn encode_fuel_attributions(
    bytes: &mut Vec<u8>,
    count: u32,
    installed: &[ObjectFuelAttribution],
) -> Result<(), InstallationError> {
    push_u32(bytes, count);
    for installed in installed {
        let attribution = &installed.attribution;
        push_u64(bytes, installed.machine.get());
        push_u32(bytes, attribution.schedule.marker());
        match attribution.site {
            NativeFuelSite::Operation(operation) => {
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(bytes, operation.get());
            }
            NativeFuelSite::Edge(edge) => {
                bytes.push(2);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(bytes, edge.get());
            }
        }
        push_u64(bytes, attribution.units);
        push_u64(
            bytes,
            u64::try_from(attribution.operation_ordinal)
                .map_err(|_| InstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(installed.text_offset)
                .map_err(|_| InstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(attribution.code_offset)
                .map_err(|_| InstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(attribution.byte_count)
                .map_err(|_| InstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
    }
    Ok(())
}

pub(super) fn decode_fuel_attributions(
    reader: &mut Reader<'_>,
) -> Result<Vec<ObjectFuelAttribution>, InstallationError> {
    let count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyFuelAttributions)?;
    if count > reader.remaining() / 64 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut fuel_attribution = Vec::with_capacity(count);
    for _ in 0..count {
        let machine = MachineId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroFuelAttributionIdentity("MachineId"))?;
        let schedule = FuelScheduleIdentity::new(reader.u32()?)
            .ok_or(InstallationError::ZeroFuelScheduleIdentity)?;
        let site_tag = reader.u8()?;
        if reader.take(3)? != [0; 3] {
            return Err(InstallationError::NonzeroReservedField);
        }
        let site_identity = reader.u64()?;
        let site = match site_tag {
            1 => NativeFuelSite::Operation(OperationId::new(site_identity).ok_or(
                InstallationError::ZeroFuelAttributionIdentity("OperationId"),
            )?),
            2 => NativeFuelSite::Edge(
                EdgeId::new(site_identity)
                    .ok_or(InstallationError::ZeroFuelAttributionIdentity("EdgeId"))?,
            ),
            _ => return Err(InstallationError::InvalidFuelSiteTag(site_tag)),
        };
        let units = reader.u64()?;
        let operation_ordinal = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::FuelAttributionOffsetNotRepresentable)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::FuelAttributionOffsetNotRepresentable)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::FuelAttributionOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::FuelAttributionOffsetNotRepresentable)?;
        fuel_attribution.push(ObjectFuelAttribution {
            machine,
            attribution: NativeFuelAttribution {
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
