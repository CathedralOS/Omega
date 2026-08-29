//! Independent decoding of AArch64 native-fuel hot and cold fragments.

use omega_calling_conventions::MachineRegister;
use omega_installation_evidence::{
    FuelAttributionSite, NativeFuelTargetPlanProjection, SponsorContextTransport,
};
use omega_target::Architecture;

use crate::{AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT, AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64NativeFuelValidationError {
    TargetPolicy,
    WrongByteCount,
    MalformedInstruction,
    ValueMismatch,
    BranchTargetMismatch,
    CoordinateOverflow,
}

impl std::fmt::Display for Aarch64NativeFuelValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid AArch64 native-fuel bytes: {self:?}")
    }
}

impl std::error::Error for Aarch64NativeFuelValidationError {}

pub fn validate_aarch64_native_fuel_charge(
    bytes: &[u8],
    plan: &NativeFuelTargetPlanProjection,
    required_units: u64,
    charge_code_offset: usize,
    cold_dispatch_code_offset: usize,
) -> Result<(), Aarch64NativeFuelValidationError> {
    use Aarch64NativeFuelValidationError as Error;
    validate_plan(plan)?;
    if bytes.len() != AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT {
        return Err(Error::WrongByteCount);
    }
    let words = words(bytes)?;
    let remaining = plan.context.remaining_units_offset as usize;
    let expected_load = load_x_from_x(16, 28, remaining).ok_or(Error::ValueMismatch)?;
    let expected_store = store_x_to_x(16, 28, remaining).ok_or(Error::ValueMismatch)?;
    if required_units == 0
        || words[0] != expected_load
        || words[1] != movz(17, halfword(required_units, 0))
        || words[2] != movk(17, halfword(required_units, 1), 1)
        || words[3] != movk(17, halfword(required_units, 2), 2)
        || words[4] != movk(17, halfword(required_units, 3), 3)
        || words[5] != 0xeb11_021f
        || words[7] != 0xcb11_0210
        || words[8] != expected_store
    {
        return Err(Error::MalformedInstruction);
    }
    let branch = words[6];
    if branch & 0xff00_001f != 0x5400_0003 {
        return Err(Error::MalformedInstruction);
    }
    let immediate = ((branch >> 5) & 0x7ffff) as i32;
    let signed_words = (immediate << 13) >> 13;
    let branch_origin = i64::try_from(
        charge_code_offset
            .checked_add(24)
            .ok_or(Error::CoordinateOverflow)?,
    )
    .map_err(|_| Error::CoordinateOverflow)?;
    let actual_target = branch_origin.checked_add(i64::from(signed_words) * 4);
    if actual_target
        != Some(i64::try_from(cold_dispatch_code_offset).map_err(|_| Error::CoordinateOverflow)?)
    {
        return Err(Error::BranchTargetMismatch);
    }
    Ok(())
}

pub fn validate_aarch64_native_fuel_cold_dispatch(
    bytes: &[u8],
    plan: &NativeFuelTargetPlanProjection,
    site: FuelAttributionSite,
    required_units: u64,
    retry_text_offset: u64,
) -> Result<(), Aarch64NativeFuelValidationError> {
    use Aarch64NativeFuelValidationError as Error;
    validate_plan(plan)?;
    if bytes.len() != AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT {
        return Err(Error::WrongByteCount);
    }
    let words = words(bytes)?;
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
        let start = ordinal * 5;
        if words[start] != movz(16, halfword(value, 0))
            || words[start + 1] != movk(16, halfword(value, 1), 1)
            || words[start + 2] != movk(16, halfword(value, 2), 2)
            || words[start + 3] != movk(16, halfword(value, 3), 3)
            || words[start + 4]
                != store_x_to_x(16, 28, offset as usize).ok_or(Error::ValueMismatch)?
        {
            return Err(Error::ValueMismatch);
        }
    }
    if required_units == 0
        || words[20]
            != load_x_from_x(16, 28, plan.context.transfer_entry_offset as usize)
                .ok_or(Error::ValueMismatch)?
        || words[21] != 0xd61f_0200
    {
        return Err(Error::MalformedInstruction);
    }
    Ok(())
}

fn validate_plan(
    plan: &NativeFuelTargetPlanProjection,
) -> Result<(), Aarch64NativeFuelValidationError> {
    if plan.target.architecture != Architecture::Aarch64
        || plan.profile.native_target() != plan.target
        || !matches!(
            plan.transport,
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::Aarch64X(28),
            }
        )
    {
        return Err(Aarch64NativeFuelValidationError::TargetPolicy);
    }
    Ok(())
}

fn words(bytes: &[u8]) -> Result<Vec<u32>, Aarch64NativeFuelValidationError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(Aarch64NativeFuelValidationError::WrongByteCount);
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("AArch64 word")))
        .collect())
}

const fn movz(register: u8, immediate: u16) -> u32 {
    0xd280_0000 | ((immediate as u32) << 5) | register as u32
}

const fn movk(register: u8, immediate: u16, halfword_shift: u8) -> u32 {
    0xf280_0000 | ((halfword_shift as u32) << 21) | ((immediate as u32) << 5) | register as u32
}

const fn halfword(value: u64, shift: u8) -> u16 {
    ((value >> (shift as u64 * 16)) & 0xffff) as u16
}

fn store_x_to_x(source: u8, base: u8, byte_offset: usize) -> Option<u32> {
    (byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095).then_some(
        0xf900_0000
            | (((byte_offset / 8) as u32) << 10)
            | (u32::from(base) << 5)
            | u32::from(source),
    )
}

fn load_x_from_x(destination: u8, base: u8, byte_offset: usize) -> Option<u32> {
    if byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095 {
        Some(
            0xf940_0000
                | (((byte_offset / 8) as u32) << 10)
                | (u32::from(base) << 5)
                | u32::from(destination),
        )
    } else if byte_offset <= 255 {
        Some(
            0xf840_0000
                | ((byte_offset as u32) << 12)
                | (u32::from(base) << 5)
                | u32::from(destination),
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_installation_evidence::NativeFuelContextLayout;
    use omega_target::TargetProfile;
    use psi_core::OperationId;

    fn plan() -> NativeFuelTargetPlanProjection {
        NativeFuelTargetPlanProjection {
            profile: TargetProfile::LinuxArm64,
            target: TargetProfile::LinuxArm64.native_target(),
            transport: SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::Aarch64X(28),
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
        let charge_offset = 40;
        let cold_offset = 348;
        let distance = cold_offset as isize - (charge_offset + 24) as isize;
        let charge = crate::encode_native_fuel_charge(&plan, 7, distance).unwrap();
        validate_aarch64_native_fuel_charge(&charge, &plan, 7, charge_offset, cold_offset).unwrap();
        for index in 0..charge.len() {
            let mut corrupted = charge.clone();
            corrupted[index] ^= 1;
            assert!(
                validate_aarch64_native_fuel_charge(
                    &corrupted,
                    &plan,
                    7,
                    charge_offset,
                    cold_offset,
                )
                .is_err(),
                "charge byte {index} escaped replay"
            );
        }

        let site = FuelAttributionSite::Operation(OperationId::new(9).unwrap());
        let cold = crate::encode_native_fuel_cold_dispatch(&plan, site, 7, 40).unwrap();
        validate_aarch64_native_fuel_cold_dispatch(&cold, &plan, site, 7, 40).unwrap();
        for index in 0..cold.len() {
            let mut corrupted = cold.clone();
            corrupted[index] ^= 1;
            assert!(
                validate_aarch64_native_fuel_cold_dispatch(&corrupted, &plan, site, 7, 40).is_err(),
                "cold-dispatch byte {index} escaped replay"
            );
        }
    }
}
