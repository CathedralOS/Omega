/*
We select the register facts each target declares: its physical register model,
instruction constraint catalog, conservative baseline reservation profile, and
ISA-owned selected constraint-key roster. By consuming the single ISA-declared
constraint-key roster per target rather than re-deriving a projection here, we
ensure the allocating environment and machine-effect catalog never diverge on
declared keys.
*/

#[cfg(test)]
use isa_aarch64::{AARCH64_AAPCS64_CALL, AARCH64_DARWIN_CALL};
use isa_aarch64::{aarch64_physical_register_model, prepare_aarch64_register_constraint_catalog};
#[cfg(test)]
use isa_x86_64::{X86_64_MICROSOFT_CALL, X86_64_SYSTEM_V_CALL};
use isa_x86_64::{prepare_x86_64_register_constraint_catalog, x86_64_physical_register_model};
#[cfg(test)]
use register_model::RegisterConstraintKey;
use register_model::{
    PhysicalRegisterModel, PreparedRegisterConstraintCatalog, RegisterReservationProfile,
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

pub(super) fn prepared_target_constraint_catalog(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<PreparedRegisterConstraintCatalog, super::TargetRegisterEnvironmentValidationError> {
    match target.architecture {
        Architecture::X86_64 => prepare_x86_64_register_constraint_catalog(physical)
            .map_err(super::TargetRegisterEnvironmentValidationError::X86_64),
        Architecture::Aarch64 => prepare_aarch64_register_constraint_catalog(physical)
            .map_err(super::TargetRegisterEnvironmentValidationError::Aarch64),
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

pub(super) fn selected_constraint_keys(target: NativeTarget) -> Option<SelectedConstraintKeys> {
    match target.architecture {
        Architecture::X86_64 => isa_x86_64::x86_64_selected_constraint_keys(target),
        Architecture::Aarch64 => isa_aarch64::aarch64_selected_constraint_keys(target),
    }
}

#[test]
fn float_result_policy_keys_are_declared_for_every_hosted_target() {
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
    }
}

#[cfg(test)]
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
