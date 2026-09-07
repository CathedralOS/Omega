use isa_aarch64::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_RETURN, AARCH64_AAPCS64_RETURN_UNIT, AARCH64_ADD_I64,
    AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH,
    AARCH64_COPY_I64, AARCH64_DARWIN_CALL, AARCH64_DARWIN_RETURN, AARCH64_DARWIN_RETURN_UNIT,
    AARCH64_MATERIALIZE_I64, AARCH64_SUBTRACT_I64, AARCH64_SUBTRACT_I64_IMMEDIATE,
    aarch64_physical_register_model, aarch64_register_constraint_catalog,
};
use isa_x86_64::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH,
    X86_64_COPY_I64, X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL,
    X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR, X86_64_MICROSOFT_RETURN,
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
        structural_unit_call: keys.structural_unit_call,
        call_i64: keys.call_i64,
        materialize_i64: keys.materialize_i64,
        copy_i64: keys.copy_i64,
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
            structural_unit_call: None,
            call_i64: isa_x86_64::x86_64_system_v_register_call_keys(),
            materialize_i64: X86_64_MATERIALIZE_I64,
            copy_i64: X86_64_COPY_I64,
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
            structural_unit_call: Some(X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR),
            call_i64: isa_x86_64::x86_64_microsoft_register_call_keys(),
            materialize_i64: X86_64_MATERIALIZE_I64,
            copy_i64: X86_64_COPY_I64,
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
            structural_unit_call: None,
            call_i64: isa_aarch64::aarch64_aapcs64_register_call_keys(),
            materialize_i64: AARCH64_MATERIALIZE_I64,
            copy_i64: AARCH64_COPY_I64,
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
            structural_unit_call: None,
            call_i64: isa_aarch64::aarch64_darwin_register_call_keys(),
            materialize_i64: AARCH64_MATERIALIZE_I64,
            copy_i64: AARCH64_COPY_I64,
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
