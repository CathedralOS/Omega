use omega_isa_aarch64::{
    AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE, AARCH64_COPY_I64, AARCH64_SUBTRACT_I64,
};
use omega_isa_x86_64::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO, X86_64_COPY_I64,
    X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR, X86_64_SUBTRACT_I64,
    X86_64RegisterConstraintCatalogValidationError, x86_64_physical_register_model,
    x86_64_register_constraint_catalog,
};
use omega_register_model::validate_physical_register_model;
use omega_target::{Architecture, ObjectFormat};

use super::catalog::conservative_baseline_reservation_profile;
use super::*;

#[test]
fn every_supported_native_target_builds_a_matching_closed_environment() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        assert_eq!(environment.target(), target);
        assert_eq!(
            environment.physical().model().architecture,
            target.architecture
        );
        assert_eq!(
            environment.constraints().architecture(),
            target.architecture
        );
        assert_eq!(
            environment.constraints().catalog().required,
            environment
                .constraints()
                .catalog()
                .constraints
                .iter()
                .map(|constraint| constraint.key)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            environment.identity(),
            baseline_target_register_environment(target)
                .unwrap()
                .identity()
        );
        let (expected_copy, expected_add, expected_add_immediate, expected_subtract) =
            match target.architecture {
                Architecture::X86_64 => (
                    X86_64_COPY_I64,
                    X86_64_ADD_I64,
                    X86_64_ADD_I64_IMMEDIATE,
                    X86_64_SUBTRACT_I64,
                ),
                Architecture::Aarch64 => (
                    AARCH64_COPY_I64,
                    AARCH64_ADD_I64,
                    AARCH64_ADD_I64_IMMEDIATE,
                    AARCH64_SUBTRACT_I64,
                ),
            };
        assert_eq!(environment.selected_keys().copy_i64, expected_copy);
        assert_eq!(environment.selected_keys().add_i64, expected_add);
        assert_eq!(
            environment.selected_keys().add_i64_immediate,
            expected_add_immediate
        );
        assert_eq!(environment.selected_keys().subtract_i64, expected_subtract);
        assert_eq!(
            environment.allocation_constraint_keys().copy_i64,
            expected_copy
        );
        assert_eq!(
            environment.allocation_constraint_keys().add_i64,
            expected_add
        );
        assert_eq!(
            environment.allocation_constraint_keys().add_i64_immediate,
            expected_add_immediate
        );
        assert_eq!(
            environment.allocation_constraint_keys().subtract_i64,
            expected_subtract
        );
        assert!(environment.constraint(expected_copy).is_some());
        assert!(environment.constraint(expected_add).is_some());
        assert!(environment.constraint(expected_add_immediate).is_some());
        assert!(environment.constraint(expected_subtract).is_some());
        let expected_structural_call = matches!(
            (target.architecture, target.object_format),
            (Architecture::X86_64, ObjectFormat::Coff)
        )
        .then_some(X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR);
        assert_eq!(
            environment.selected_keys().structural_unit_call,
            expected_structural_call
        );
        assert_eq!(
            environment
                .allocation_constraint_keys()
                .structural_unit_call,
            expected_structural_call
        );
        if let Some(key) = expected_structural_call {
            let row = environment
                .constraint(key)
                .expect("applicable structural Unit call row is catalog-owned");
            assert!(row.operands.is_empty());
        }
    }
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

#[test]
fn raw_join_rejects_target_drift_and_target_semantic_corruption() {
    let raw = x86_64_physical_register_model();
    let physical = validate_physical_register_model(raw.clone()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&physical);
    assert_eq!(
        validate_target_register_environment(
            NativeTarget::linux_arm64(),
            raw.clone(),
            catalog.clone()
        ),
        Err(
            TargetRegisterEnvironmentValidationError::TargetArchitectureMismatch {
                target: Architecture::Aarch64,
                model: Architecture::X86_64,
            }
        )
    );

    let mut corrupted = catalog;
    let compare = corrupted
        .constraints
        .iter_mut()
        .find(|constraint| constraint.key == X86_64_COMPARE_I64_ZERO)
        .unwrap();
    compare.implicit_defs.clear();
    assert!(matches!(
        validate_target_register_environment(NativeTarget::linux_x64(), raw, corrupted),
        Err(TargetRegisterEnvironmentValidationError::X86_64(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                X86_64_COMPARE_I64_ZERO
            )
        ))
    ));

    let canonical = x86_64_physical_register_model();
    let canonical_validated = validate_physical_register_model(canonical.clone()).unwrap();
    let canonical_catalog = x86_64_register_constraint_catalog(&canonical_validated);
    let mut forged = canonical;
    forged.views[0].name = "forged.rax".into();
    assert_eq!(
        validate_target_register_environment(NativeTarget::linux_x64(), forged, canonical_catalog,),
        Err(TargetRegisterEnvironmentValidationError::X86_64(
            X86_64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel,
        ))
    );
}
