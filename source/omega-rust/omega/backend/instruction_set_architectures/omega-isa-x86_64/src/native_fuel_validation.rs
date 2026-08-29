//! Independent decoding of x86-64 native-fuel hot and cold fragments.

use omega_calling_conventions::MachineRegister;
use omega_installation_evidence::{
    FuelAttributionSite, NativeFuelTargetPlanProjection, SponsorContextTransport,
};
use omega_target::Architecture;

use crate::{X86_NATIVE_FUEL_CHARGE_BYTE_COUNT, X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86NativeFuelValidationError {
    TargetPolicy,
    WrongByteCount,
    MalformedInstruction,
    ValueMismatch,
    BranchTargetMismatch,
    CoordinateOverflow,
}

impl std::fmt::Display for X86NativeFuelValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid x86-64 native-fuel bytes: {self:?}")
    }
}

impl std::error::Error for X86NativeFuelValidationError {}

pub fn validate_x86_native_fuel_charge(
    bytes: &[u8],
    plan: &NativeFuelTargetPlanProjection,
    required_units: u64,
    charge_code_offset: usize,
    cold_dispatch_code_offset: usize,
) -> Result<(), X86NativeFuelValidationError> {
    use X86NativeFuelValidationError as Error;
    validate_plan(plan)?;
    if bytes.len() != X86_NATIVE_FUEL_CHARGE_BYTE_COUNT {
        return Err(Error::WrongByteCount);
    }
    if bytes[0..3] != [0x4c, 0x8b, 0x93]
        || bytes[7..9] != [0x49, 0xbb]
        || bytes[17..20] != [0x4d, 0x39, 0xda]
        || bytes[20..22] != [0x0f, 0x82]
        || bytes[26..29] != [0x4d, 0x29, 0xda]
        || bytes[29..32] != [0x4c, 0x89, 0x93]
    {
        return Err(Error::MalformedInstruction);
    }
    let expected_remaining =
        i32::try_from(plan.context.remaining_units_offset).map_err(|_| Error::ValueMismatch)?;
    let load_offset = i32::from_le_bytes(bytes[3..7].try_into().expect("disp32"));
    let units = u64::from_le_bytes(bytes[9..17].try_into().expect("imm64"));
    let store_offset = i32::from_le_bytes(bytes[32..36].try_into().expect("disp32"));
    if required_units == 0
        || units != required_units
        || load_offset != expected_remaining
        || store_offset != expected_remaining
    {
        return Err(Error::ValueMismatch);
    }
    let displacement = i32::from_le_bytes(bytes[22..26].try_into().expect("rel32"));
    let branch_end = charge_code_offset
        .checked_add(26)
        .ok_or(Error::CoordinateOverflow)?;
    let actual_target = i128::try_from(branch_end)
        .ok()
        .and_then(|end| end.checked_add(i128::from(displacement)))
        .and_then(|target| usize::try_from(target).ok());
    if actual_target != Some(cold_dispatch_code_offset) {
        return Err(Error::BranchTargetMismatch);
    }
    Ok(())
}

pub fn validate_x86_native_fuel_cold_dispatch(
    bytes: &[u8],
    plan: &NativeFuelTargetPlanProjection,
    site: FuelAttributionSite,
    required_units: u64,
    retry_text_offset: u64,
) -> Result<(), X86NativeFuelValidationError> {
    use X86NativeFuelValidationError as Error;
    validate_plan(plan)?;
    if bytes.len() != X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT {
        return Err(Error::WrongByteCount);
    }
    let (site_kind, site_identity) = match site {
        FuelAttributionSite::Operation(operation) => (0, operation.get()),
        FuelAttributionSite::Edge(edge) => (1, edge.get()),
    };
    for (ordinal, (offset, value)) in [
        (plan.context.unpaid_site_kind_offset, site_kind),
        (plan.context.unpaid_site_identity_offset, site_identity),
        (plan.context.required_units_offset, required_units),
        (plan.context.retry_code_offset_offset, retry_text_offset),
    ]
    .into_iter()
    .enumerate()
    {
        let start = ordinal * 17;
        if bytes[start..start + 2] != [0x49, 0xba]
            || bytes[start + 10..start + 13] != [0x4c, 0x89, 0x93]
        {
            return Err(Error::MalformedInstruction);
        }
        let actual_value =
            u64::from_le_bytes(bytes[start + 2..start + 10].try_into().expect("imm64"));
        let actual_offset =
            i32::from_le_bytes(bytes[start + 13..start + 17].try_into().expect("disp32"));
        if actual_value != value
            || actual_offset != i32::try_from(offset).map_err(|_| Error::ValueMismatch)?
        {
            return Err(Error::ValueMismatch);
        }
    }
    if required_units == 0
        || bytes[68..71] != [0x4c, 0x8b, 0x93]
        || bytes[75..78] != [0x41, 0xff, 0xe2]
    {
        return Err(Error::MalformedInstruction);
    }
    let transfer_offset = i32::from_le_bytes(bytes[71..75].try_into().expect("disp32"));
    if transfer_offset
        != i32::try_from(plan.context.transfer_entry_offset).map_err(|_| Error::ValueMismatch)?
    {
        return Err(Error::ValueMismatch);
    }
    Ok(())
}

fn validate_plan(
    plan: &NativeFuelTargetPlanProjection,
) -> Result<(), X86NativeFuelValidationError> {
    if plan.target.architecture != Architecture::X86_64
        || plan.profile.native_target() != plan.target
        || !matches!(
            plan.transport,
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx,
            }
        )
    {
        return Err(X86NativeFuelValidationError::TargetPolicy);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_installation_evidence::NativeFuelContextLayout;
    use omega_target::TargetProfile;
    use psi_core::OperationId;

    fn plan() -> NativeFuelTargetPlanProjection {
        NativeFuelTargetPlanProjection {
            profile: TargetProfile::LinuxX64,
            target: TargetProfile::LinuxX64.native_target(),
            transport: SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx,
            },
            context: NativeFuelContextLayout {
                byte_size: 256,
                alignment: 16,
                remaining_units_offset: 24,
                unpaid_site_kind_offset: 32,
                unpaid_site_identity_offset: 40,
                required_units_offset: 48,
                transfer_entry_offset: 56,
                retry_code_offset_offset: 64,
                sponsor_stack_top_offset: 72,
                activation_state_offset: 80,
                activation_state_byte_count: 176,
            },
            transfer_plan_report_identity: 1,
            transfer_plan_commitment:
                omega_installation_evidence::NativeFuelTransferPlanCommitment::from_bytes([1; 32]),
        }
    }

    #[test]
    fn decoder_accepts_exact_fragments_and_rejects_every_byte_mutation() {
        let plan = plan();
        let charge_offset = 41;
        let cold_offset = 345;
        let distance = cold_offset as isize - (charge_offset + 26) as isize;
        let charge = crate::encode_native_fuel_charge(&plan, 7, distance).unwrap();
        validate_x86_native_fuel_charge(&charge, &plan, 7, charge_offset, cold_offset).unwrap();
        for index in 0..charge.len() {
            let mut corrupted = charge.clone();
            corrupted[index] ^= 1;
            assert!(
                validate_x86_native_fuel_charge(&corrupted, &plan, 7, charge_offset, cold_offset,)
                    .is_err(),
                "charge byte {index} escaped replay"
            );
        }

        let site = FuelAttributionSite::Operation(OperationId::new(9).unwrap());
        let cold = crate::encode_native_fuel_cold_dispatch(&plan, site, 7, 41).unwrap();
        validate_x86_native_fuel_cold_dispatch(&cold, &plan, site, 7, 41).unwrap();
        for index in 0..cold.len() {
            let mut corrupted = cold.clone();
            corrupted[index] ^= 1;
            assert!(
                validate_x86_native_fuel_cold_dispatch(&corrupted, &plan, site, 7, 41).is_err(),
                "cold-dispatch byte {index} escaped replay"
            );
        }
    }
}
