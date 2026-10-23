//! The register facts each target declares: its physical register model,
//! constraint catalog and conservative baseline reservation profile. The
//! selected constraint-key roster is not re-declared here: its ISA owner
//! carries the one roster per target (see `selected_constraint_keys`).

use isa_aarch64::{
    AARCH64_AAPCS64_CALL, AARCH64_DARWIN_CALL, aarch64_physical_register_model,
    aarch64_register_constraint_catalog,
};
use isa_x86_64::{
    X86_64_MICROSOFT_CALL, X86_64_SYSTEM_V_CALL, x86_64_physical_register_model,
    x86_64_register_constraint_catalog,
};
use register_model::{
    PhysicalRegisterModel, RegisterConstraintCatalog, RegisterConstraintKey,
    RegisterReservationProfile, TargetRegisterEnvironmentConstraintKeys,
    ValidatedPhysicalRegisterModel,
};
use selected_instructions::SelectedConstraintKeys;
use target::{Architecture, NativeTarget, ObjectFormat};

pub(super) fn target_physical_register_model(target: NativeTarget) -> PhysicalRegisterModel {
    match target.architecture {
        Architecture::X86_64 => x86_64_physical_register_model(),
        Architecture::Aarch64 => aarch64_physical_register_model(),
    }
}

pub(super) fn target_constraint_catalog(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
) -> RegisterConstraintCatalog {
    match target.architecture {
        Architecture::X86_64 => x86_64_register_constraint_catalog(physical),
        Architecture::Aarch64 => aarch64_register_constraint_catalog(physical),
    }
}

pub(super) fn conservative_baseline_reservation_profile(
    target: NativeTarget,
    physical: &PhysicalRegisterModel,
) -> RegisterReservationProfile {
    let mut active_overlays = physical
        .reservations
        .iter()
        .filter(|overlay| {
            overlay.name != "darwin.aarch64.platform" || target.object_format == ObjectFormat::MachO
        })
        .map(|overlay| overlay.name.clone())
        .collect::<Vec<_>>();
    active_overlays.sort();
    RegisterReservationProfile {
        name: "omega.conservative-baseline-v1".into(),
        active_overlays,
    }
}

pub(super) fn selected_environment_keys(
    keys: SelectedConstraintKeys,
) -> TargetRegisterEnvironmentConstraintKeys {
    TargetRegisterEnvironmentConstraintKeys {
        port_write: keys.port_write,
        system_register_read: keys.system_register_read,
        system_register_write: keys.system_register_write,
        control_register_read: keys.control_register_read,
        control_register_write: keys.control_register_write,
        port_register_read: keys.port_register_read,
        port_register_write: keys.port_register_write,
        model_specific_read: keys.model_specific_read,
        model_specific_write: keys.model_specific_write,
        processor_halt: keys.processor_halt,
        interrupt_control: keys.interrupt_control,
        atomic_fence: keys.atomic_fence,
        placed_read: keys.placed_read,
        placed_write: keys.placed_write,
        atomic_load: keys.atomic_load,
        atomic_store: keys.atomic_store,
        atomic_update: keys.atomic_update,
        atomic_bitwise_update: keys.atomic_bitwise_update,
        atomic_compare_exchange: keys.atomic_compare_exchange,
        crash: keys.crash,
        load64: keys.load64,
        load8: keys.load8,
        load16: keys.load16,
        load32: keys.load32,
        load_packed: keys.load_packed,
        store_packed: keys.store_packed,
        load8_indexed: keys.load8_indexed,
        copy_bytes: keys.copy_bytes,
        zero_bytes: keys.zero_bytes,
        scan_byte_sequence: keys.scan_byte_sequence,
        load_encoded_scalar: keys.load_encoded_scalar,
        store_encoded_scalar: keys.store_encoded_scalar,
        compare_byte_sequences: keys.compare_byte_sequences,
        store: keys.store,
        address_offset: keys.address_offset,
        store64: keys.store64,
        frame_address: keys.frame_address,
        byte_literal_image_address: keys.byte_literal_image_address,
        hosted_read_byte: keys.hosted_read_byte,
        hosted_write_byte_i32: keys.hosted_write_byte_i32,
        save_floating_control: keys.save_floating_control,
        restore_floating_control: keys.restore_floating_control,
        hosted_exit_process_i32: keys.hosted_exit_process_i32,
        call_unit: keys.call_unit,
        call_scalar: keys.call_scalar,
        call_aggregate: keys.call_aggregate,
        call_normalized_foreign: keys.call_normalized_foreign,
        return_aggregate: keys.return_aggregate,
        materialize_i64: keys.materialize_i64,
        materialize_boolean: keys.materialize_boolean,
        copy_i64: keys.copy_i64,
        float32_to_bits: keys.float32_to_bits,
        float64_to_bits: keys.float64_to_bits,
        bits_to_float32: keys.bits_to_float32,
        bits_to_float64: keys.bits_to_float64,
        ieee_float_arithmetic: keys.ieee_float_arithmetic,
        ieee_float_directed_binary: keys.ieee_float_directed_binary,
        ieee_float_directed_unary: keys.ieee_float_directed_unary,
        ieee_float_unary: keys.ieee_float_unary,
        ieee_float_conversion_control: keys.ieee_float_conversion_control,
        ieee_float_ternary: keys.ieee_float_ternary,
        call_unit_mixed: keys.call_unit_mixed,
        add_i64: keys.add_i64,
        add_i64_immediate: keys.add_i64_immediate,
        subtract_i64: keys.subtract_i64,
        multiply_i64: keys.multiply_i64,
        saturating_subtract_unsigned: keys.saturating_subtract_unsigned,
        saturating_add_u64: keys.saturating_add_u64,
        divide_u64: keys.divide_u64,
        remainder_u64: keys.remainder_u64,
        remainder_i64: keys.remainder_i64,
        divide_i64: keys.divide_i64,
        shift_i64: keys.shift_i64,
        saturating_add_clamped: keys.saturating_add_clamped,
        saturating_subtract_clamped: keys.saturating_subtract_clamped,
        saturating_divide_signed: keys.saturating_divide_signed,
        saturating_multiply_clamped: keys.saturating_multiply_clamped,
        saturating_multiply_u64: keys.saturating_multiply_u64,
        trapping_binary: keys.trapping_binary,
        trapping_fixed_pair: keys.trapping_fixed_pair,
        trapping_shift: keys.trapping_shift,
        trapping_convert: keys.trapping_convert,
        float_result_trapping: keys.float_result_trapping,
        float_result_saturating: keys.float_result_saturating,
        subtract_i64_immediate: keys.subtract_i64_immediate,
        compare_i64_zero: keys.compare_i64_zero,
        compare_i64: keys.compare_i64,
        compare_i64_immediate: keys.compare_i64_immediate,
        conditional_branch: keys.conditional_branch,
        jump: keys.jump,
        return_float: keys.return_float,
        return_i64: keys.return_i64,
        return_unit: keys.return_unit,
    }
}

/// The target's selected constraint-key roster, resolved by its ISA owner.
///
/// One roster exists per target: the machine-effect catalog and this
/// environment consume the same ISA declaration, so the allocator cannot omit
/// a key the target declares. While this module re-derived the roster itself,
/// the Windows x86-64 arm silently dropped four port-register and
/// model-specific keys the ISA declared and ordinary programs were rejected
/// with `CatalogSelectedKeysMismatch`.
///
/// `None` means the declared (architecture, object format) pair has no ABI for
/// that ISA; the join reports it as `UnsupportedSelectedInstructionAbi`.
pub(super) fn selected_constraint_keys(target: NativeTarget) -> Option<SelectedConstraintKeys> {
    match target.architecture {
        Architecture::X86_64 => isa_x86_64::x86_64_selected_constraint_keys(target),
        Architecture::Aarch64 => isa_aarch64::aarch64_selected_constraint_keys(target),
    }
}

#[test]
fn float_result_policy_keys_survive_the_hosted_environment_projection() {
    for target in [
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
    ] {
        let (trapping, saturating) = match target.architecture {
            Architecture::Aarch64 => (
                isa_aarch64::AARCH64_FLOAT_RESULT_TRAPPING,
                isa_aarch64::AARCH64_FLOAT_RESULT_SATURATING,
            ),
            Architecture::X86_64 => (
                isa_x86_64::X86_64_FLOAT_RESULT_TRAPPING,
                isa_x86_64::X86_64_FLOAT_RESULT_SATURATING,
            ),
        };
        let keys = selected_constraint_keys(target).unwrap();
        assert_eq!(keys.float_result_trapping, Some(trapping));
        assert_eq!(keys.float_result_saturating, Some(saturating));
        let environment_keys = selected_environment_keys(keys);
        assert_eq!(environment_keys.float_result_trapping, Some(trapping));
        assert_eq!(environment_keys.float_result_saturating, Some(saturating));
    }
}

/// Exact scalar-call ABI row selected by one native target. General selected
/// call lowering is not implemented yet; this mapping makes the future entry
/// explicit without adding call authority to the current selected CFG.
pub(super) const fn scalar_call_constraint_key(
    target: NativeTarget,
) -> Option<RegisterConstraintKey> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf | ObjectFormat::MachO) => {
            Some(X86_64_SYSTEM_V_CALL)
        }
        (Architecture::X86_64, ObjectFormat::Coff) => Some(X86_64_MICROSOFT_CALL),
        (Architecture::Aarch64, ObjectFormat::Elf) => Some(AARCH64_AAPCS64_CALL),
        (Architecture::Aarch64, ObjectFormat::MachO) => Some(AARCH64_DARWIN_CALL),
        _ => None,
    }
}
