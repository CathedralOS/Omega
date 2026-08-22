//! Exact x86-64 hot-path charge encoding for native logical fuel.
//!
//! RBX owns the sponsor-context pointer. R10/R11 are terminal-emitter scratch;
//! the sequence touches no activation-stack memory. The failure target is a
//! later cold thunk and the supplied distance is measured from the end of the
//! conditional branch.

use omega_calling_conventions::{MachineRegister, RegisterSet};
use omega_target::Architecture;
use omega_terminal_installation_evidence::{
    NativeFuelTargetPlanProjection, SponsorContextTransport, TerminalFuelAttributionSite,
};
use psi_diagnostics::Diagnostic;

use crate::{Reg64, append_jcc_rel32, append_mov_reg_imm64, disp32};

pub const X86_NATIVE_FUEL_CHARGE_BYTE_COUNT: usize = 36;
pub const X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT: usize = 78;

pub fn native_fuel_charge_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R11])
}

pub fn native_fuel_cold_dispatch_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R10])
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
    validate_x86_plan(plan)?;
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

/// Record one exact unpaid semantic site and tail-jump through the admitted
/// transfer entry. The retry value is a code offset interpreted only by the
/// validated transfer plan; this emits no source-visible/raw continuation.
pub fn encode_native_fuel_cold_dispatch(
    plan: &NativeFuelTargetPlanProjection,
    site: TerminalFuelAttributionSite,
    required_units: u64,
    retry_code_offset: u64,
) -> Result<Vec<u8>, Diagnostic> {
    validate_x86_plan(plan)?;
    if required_units == 0 {
        return Err(Diagnostic::error(
            "native fuel cold dispatch requires a nonzero logical-unit cost",
        ));
    }
    let (site_kind, site_identity) = match site {
        TerminalFuelAttributionSite::Operation(operation) => (0, operation.get()),
        TerminalFuelAttributionSite::Edge(edge) => (1, edge.get()),
    };
    let mut bytes = Vec::with_capacity(X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);
    append_context_u64_store(&mut bytes, plan.context.unpaid_site_kind_offset, site_kind)?;
    append_context_u64_store(
        &mut bytes,
        plan.context.unpaid_site_identity_offset,
        site_identity,
    )?;
    append_context_u64_store(
        &mut bytes,
        plan.context.required_units_offset,
        required_units,
    )?;
    append_context_u64_store(
        &mut bytes,
        plan.context.retry_code_offset_offset,
        retry_code_offset,
    )?;
    let transfer_entry = disp32(plan.context.transfer_entry_offset as usize)?;
    bytes.extend([0x4c, 0x8b, 0x93]); // mov r10, [rbx + disp32]
    bytes.extend(transfer_entry.to_le_bytes());
    bytes.extend([0x41, 0xff, 0xe2]); // jmp r10
    debug_assert_eq!(bytes.len(), X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);
    Ok(bytes)
}

fn validate_x86_plan(plan: &NativeFuelTargetPlanProjection) -> Result<(), Diagnostic> {
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
    Ok(())
}

fn append_context_u64_store(
    bytes: &mut Vec<u8>,
    byte_offset: u32,
    value: u64,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset as usize)?;
    append_mov_reg_imm64(bytes, Reg64::R10, value);
    bytes.extend([0x4c, 0x89, 0x93]); // mov [rbx + disp32], r10
    bytes.extend(displacement.to_le_bytes());
    Ok(())
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
                retry_code_offset_offset: 64,
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

    #[test]
    fn cold_dispatch_records_exact_site_and_tail_jumps_without_stack_access() {
        let operation = psi_core::OperationId::new(9).unwrap();
        let bytes = encode_native_fuel_cold_dispatch(
            &plan(),
            TerminalFuelAttributionSite::Operation(operation),
            u64::MAX,
            0x1020,
        )
        .expect("exact cold dispatch");
        assert_eq!(bytes.len(), X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);
        assert_eq!(&bytes[0..2], &[0x49, 0xba]); // mov r10, site kind
        assert_eq!(&bytes[2..10], &0_u64.to_le_bytes());
        assert_eq!(&bytes[17..19], &[0x49, 0xba]); // mov r10, site identity
        assert_eq!(&bytes[19..27], &9_u64.to_le_bytes());
        assert_eq!(&bytes[36..44], &u64::MAX.to_le_bytes());
        assert_eq!(&bytes[53..61], &0x1020_u64.to_le_bytes());
        assert_eq!(&bytes[68..71], &[0x4c, 0x8b, 0x93]);
        assert_eq!(&bytes[75..], &[0x41, 0xff, 0xe2]);
        assert_eq!(
            native_fuel_cold_dispatch_clobbers().as_slice(),
            &[MachineRegister::X86R10]
        );

        let edge = encode_native_fuel_cold_dispatch(
            &plan(),
            TerminalFuelAttributionSite::Edge(psi_core::EdgeId::new(9).unwrap()),
            1,
            0,
        )
        .unwrap();
        assert_eq!(&edge[2..10], &1_u64.to_le_bytes());
    }
}
