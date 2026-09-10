use isa_aarch64::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_RETURN, AARCH64_AAPCS64_RETURN_UNIT, AARCH64_ADD_I64,
    AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH,
    AARCH64_COPY_I64, AARCH64_DARWIN_CALL, AARCH64_DARWIN_RETURN, AARCH64_DARWIN_RETURN_UNIT,
    AARCH64_MATERIALIZE_I64, AARCH64_SUBTRACT_I64, AARCH64_SUBTRACT_I64_IMMEDIATE,
    aarch64_physical_register_model, aarch64_register_constraint_catalog,
};
use isa_x86_64::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH,
    X86_64_COPY_I64, X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_RETURN,
    X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64, X86_64_SUBTRACT_I64_IMMEDIATE,
    X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
    x86_64_physical_register_model, x86_64_register_constraint_catalog,
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
        load64: keys.load64,
        load8: keys.load8,
        load16: keys.load16,
        load32: keys.load32,
        load_packed: keys.load_packed,
        store_packed: keys.store_packed,
        load8_indexed: keys.load8_indexed,
        store: keys.store,
        address_offset: keys.address_offset,
        store64: keys.store64,
        frame_address: keys.frame_address,
        hosted_read_byte: keys.hosted_read_byte,
        hosted_write_byte_i32: keys.hosted_write_byte_i32,
        hosted_exit_process_i32: keys.hosted_exit_process_i32,
        call_unit: keys.call_unit,
        call_i64: keys.call_i64,
        call_aggregate: keys.call_aggregate,
        return_aggregate: keys.return_aggregate,
        materialize_i64: keys.materialize_i64,
        materialize_boolean: keys.materialize_boolean,
        copy_i64: keys.copy_i64,
        float32_to_bits: keys.float32_to_bits,
        float64_to_bits: keys.float64_to_bits,
        bits_to_float32: keys.bits_to_float32,
        bits_to_float64: keys.bits_to_float64,
        call_unit_mixed: keys.call_unit_mixed,
        add_i64: keys.add_i64,
        add_i64_immediate: keys.add_i64_immediate,
        subtract_i64: keys.subtract_i64,
        subtract_i64_immediate: keys.subtract_i64_immediate,
        compare_i64_zero: keys.compare_i64_zero,
        compare_i64: keys.compare_i64,
        conditional_branch: keys.conditional_branch,
        jump: keys.jump,
        return_i64: keys.return_i64,
        return_unit: keys.return_unit,
    }
}

pub(super) fn selected_constraint_keys(target: NativeTarget) -> Option<SelectedConstraintKeys> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) => Some(SelectedConstraintKeys {
            call_aggregate: isa_x86_64::x86_64_system_v_aggregate_call_keys()
                .into_iter()
                .chain(isa_x86_64::x86_64_system_v_mixed_aggregate_call_keys())
                .collect(),
            return_aggregate: isa_x86_64::x86_64_system_v_aggregate_return_keys(),
            hosted_read_byte: (target == NativeTarget::linux_x64())
                .then_some(isa_x86_64::X86_64_HOSTED_READ_BYTE),
            hosted_write_byte_i32: Some(isa_x86_64::X86_64_HOSTED_WRITE_BYTE_I32),
            hosted_exit_process_i32: (target == NativeTarget::linux_x64())
                .then_some(isa_x86_64::X86_64_HOSTED_EXIT_PROCESS_I32),
            load64: Some(isa_x86_64::X86_64_LOAD64),
            load8: Some(isa_x86_64::X86_64_LOAD8),
            load16: Some(isa_x86_64::X86_64_LOAD16),
            load32: Some(isa_x86_64::X86_64_LOAD32),
            load_packed: Some(isa_x86_64::X86_64_LOAD_PACKED),
            store_packed: Some(isa_x86_64::X86_64_STORE_PACKED),
            load8_indexed: Some(isa_x86_64::X86_64_LOAD8_INDEXED),
            store: Some(isa_x86_64::X86_64_STORE),
            address_offset: Some(isa_x86_64::X86_64_ADDRESS_OFFSET),
            store64: Some(isa_x86_64::X86_64_STORE64),
            frame_address: Some(isa_x86_64::X86_64_FRAME_ADDRESS),
            call_unit: isa_x86_64::x86_64_system_v_register_unit_call_keys(),
            call_unit_mixed: isa_x86_64::x86_64_system_v_mixed_unit_call_keys(),
            call_i64: isa_x86_64::x86_64_system_v_register_call_keys(),
            materialize_i64: X86_64_MATERIALIZE_I64,
            materialize_boolean: isa_x86_64::X86_64_MATERIALIZE_BOOLEAN,
            copy_i64: X86_64_COPY_I64,
            float32_to_bits: Some(isa_x86_64::X86_64_FLOAT32_TO_BITS),
            float64_to_bits: Some(isa_x86_64::X86_64_FLOAT64_TO_BITS),
            bits_to_float32: Some(isa_x86_64::X86_64_BITS_TO_FLOAT32),
            bits_to_float64: Some(isa_x86_64::X86_64_BITS_TO_FLOAT64),
            add_i64: X86_64_ADD_I64,
            add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
            subtract_i64: X86_64_SUBTRACT_I64,
            subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: X86_64_COMPARE_I64_ZERO,
            compare_i64: isa_x86_64::X86_64_COMPARE_I64,
            conditional_branch: X86_64_CONDITIONAL_BRANCH,
            jump: isa_x86_64::X86_64_JUMP,
            return_i64: X86_64_SYSTEM_V_RETURN,
            return_unit: X86_64_SYSTEM_V_RETURN_UNIT,
        }),
        (Architecture::X86_64, ObjectFormat::Coff) => Some(SelectedConstraintKeys {
            call_aggregate: isa_x86_64::x86_64_microsoft_aggregate_call_keys()
                .into_iter()
                .chain(isa_x86_64::x86_64_microsoft_mixed_aggregate_call_keys())
                .collect(),
            return_aggregate: isa_x86_64::x86_64_microsoft_aggregate_return_keys(),
            hosted_read_byte: None,
            hosted_write_byte_i32: None,
            hosted_exit_process_i32: None,
            load64: Some(isa_x86_64::X86_64_LOAD64),
            load8: Some(isa_x86_64::X86_64_LOAD8),
            load16: Some(isa_x86_64::X86_64_LOAD16),
            load32: Some(isa_x86_64::X86_64_LOAD32),
            load_packed: Some(isa_x86_64::X86_64_LOAD_PACKED),
            store_packed: Some(isa_x86_64::X86_64_STORE_PACKED),
            load8_indexed: Some(isa_x86_64::X86_64_LOAD8_INDEXED),
            store: Some(isa_x86_64::X86_64_STORE),
            address_offset: Some(isa_x86_64::X86_64_ADDRESS_OFFSET),
            store64: Some(isa_x86_64::X86_64_STORE64),
            frame_address: Some(isa_x86_64::X86_64_FRAME_ADDRESS),
            call_unit: isa_x86_64::x86_64_microsoft_register_unit_call_keys(),
            call_unit_mixed: isa_x86_64::x86_64_microsoft_mixed_unit_call_keys(),
            call_i64: isa_x86_64::x86_64_microsoft_register_call_keys(),
            materialize_i64: X86_64_MATERIALIZE_I64,
            materialize_boolean: isa_x86_64::X86_64_MATERIALIZE_BOOLEAN,
            copy_i64: X86_64_COPY_I64,
            float32_to_bits: Some(isa_x86_64::X86_64_FLOAT32_TO_BITS),
            float64_to_bits: Some(isa_x86_64::X86_64_FLOAT64_TO_BITS),
            bits_to_float32: Some(isa_x86_64::X86_64_BITS_TO_FLOAT32),
            bits_to_float64: Some(isa_x86_64::X86_64_BITS_TO_FLOAT64),
            add_i64: X86_64_ADD_I64,
            add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
            subtract_i64: X86_64_SUBTRACT_I64,
            subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: X86_64_COMPARE_I64_ZERO,
            compare_i64: isa_x86_64::X86_64_COMPARE_I64,
            conditional_branch: X86_64_CONDITIONAL_BRANCH,
            jump: isa_x86_64::X86_64_JUMP,
            return_i64: X86_64_MICROSOFT_RETURN,
            return_unit: X86_64_MICROSOFT_RETURN_UNIT,
        }),
        (Architecture::Aarch64, ObjectFormat::Elf) => Some(SelectedConstraintKeys {
            call_aggregate: isa_aarch64::aarch64_register_aggregate_call_keys(false)
                .into_iter()
                .chain(isa_aarch64::aarch64_mixed_aggregate_call_keys(false))
                .collect(),
            return_aggregate: isa_aarch64::aarch64_register_aggregate_return_keys(false),
            hosted_read_byte: (target == NativeTarget::linux_arm64())
                .then_some(isa_aarch64::AARCH64_HOSTED_READ_BYTE),
            hosted_write_byte_i32: (target == NativeTarget::linux_arm64())
                .then_some(isa_aarch64::AARCH64_HOSTED_WRITE_BYTE_I32),
            hosted_exit_process_i32: (target == NativeTarget::linux_arm64())
                .then_some(isa_aarch64::AARCH64_HOSTED_EXIT_PROCESS_I32),
            load64: Some(isa_aarch64::AARCH64_LOAD64),
            load8: Some(isa_aarch64::AARCH64_LOAD8),
            load16: Some(isa_aarch64::AARCH64_LOAD16),
            load32: Some(isa_aarch64::AARCH64_LOAD32),
            load_packed: Some(isa_aarch64::AARCH64_LOAD_PACKED),
            store_packed: Some(isa_aarch64::AARCH64_STORE_PACKED),
            load8_indexed: Some(isa_aarch64::AARCH64_LOAD8_INDEXED),
            store: Some(isa_aarch64::AARCH64_STORE),
            address_offset: Some(isa_aarch64::AARCH64_ADDRESS_OFFSET),
            store64: Some(isa_aarch64::AARCH64_STORE64),
            frame_address: Some(isa_aarch64::AARCH64_FRAME_ADDRESS),
            call_unit: isa_aarch64::aarch64_aapcs64_register_unit_call_keys(),
            call_unit_mixed: isa_aarch64::aarch64_aapcs64_mixed_unit_call_keys(),
            call_i64: isa_aarch64::aarch64_aapcs64_register_call_keys(),
            materialize_i64: AARCH64_MATERIALIZE_I64,
            materialize_boolean: isa_aarch64::AARCH64_MATERIALIZE_BOOLEAN,
            copy_i64: AARCH64_COPY_I64,
            float32_to_bits: Some(isa_aarch64::AARCH64_FLOAT32_TO_BITS),
            float64_to_bits: Some(isa_aarch64::AARCH64_FLOAT64_TO_BITS),
            bits_to_float32: Some(isa_aarch64::AARCH64_BITS_TO_FLOAT32),
            bits_to_float64: Some(isa_aarch64::AARCH64_BITS_TO_FLOAT64),
            add_i64: AARCH64_ADD_I64,
            add_i64_immediate: AARCH64_ADD_I64_IMMEDIATE,
            subtract_i64: AARCH64_SUBTRACT_I64,
            subtract_i64_immediate: AARCH64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: AARCH64_COMPARE_I64_ZERO,
            compare_i64: isa_aarch64::AARCH64_COMPARE_I64,
            conditional_branch: AARCH64_CONDITIONAL_BRANCH,
            jump: isa_aarch64::AARCH64_JUMP,
            return_i64: AARCH64_AAPCS64_RETURN,
            return_unit: AARCH64_AAPCS64_RETURN_UNIT,
        }),
        (Architecture::Aarch64, ObjectFormat::MachO) => Some(SelectedConstraintKeys {
            call_aggregate: isa_aarch64::aarch64_register_aggregate_call_keys(true)
                .into_iter()
                .chain(isa_aarch64::aarch64_mixed_aggregate_call_keys(true))
                .collect(),
            return_aggregate: isa_aarch64::aarch64_register_aggregate_return_keys(true),
            hosted_read_byte: (target == NativeTarget::macos_arm64())
                .then_some(isa_aarch64::AARCH64_DARWIN_HOSTED_READ_BYTE),
            hosted_write_byte_i32: (target == NativeTarget::macos_arm64())
                .then_some(isa_aarch64::AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32),
            hosted_exit_process_i32: (target == NativeTarget::macos_arm64())
                .then_some(isa_aarch64::AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32),
            load64: Some(isa_aarch64::AARCH64_LOAD64),
            load8: Some(isa_aarch64::AARCH64_LOAD8),
            load16: Some(isa_aarch64::AARCH64_LOAD16),
            load32: Some(isa_aarch64::AARCH64_LOAD32),
            load_packed: Some(isa_aarch64::AARCH64_LOAD_PACKED),
            store_packed: Some(isa_aarch64::AARCH64_STORE_PACKED),
            load8_indexed: Some(isa_aarch64::AARCH64_LOAD8_INDEXED),
            store: Some(isa_aarch64::AARCH64_STORE),
            address_offset: Some(isa_aarch64::AARCH64_ADDRESS_OFFSET),
            store64: Some(isa_aarch64::AARCH64_STORE64),
            frame_address: Some(isa_aarch64::AARCH64_FRAME_ADDRESS),
            call_unit: isa_aarch64::aarch64_darwin_register_unit_call_keys(),
            call_unit_mixed: isa_aarch64::aarch64_darwin_mixed_unit_call_keys(),
            call_i64: isa_aarch64::aarch64_darwin_register_call_keys(),
            materialize_i64: AARCH64_MATERIALIZE_I64,
            materialize_boolean: isa_aarch64::AARCH64_MATERIALIZE_BOOLEAN,
            copy_i64: AARCH64_COPY_I64,
            float32_to_bits: Some(isa_aarch64::AARCH64_FLOAT32_TO_BITS),
            float64_to_bits: Some(isa_aarch64::AARCH64_FLOAT64_TO_BITS),
            bits_to_float32: Some(isa_aarch64::AARCH64_BITS_TO_FLOAT32),
            bits_to_float64: Some(isa_aarch64::AARCH64_BITS_TO_FLOAT64),
            add_i64: AARCH64_ADD_I64,
            add_i64_immediate: AARCH64_ADD_I64_IMMEDIATE,
            subtract_i64: AARCH64_SUBTRACT_I64,
            subtract_i64_immediate: AARCH64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: AARCH64_COMPARE_I64_ZERO,
            compare_i64: isa_aarch64::AARCH64_COMPARE_I64,
            conditional_branch: AARCH64_CONDITIONAL_BRANCH,
            jump: isa_aarch64::AARCH64_JUMP,
            return_i64: AARCH64_DARWIN_RETURN,
            return_unit: AARCH64_DARWIN_RETURN_UNIT,
        }),
        _ => None,
    }
}

/// Exact scalar-call ABI row selected by one native target. General selected
/// call lowering is not implemented yet; this mapping makes the future entry
/// explicit without adding call authority to the current selected CFG.
pub(super) const fn scalar_call_constraint_key(
    target: NativeTarget,
) -> Option<RegisterConstraintKey> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) => Some(X86_64_SYSTEM_V_CALL),
        (Architecture::X86_64, ObjectFormat::Coff) => Some(X86_64_MICROSOFT_CALL),
        (Architecture::Aarch64, ObjectFormat::Elf) => Some(AARCH64_AAPCS64_CALL),
        (Architecture::Aarch64, ObjectFormat::MachO) => Some(AARCH64_DARWIN_CALL),
        _ => None,
    }
}
