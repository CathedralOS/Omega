//! Exact AArch64 hot-path charge encoding for native logical fuel.
//!
//! X28 owns the sponsor-context pointer. X16/X17 are terminal-emitter scratch;
//! the sequence touches no activation-stack memory. The failure target is a
//! later cold thunk and the supplied distance is measured from the conditional
//! branch instruction itself, following AArch64 PC-relative branch semantics.

use omega_calling_conventions::{MachineRegister, RegisterSet};
use omega_target::Architecture;
use omega_terminal_installation_evidence::{
    NativeFuelTargetPlanProjection, SponsorContextTransport, TerminalFuelAttributionSite,
};
use psi_diagnostics::Diagnostic;

use super::{
    append_unsigned_immediate_padded, encode_compare_x_register, encode_conditional_branch_lower,
    encode_load_x_from_x, encode_store_x_to_x, encode_sub_x_register,
};

pub const AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT: usize = 36;
/// Offset from charge start to the `B.LO` instruction address.
pub const AARCH64_NATIVE_FUEL_FAILURE_BRANCH_OFFSET: usize = 24;
pub const AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT: usize = 88;

pub fn native_fuel_charge_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)])
}

pub fn native_fuel_cold_dispatch_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::Aarch64X(16)])
}

/// Encode `LDR remaining; materialize required; CMP; B.LO failure; SUB; STR`.
/// `B.LO` is unsigned strict-less-than, so equality is payable and stores zero.
pub fn encode_native_fuel_charge(
    plan: &NativeFuelTargetPlanProjection,
    required_units: u64,
    cold_failure_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    validate_aarch64_plan(plan)?;
    if required_units == 0 {
        return Err(Diagnostic::error(
            "native fuel charge requires a nonzero logical-unit cost",
        ));
    }
    let remaining = plan.context.remaining_units_offset as usize;

    let mut bytes = Vec::with_capacity(AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT);
    bytes.extend(encode_load_x_from_x(16, 28, remaining)?);
    append_unsigned_immediate_padded(&mut bytes, 17, required_units);
    bytes.extend(encode_compare_x_register(16, 17));
    bytes.extend(encode_conditional_branch_lower(cold_failure_distance)?);
    bytes.extend(encode_sub_x_register(16, 16, 17));
    bytes.extend(encode_store_x_to_x(16, 28, remaining)?);
    debug_assert_eq!(bytes.len(), AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT);
    Ok(bytes)
}

/// Record one exact unpaid semantic site and tail-branch through the admitted
/// transfer entry. The retry value is a code offset interpreted only by the
/// validated transfer plan; this emits no source-visible/raw continuation.
pub fn encode_native_fuel_cold_dispatch(
    plan: &NativeFuelTargetPlanProjection,
    site: TerminalFuelAttributionSite,
    required_units: u64,
    retry_code_offset: u64,
) -> Result<Vec<u8>, Diagnostic> {
    validate_aarch64_plan(plan)?;
    if required_units == 0 {
        return Err(Diagnostic::error(
            "native fuel cold dispatch requires a nonzero logical-unit cost",
        ));
    }
    let (site_kind, site_identity) = match site {
        TerminalFuelAttributionSite::Operation(operation) => (0, operation.get()),
        TerminalFuelAttributionSite::Edge(edge) => (1, edge.get()),
    };
    let mut bytes = Vec::with_capacity(AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);
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
    bytes.extend(encode_load_x_from_x(
        16,
        28,
        plan.context.transfer_entry_offset as usize,
    )?);
    bytes.extend((0xD61F0000_u32 | (16_u32 << 5)).to_le_bytes()); // br x16
    debug_assert_eq!(bytes.len(), AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);
    Ok(bytes)
}

fn validate_aarch64_plan(plan: &NativeFuelTargetPlanProjection) -> Result<(), Diagnostic> {
    if plan.target.architecture != Architecture::Aarch64
        || !matches!(
            plan.transport,
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::Aarch64X(28)
            }
        )
    {
        return Err(Diagnostic::error(
            "AArch64 native fuel charging requires the admitted X28 context transport",
        ));
    }
    if plan.profile.native_target() != plan.target {
        return Err(Diagnostic::error(
            "AArch64 native fuel charging rejects target-profile drift",
        ));
    }
    Ok(())
}

fn append_context_u64_store(
    bytes: &mut Vec<u8>,
    byte_offset: u32,
    value: u64,
) -> Result<(), Diagnostic> {
    append_unsigned_immediate_padded(bytes, 16, value);
    bytes.extend(encode_store_x_to_x(16, 28, byte_offset as usize)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_target::TargetProfile;
    use omega_terminal_installation_evidence::NativeFuelContextLayout;

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
            transfer_plan_identity: 1,
        }
    }

    #[test]
    fn charge_is_unsigned_compare_before_subtract_and_store() {
        let bytes = encode_native_fuel_charge(&plan(), u64::MAX, -36).expect("exact charge");
        let mut expected = Vec::new();
        expected.extend(encode_load_x_from_x(16, 28, 24).unwrap());
        append_unsigned_immediate_padded(&mut expected, 17, u64::MAX);
        expected.extend(encode_compare_x_register(16, 17));
        expected.extend(encode_conditional_branch_lower(-36).unwrap());
        expected.extend(encode_sub_x_register(16, 16, 17));
        expected.extend(encode_store_x_to_x(16, 28, 24).unwrap());
        assert_eq!(bytes, expected);
        assert_eq!(bytes.len(), AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT);
        assert_eq!(
            native_fuel_charge_clobbers().as_slice(),
            &[MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)]
        );
    }

    #[test]
    fn charge_rejects_zero_cost_wrong_transport_and_large_offsets() {
        assert!(encode_native_fuel_charge(&plan(), 0, 0).is_err());

        let mut wrong_transport = plan();
        wrong_transport.transport = SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::Aarch64X(27),
        };
        assert!(encode_native_fuel_charge(&wrong_transport, 1, 0).is_err());

        let mut large_offset = plan();
        large_offset.context.remaining_units_offset = u32::MAX;
        assert!(encode_native_fuel_charge(&large_offset, 1, 0).is_err());
    }

    #[test]
    fn cold_dispatch_records_exact_site_and_tail_branches_without_stack_access() {
        let bytes = encode_native_fuel_cold_dispatch(
            &plan(),
            TerminalFuelAttributionSite::Operation(psi_core::OperationId::new(9).unwrap()),
            u64::MAX,
            0x1020,
        )
        .expect("exact cold dispatch");
        assert_eq!(bytes.len(), AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);

        let mut kind = Vec::new();
        append_unsigned_immediate_padded(&mut kind, 16, 0);
        kind.extend(encode_store_x_to_x(16, 28, 32).unwrap());
        assert_eq!(&bytes[0..20], kind);
        let mut identity = Vec::new();
        append_unsigned_immediate_padded(&mut identity, 16, 9);
        identity.extend(encode_store_x_to_x(16, 28, 40).unwrap());
        assert_eq!(&bytes[20..40], identity);
        assert_eq!(&bytes[80..84], &encode_load_x_from_x(16, 28, 56).unwrap());
        assert_eq!(&bytes[84..], &(0xD61F0200_u32).to_le_bytes()); // br x16
        assert_eq!(
            native_fuel_cold_dispatch_clobbers().as_slice(),
            &[MachineRegister::Aarch64X(16)]
        );

        let edge = encode_native_fuel_cold_dispatch(
            &plan(),
            TerminalFuelAttributionSite::Edge(psi_core::EdgeId::new(9).unwrap()),
            1,
            0,
        )
        .unwrap();
        let mut edge_kind = Vec::new();
        append_unsigned_immediate_padded(&mut edge_kind, 16, 1);
        assert_eq!(&edge[0..16], edge_kind);
    }
}
