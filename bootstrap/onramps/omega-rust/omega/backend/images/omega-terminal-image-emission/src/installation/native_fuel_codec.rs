//! Canonical format-36 native-fuel realization codec.
//!
//! Semantic rows remain in source coordinates in the parent record. This
//! section owns the distinct source-to-metered function map and exact
//! hot/semantic/cold charge catalog.

use omega_calling_conventions::MachineRegister;
use omega_target::{NativeTarget, TargetProfile};
use omega_terminal_installation_evidence::{
    NativeFuelContextLayout, NativeFuelTargetPlanProjection, SponsorContextTransport,
    TerminalFuelAttributionEvidence, TerminalFuelAttributionSite, TerminalNativeFuelChargeEvidence,
};
use psi_core::{EdgeId, FuelScheduleIdentity, MachineId, OperationId};

use super::{
    Reader, TerminalInstallationError, TerminalInstalledNativeFuel,
    TerminalInstalledNativeFuelFunction, TerminalNativeFuelSourceFingerprint, decode_boolean,
    push_u32, push_u64,
};

pub(super) fn encode_native_fuel(
    bytes: &mut Vec<u8>,
    installed: Option<&TerminalInstalledNativeFuel>,
) -> Result<(), TerminalInstallationError> {
    bytes.push(u8::from(installed.is_some()));
    bytes.extend_from_slice(&[0; 3]);
    let Some(installed) = installed else {
        return Ok(());
    };

    bytes.push(profile_tag(installed.target_policy.profile));
    bytes.push(transport_tag(installed.target_policy.transport));
    bytes.extend_from_slice(&[0; 2]);
    let context = installed.target_policy.context;
    for value in [
        context.byte_size,
        context.alignment,
        context.remaining_units_offset,
        context.unpaid_site_kind_offset,
        context.unpaid_site_identity_offset,
        context.required_units_offset,
        context.transfer_entry_offset,
        context.retry_code_offset_offset,
        context.sponsor_stack_top_offset,
        context.activation_state_offset,
        context.activation_state_byte_count,
    ] {
        push_u32(bytes, value);
    }
    push_u64(bytes, installed.target_policy.transfer_plan_identity);
    bytes.extend_from_slice(installed.source_text_fingerprint.as_bytes());

    push_u32(
        bytes,
        u32::try_from(installed.functions.len())
            .map_err(|_| TerminalInstallationError::TooManyNativeFuelFunctions)?,
    );
    for function in &installed.functions {
        push_u64(bytes, function.machine.get());
        for value in [
            function.source_text_offset,
            function.source_byte_count,
            function.metered_text_offset,
            function.metered_byte_count,
            function.metered_semantic_end_offset,
        ] {
            push_u64(
                bytes,
                u64::try_from(value)
                    .map_err(|_| TerminalInstallationError::NativeFuelOffsetNotRepresentable)?,
            );
        }
    }

    push_u32(
        bytes,
        u32::try_from(installed.charges.len())
            .map_err(|_| TerminalInstallationError::TooManyNativeFuelCharges)?,
    );
    for charge in &installed.charges {
        encode_charge(bytes, charge)?;
    }
    Ok(())
}

pub(super) fn decode_native_fuel(
    reader: &mut Reader<'_>,
    target: NativeTarget,
) -> Result<Option<TerminalInstalledNativeFuel>, TerminalInstallationError> {
    let present = decode_boolean(reader.u8()?)?;
    if reader.take(3)? != [0; 3] {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    if !present {
        return Ok(None);
    }

    let profile = decode_profile(reader.u8()?)?;
    let transport = decode_transport(reader.u8()?)?;
    if reader.take(2)? != [0; 2] {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    let context = NativeFuelContextLayout {
        byte_size: reader.u32()?,
        alignment: reader.u32()?,
        remaining_units_offset: reader.u32()?,
        unpaid_site_kind_offset: reader.u32()?,
        unpaid_site_identity_offset: reader.u32()?,
        required_units_offset: reader.u32()?,
        transfer_entry_offset: reader.u32()?,
        retry_code_offset_offset: reader.u32()?,
        sponsor_stack_top_offset: reader.u32()?,
        activation_state_offset: reader.u32()?,
        activation_state_byte_count: reader.u32()?,
    };
    let target_policy = NativeFuelTargetPlanProjection {
        profile,
        target,
        transport,
        context,
        transfer_plan_identity: reader.u64()?,
    };
    let source_text_fingerprint = TerminalNativeFuelSourceFingerprint(reader.array()?);

    let function_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyNativeFuelFunctions)?;
    if function_count > reader.remaining() / 48 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut functions = Vec::with_capacity(function_count);
    for _ in 0..function_count {
        functions.push(TerminalInstalledNativeFuelFunction {
            machine: MachineId::new(reader.u64()?).ok_or(
                TerminalInstallationError::ZeroNativeFuelIdentity("MachineId"),
            )?,
            source_text_offset: decode_offset(reader)?,
            source_byte_count: decode_offset(reader)?,
            metered_text_offset: decode_offset(reader)?,
            metered_byte_count: decode_offset(reader)?,
            metered_semantic_end_offset: decode_offset(reader)?,
        });
    }

    let charge_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyNativeFuelCharges)?;
    if charge_count > reader.remaining() / 96 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut charges = Vec::with_capacity(charge_count);
    for _ in 0..charge_count {
        charges.push(decode_charge(reader)?);
    }
    Ok(Some(TerminalInstalledNativeFuel {
        target_policy,
        source_text_fingerprint,
        functions,
        charges,
    }))
}

fn encode_charge(
    bytes: &mut Vec<u8>,
    charge: &TerminalNativeFuelChargeEvidence,
) -> Result<(), TerminalInstallationError> {
    let attribution = charge.attribution;
    push_u64(bytes, attribution.machine.get());
    push_u32(bytes, attribution.schedule.marker());
    match attribution.site {
        TerminalFuelAttributionSite::Operation(operation) => {
            bytes.push(1);
            bytes.extend_from_slice(&[0; 3]);
            push_u64(bytes, operation.get());
        }
        TerminalFuelAttributionSite::Edge(edge) => {
            bytes.push(2);
            bytes.extend_from_slice(&[0; 3]);
            push_u64(bytes, edge.get());
        }
    }
    push_u64(bytes, attribution.units);
    for value in [
        attribution.operation_ordinal,
        attribution.text_offset,
        attribution.byte_count,
        charge.charge_text_offset,
        charge.charge_byte_count,
        charge.semantic_text_offset,
        charge.cold_dispatch_text_offset,
        charge.cold_dispatch_byte_count,
    ] {
        push_u64(
            bytes,
            u64::try_from(value)
                .map_err(|_| TerminalInstallationError::NativeFuelOffsetNotRepresentable)?,
        );
    }
    Ok(())
}

fn decode_charge(
    reader: &mut Reader<'_>,
) -> Result<TerminalNativeFuelChargeEvidence, TerminalInstallationError> {
    let machine = MachineId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroNativeFuelIdentity("MachineId"),
    )?;
    let schedule = FuelScheduleIdentity::new(reader.u32()?)
        .ok_or(TerminalInstallationError::ZeroFuelScheduleIdentity)?;
    let site_tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    let identity = reader.u64()?;
    let site = match site_tag {
        1 => TerminalFuelAttributionSite::Operation(OperationId::new(identity).ok_or(
            TerminalInstallationError::ZeroNativeFuelIdentity("OperationId"),
        )?),
        2 => TerminalFuelAttributionSite::Edge(
            EdgeId::new(identity)
                .ok_or(TerminalInstallationError::ZeroNativeFuelIdentity("EdgeId"))?,
        ),
        _ => {
            return Err(TerminalInstallationError::InvalidNativeFuelSiteTag(
                site_tag,
            ));
        }
    };
    let units = reader.u64()?;
    Ok(TerminalNativeFuelChargeEvidence {
        attribution: TerminalFuelAttributionEvidence {
            machine,
            schedule,
            site,
            units,
            operation_ordinal: decode_offset(reader)?,
            text_offset: decode_offset(reader)?,
            byte_count: decode_offset(reader)?,
        },
        charge_text_offset: decode_offset(reader)?,
        charge_byte_count: decode_offset(reader)?,
        semantic_text_offset: decode_offset(reader)?,
        cold_dispatch_text_offset: decode_offset(reader)?,
        cold_dispatch_byte_count: decode_offset(reader)?,
    })
}

fn decode_offset(reader: &mut Reader<'_>) -> Result<usize, TerminalInstallationError> {
    usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::NativeFuelOffsetNotRepresentable)
}

fn profile_tag(profile: TargetProfile) -> u8 {
    match profile {
        TargetProfile::LinuxArm64 => 1,
        TargetProfile::LinuxX64 => 2,
        TargetProfile::MacosArm64 => 3,
        TargetProfile::WindowsX64 => 4,
        TargetProfile::UefiX64 => 5,
        TargetProfile::CrossPlatformCli => 6,
        TargetProfile::LocalUnchecked => 7,
    }
}

fn decode_profile(tag: u8) -> Result<TargetProfile, TerminalInstallationError> {
    match tag {
        1 => Ok(TargetProfile::LinuxArm64),
        2 => Ok(TargetProfile::LinuxX64),
        3 => Ok(TargetProfile::MacosArm64),
        4 => Ok(TargetProfile::WindowsX64),
        5 => Ok(TargetProfile::UefiX64),
        6 => Ok(TargetProfile::CrossPlatformCli),
        7 => Ok(TargetProfile::LocalUnchecked),
        _ => Err(TerminalInstallationError::InvalidNativeFuelProfileTag(tag)),
    }
}

fn transport_tag(transport: SponsorContextTransport) -> u8 {
    match transport {
        SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::X86Rbx,
        } => 1,
        SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::Aarch64X(28),
        } => 2,
        SponsorContextTransport::ReservedNonvolatileRegister { .. } => 0,
    }
}

fn decode_transport(tag: u8) -> Result<SponsorContextTransport, TerminalInstallationError> {
    match tag {
        1 => Ok(SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::X86Rbx,
        }),
        2 => Ok(SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::Aarch64X(28),
        }),
        _ => Err(TerminalInstallationError::InvalidNativeFuelTransportTag(
            tag,
        )),
    }
}
