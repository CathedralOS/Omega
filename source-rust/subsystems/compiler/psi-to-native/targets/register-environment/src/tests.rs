/*
We organize register-environment verification suites across supported targets,
ABI call clobbers, and environment identity/reservation policy invariants. We
verify here that baseline reservation profiles enforce platform applicability
(such as Darwin AArch64 overlays) and that environment identity digests bind
every component and policy variation.
*/

mod abi_call_clobbers;
mod supported_targets;

use super::catalog::{
    conservative_baseline_reservation_profile, target_validated_physical_register_model,
};
use super::{
    TargetRegisterEnvironmentValidationError, assemble_target_register_environment,
    baseline_target_register_environment,
};
use register_model::validate_physical_register_model;
use target::NativeTarget;
use target_x86_64::{validated_x86_64_physical_register_model, x86_64_register_constraint_catalog};

#[test]
fn prepared_constraints_share_rows_and_keep_the_receiving_identity() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let cloned = environment.clone();
        assert!(std::ptr::eq(
            environment.constraints().catalog(),
            cloned.constraints().catalog()
        ));
        let (prepared, received) = match target.architecture {
            target::Architecture::X86_64 => (
                target_x86_64::prepare_x86_64_register_constraint_catalog(environment.physical())
                    .unwrap(),
                target_x86_64::validate_x86_64_register_constraint_catalog(
                    environment.constraints().catalog().clone(),
                    environment.physical(),
                )
                .unwrap(),
            ),
            target::Architecture::Aarch64 => (
                target_aarch64::prepare_aarch64_register_constraint_catalog(environment.physical())
                    .unwrap(),
                target_aarch64::validate_aarch64_register_constraint_catalog(
                    environment.constraints().catalog().clone(),
                    environment.physical(),
                )
                .unwrap(),
            ),
        };
        assert!(std::ptr::eq(
            prepared.catalog(),
            environment.constraints().catalog()
        ));
        assert_eq!(prepared, received);
        assert_eq!(prepared.identity(), received.identity());
        assert_eq!(
            prepared.physical_identity(),
            environment.physical().identity()
        );
    }
}

#[test]
fn prepared_constraints_require_the_exact_isa_model() {
    let x86 = target_x86_64::validated_x86_64_physical_register_model();
    let arm = target_aarch64::validated_aarch64_physical_register_model();
    assert!(matches!(target_x86_64::prepare_x86_64_register_constraint_catalog(arm),
        Err(target_x86_64::X86_64RegisterConstraintCatalogValidationError::PhysicalModelArchitectureMismatch)));
    assert!(matches!(target_aarch64::prepare_aarch64_register_constraint_catalog(x86),
        Err(target_aarch64::Aarch64RegisterConstraintCatalogValidationError::PhysicalModelArchitectureMismatch)));

    let mut changed = x86.model().clone();
    changed.conventions[0].name.push_str(".different-model");
    let changed = validate_physical_register_model(changed).unwrap();
    assert!(matches!(
        target_x86_64::prepare_x86_64_register_constraint_catalog(&changed),
        Err(target_x86_64::X86_64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel)
    ));
    let mut changed = arm.model().clone();
    changed.conventions[0].name.push_str(".different-model");
    let changed = validate_physical_register_model(changed).unwrap();
    assert!(matches!(
        target_aarch64::prepare_aarch64_register_constraint_catalog(&changed),
        Err(
            target_aarch64::Aarch64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel
        )
    ));
}

#[test]
fn prepared_constraints_cannot_cross_environment_roots() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let arm = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        assemble_target_register_environment(
            target,
            target_x86_64::validated_x86_64_physical_register_model(),
            arm.constraints().clone(),
            environment.reservations().profile().clone()
        ),
        Err(TargetRegisterEnvironmentValidationError::ConstraintArchitectureMismatch)
    );

    let mut changed_model = environment.physical().model().clone();
    changed_model.conventions[0]
        .name
        .push_str(".different-model");
    let changed_model = validate_physical_register_model(changed_model).unwrap();
    let other_root = register_model::produce_register_constraint_catalog(&changed_model, |_| {
        environment.constraints().catalog().clone()
    });
    assert_eq!(
        assemble_target_register_environment(
            target,
            target_x86_64::validated_x86_64_physical_register_model(),
            other_root,
            environment.reservations().profile().clone()
        ),
        Err(TargetRegisterEnvironmentValidationError::ConstraintPhysicalModelMismatch)
    );
}

#[test]
fn prepared_constraints_do_not_bypass_supplied_row_validation() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let call = environment.scalar_call_constraint().unwrap().key;
        let mut supplied = environment.constraints().catalog().clone();
        let row = supplied
            .constraints
            .iter_mut()
            .find(|row| row.key == call)
            .unwrap();
        assert!(!row.clobbers.is_empty());
        row.clobbers.clear();
        let error = validate_target_register_environment_join(
            target,
            target_validated_physical_register_model(target),
            supplied,
            environment.reservations().profile().clone(),
        )
        .unwrap_err();
        match target.architecture {
            target::Architecture::X86_64 => assert_eq!(error,
                TargetRegisterEnvironmentValidationError::X86_64(
                    target_x86_64::X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(call))),
            target::Architecture::Aarch64 => assert_eq!(error,
                TargetRegisterEnvironmentValidationError::Aarch64(
                    target_aarch64::Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(call))),
        }
    }
}

// Raw tables are test inputs for the receiving validators, not the default
// environment producer's already-prepared output.
fn validate_target_register_environment_join(
    target: NativeTarget,
    physical: &'static register_model::ValidatedPhysicalRegisterModel,
    constraints: register_model::RegisterConstraintCatalog,
    reservations: register_model::RegisterReservationProfile,
) -> Result<super::ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
    if target.architecture != physical.model().architecture {
        return Err(
            TargetRegisterEnvironmentValidationError::TargetArchitectureMismatch {
                target: target.architecture,
                model: physical.model().architecture,
            },
        );
    }
    let constraints = match target.architecture {
        target::Architecture::X86_64 => {
            target_x86_64::validate_x86_64_register_constraint_catalog(constraints, physical)
                .map_err(TargetRegisterEnvironmentValidationError::X86_64)?
        }
        target::Architecture::Aarch64 => {
            target_aarch64::validate_aarch64_register_constraint_catalog(constraints, physical)
                .map_err(TargetRegisterEnvironmentValidationError::Aarch64)?
        }
    };
    assemble_target_register_environment(target, physical, constraints, reservations)
}

#[test]
fn baseline_profile_is_exact_conservative_and_platform_applicable() {
    let linux = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    let macos = baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert!(
        !linux
            .reservations()
            .profile()
            .active_overlays
            .iter()
            .any(|name| name == target_aarch64::AARCH64_PLATFORM_RESERVATION_OVERLAY)
    );
    assert!(
        macos
            .reservations()
            .profile()
            .active_overlays
            .iter()
            .any(|name| name == target_aarch64::AARCH64_PLATFORM_RESERVATION_OVERLAY)
    );
    assert_ne!(
        linux.reservations().identity(),
        macos.reservations().identity()
    );
    assert_ne!(linux.identity(), macos.identity());

    let physical = target_aarch64::validated_aarch64_physical_register_model();
    let catalog = target_aarch64::aarch64_register_constraint_catalog(physical);
    let mut inapplicable =
        conservative_baseline_reservation_profile(NativeTarget::macos_arm64(), physical.model());
    inapplicable.name = "test.inapplicable-platform".into();
    assert_eq!(
        validate_target_register_environment_join(
            NativeTarget::linux_arm64(),
            physical,
            catalog,
            inapplicable,
        ),
        Err(TargetRegisterEnvironmentValidationError::InapplicableReservationOverlay)
    );
}

// Windows ARM64 keeps the Thread Environment Block pointer in x18, so the
// allocator must never hand x18 out there, exactly as on Apple platforms.
#[test]
fn x18_is_reserved_on_macos_and_windows_arm64_but_not_linux_arm64() {
    let x18_reserved = |target: NativeTarget| {
        let environment = baseline_target_register_environment(target).unwrap();
        let model = environment.physical().model();
        let x18 = model.view_named("x18").unwrap();
        let reserved = environment.reservations().reserved_units();
        let overlay_active = environment
            .reservations()
            .profile()
            .active_overlays
            .iter()
            .any(|name| name == target_aarch64::AARCH64_PLATFORM_RESERVATION_OVERLAY);
        let units_reserved = x18.units.iter().all(|unit| reserved.contains(unit));
        assert_eq!(overlay_active, units_reserved, "{target:?}");
        units_reserved
    };
    assert!(!x18_reserved(NativeTarget::linux_arm64()));
    assert!(x18_reserved(NativeTarget::macos_arm64()));
    assert!(x18_reserved(NativeTarget::windows_arm64()));

    let windows = baseline_target_register_environment(NativeTarget::windows_arm64()).unwrap();
    let linux = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_ne!(
        windows.reservations().identity(),
        linux.reservations().identity()
    );
}

#[test]
fn environment_identity_binds_each_component_and_explicit_policy() {
    let target = NativeTarget::linux_x64();
    let baseline = baseline_target_register_environment(target).unwrap();
    assert_ne!(
        baseline.physical().identity(),
        baseline_target_register_environment(NativeTarget::linux_arm64())
            .unwrap()
            .physical()
            .identity()
    );
    assert_ne!(
        baseline.constraints().identity(),
        baseline_target_register_environment(NativeTarget::linux_arm64())
            .unwrap()
            .constraints()
            .identity()
    );

    let physical = validated_x86_64_physical_register_model();
    let catalog = x86_64_register_constraint_catalog(physical);
    let mut reduced = conservative_baseline_reservation_profile(target, physical.model());
    reduced.name = "test.no-metering-reservation".into();
    reduced
        .active_overlays
        .retain(|name| name != "omega.x86.metering");
    let reduced =
        validate_target_register_environment_join(target, physical, catalog.clone(), reduced)
            .unwrap();
    assert_ne!(
        baseline.reservations().identity(),
        reduced.reservations().identity()
    );
    assert_ne!(baseline.identity(), reduced.identity());

    let changed_layout_target = NativeTarget {
        pointer_size: 4,
        ..target
    };
    let changed_layout = validate_target_register_environment_join(
        changed_layout_target,
        physical,
        catalog.clone(),
        conservative_baseline_reservation_profile(changed_layout_target, physical.model()),
    )
    .unwrap();
    assert_ne!(baseline.identity(), changed_layout.identity());

    let windows = baseline_target_register_environment(NativeTarget::windows_x64()).unwrap();
    assert_eq!(
        baseline.physical().identity(),
        windows.physical().identity()
    );
    assert_eq!(
        baseline.constraints().identity(),
        windows.constraints().identity()
    );
    assert_ne!(baseline.identity(), windows.identity());
}
