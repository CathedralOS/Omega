//! Exact x86-64 hot-path charge encoding for native logical fuel.
//!
//! RBX owns the sponsor-context pointer. R10/R11 are terminal-emitter scratch;
//! the sequence touches no activation-stack memory. The failure target is a
//! later cold thunk and the supplied distance is measured from the end of the
//! conditional branch.

use omega_calling_conventions::{MachineRegister, RegisterSet};
use omega_target::Architecture;
use omega_terminal_installation_evidence::{
    NativeFuelTargetPlanProjection, SponsorContextTransport,
};
use psi_diagnostics::Diagnostic;

use crate::{Reg64, append_jcc_rel32, append_mov_reg_imm64, disp32};

pub const X86_NATIVE_FUEL_CHARGE_BYTE_COUNT: usize = 36;

pub fn native_fuel_charge_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R11])
}

/// Encode:
///
/// ```text
/// mov r10, [rbx + remaining]
/// mov r11, required
/// cmp r10, r11
/// jb  cold_failure
/// sub r10, r11
/// mov [rbx + remaining], r10
/// ```
///
/// `jb` is unsigned strict-less-than, so equality is payable and stores zero.
pub fn encode_native_fuel_charge(
    plan: &NativeFuelTargetPlanProjection,
    required_units: u64,
    cold_failure_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    if plan.target.architecture != Architecture::X86_64
        || !matches!(
            plan.transport,
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx
            }
        )
    {
        return Err(Diagnostic::error(
            "x86-64 native fuel charging requires the admitted RBX context transport",
        ));
    }
    if plan.profile.native_target() != plan.target {
        return Err(Diagnostic::error(
            "x86-64 native fuel charging rejects target-profile drift",
        ));
    }
    if required_units == 0 {
        return Err(Diagnostic::error(
            "native fuel charge requires a nonzero logical-unit cost",
        ));
    }
    let remaining = disp32(plan.context.remaining_units_offset as usize)?;

    let mut bytes = Vec::with_capacity(X86_NATIVE_FUEL_CHARGE_BYTE_COUNT);
    bytes.extend([0x4c, 0x8b, 0x93]); // mov r10, [rbx + disp32]
    bytes.extend(remaining.to_le_bytes());
    append_mov_reg_imm64(&mut bytes, Reg64::R11, required_units);
    bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
    append_jcc_rel32(&mut bytes, 0x82, cold_failure_distance)?; // jb
    bytes.extend([0x4d, 0x29, 0xda]); // sub r10, r11
    bytes.extend([0x4c, 0x89, 0x93]); // mov [rbx + disp32], r10
    bytes.extend(remaining.to_le_bytes());
    debug_assert_eq!(bytes.len(), X86_NATIVE_FUEL_CHARGE_BYTE_COUNT);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_target::TargetProfile;
    use omega_terminal_installation_evidence::NativeFuelContextLayout;

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
                retry_address_offset: 64,
                sponsor_stack_top_offset: 72,
                activation_state_offset: 80,
                activation_state_byte_count: 176,
            },
            transfer_plan_identity: 1,
        }
    }

    #[test]
    fn charge_is_unsigned_compare_before_subtract_and_store() {
        let bytes = encode_native_fuel_charge(&plan(), u64::MAX, -36).expect("exact charge");
        let mut expected = vec![0x4c, 0x8b, 0x93];
        expected.extend(24_i32.to_le_bytes());
        expected.extend([0x49, 0xbb]);
        expected.extend(u64::MAX.to_le_bytes());
        expected.extend([0x4d, 0x39, 0xda]);
        expected.extend([0x0f, 0x82]);
        expected.extend((-36_i32).to_le_bytes());
        expected.extend([0x4d, 0x29, 0xda]);
        expected.extend([0x4c, 0x89, 0x93]);
        expected.extend(24_i32.to_le_bytes());
        assert_eq!(bytes, expected);
        assert_eq!(bytes.len(), X86_NATIVE_FUEL_CHARGE_BYTE_COUNT);
        assert_eq!(
            native_fuel_charge_clobbers().as_slice(),
            &[MachineRegister::X86R10, MachineRegister::X86R11]
        );
    }

    #[test]
    fn charge_rejects_zero_cost_wrong_transport_and_large_offsets() {
        assert!(encode_native_fuel_charge(&plan(), 0, 0).is_err());

        let mut wrong_transport = plan();
        wrong_transport.transport = SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::X86R15,
        };
        assert!(encode_native_fuel_charge(&wrong_transport, 1, 0).is_err());

        let mut large_offset = plan();
        large_offset.context.remaining_units_offset = u32::MAX;
        assert!(encode_native_fuel_charge(&large_offset, 1, 0).is_err());
    }
}
