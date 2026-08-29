use omega_isa_x86_64::{x86_64_physical_register_model, x86_64_register_constraint_catalog};
use omega_register_model::validate_physical_register_model;
use omega_target::NativeTarget;

use super::super::catalog::conservative_baseline_reservation_profile;
use super::super::*;

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
            .any(|name| name == "darwin.aarch64.platform")
    );
    assert!(
        macos
            .reservations()
            .profile()
            .active_overlays
            .iter()
            .any(|name| name == "darwin.aarch64.platform")
    );
    assert_ne!(
        linux.reservations().identity(),
        macos.reservations().identity()
    );
    assert_ne!(linux.identity(), macos.identity());

    let raw = omega_isa_aarch64::aarch64_physical_register_model();
    let physical = validate_physical_register_model(raw.clone()).unwrap();
    let catalog = omega_isa_aarch64::aarch64_register_constraint_catalog(&physical);
    let mut inapplicable =
        conservative_baseline_reservation_profile(NativeTarget::macos_arm64(), &raw);
    inapplicable.name = "test.inapplicable-platform".into();
    assert_eq!(
        validate_target_register_environment_with_reservations(
            NativeTarget::linux_arm64(),
            raw,
            catalog,
            inapplicable,
        ),
        Err(TargetRegisterEnvironmentValidationError::InapplicableReservationOverlay)
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

    let raw = x86_64_physical_register_model();
    let physical = validate_physical_register_model(raw.clone()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&physical);
    let mut reduced = conservative_baseline_reservation_profile(target, &raw);
    reduced.name = "test.no-metering-reservation".into();
    reduced
        .active_overlays
        .retain(|name| name != "omega.x86.metering");
    let reduced = validate_target_register_environment_with_reservations(
        target,
        raw.clone(),
        catalog.clone(),
        reduced,
    )
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
    let changed_layout = validate_target_register_environment_with_reservations(
        changed_layout_target,
        raw.clone(),
        catalog.clone(),
        conservative_baseline_reservation_profile(changed_layout_target, &raw),
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
