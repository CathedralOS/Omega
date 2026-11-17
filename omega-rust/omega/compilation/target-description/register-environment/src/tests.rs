/*
We organize register-environment verification suites across supported targets,
ABI call clobbers, and environment identity/reservation policy invariants. We
verify here that baseline reservation profiles enforce platform applicability
(such as Darwin AArch64 overlays) and that environment identity digests bind
every component and policy variation.
*/

mod abi_call_clobbers;
mod supported_targets;

use super::catalog::conservative_baseline_reservation_profile;
use super::{
    TargetRegisterEnvironmentValidationError, assemble_target_register_environment,
    baseline_target_register_environment,
};
use isa_x86_64::{x86_64_physical_register_model, x86_64_register_constraint_catalog};
use register_model::validate_physical_register_model;
use target::NativeTarget;

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
                isa_x86_64::prepare_x86_64_register_constraint_catalog(environment.physical())
                    .unwrap(),
                isa_x86_64::validate_x86_64_register_constraint_catalog(
                    environment.constraints().catalog().clone(),
                    environment.physical(),
                )
                .unwrap(),
            ),
            target::Architecture::Aarch64 => (
                isa_aarch64::prepare_aarch64_register_constraint_catalog(environment.physical())
                    .unwrap(),
                isa_aarch64::validate_aarch64_register_constraint_catalog(
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
    let x86 = isa_x86_64::validated_x86_64_physical_register_model();
    let arm = isa_aarch64::validated_aarch64_physical_register_model();
    assert!(matches!(isa_x86_64::prepare_x86_64_register_constraint_catalog(arm),
        Err(isa_x86_64::X86_64RegisterConstraintCatalogValidationError::PhysicalModelArchitectureMismatch)));
    assert!(matches!(isa_aarch64::prepare_aarch64_register_constraint_catalog(x86),
        Err(isa_aarch64::Aarch64RegisterConstraintCatalogValidationError::PhysicalModelArchitectureMismatch)));

    let mut changed = x86.model().clone();
    changed.conventions[0].name.push_str(".different-model");
    let changed = validate_physical_register_model(changed).unwrap();
    assert!(matches!(
        isa_x86_64::prepare_x86_64_register_constraint_catalog(&changed),
        Err(isa_x86_64::X86_64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel)
    ));
    let mut changed = arm.model().clone();
    changed.conventions[0].name.push_str(".different-model");
    let changed = validate_physical_register_model(changed).unwrap();
    assert!(matches!(
        isa_aarch64::prepare_aarch64_register_constraint_catalog(&changed),
        Err(
            isa_aarch64::Aarch64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel
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
            environment.physical().clone(),
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
            environment.physical().clone(),
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
            environment.physical().clone(),
            supplied,
            environment.reservations().profile().clone(),
        )
        .unwrap_err();
        match target.architecture {
            target::Architecture::X86_64 => assert_eq!(error,
                TargetRegisterEnvironmentValidationError::X86_64(
                    isa_x86_64::X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(call))),
            target::Architecture::Aarch64 => assert_eq!(error,
                TargetRegisterEnvironmentValidationError::Aarch64(
                    isa_aarch64::Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(call))),
        }
    }
}

// Raw tables are test inputs for the receiving validators, not the default
// environment producer's already-prepared output.
fn validate_target_register_environment_join(
    target: NativeTarget,
    physical: register_model::ValidatedPhysicalRegisterModel,
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
            isa_x86_64::validate_x86_64_register_constraint_catalog(constraints, &physical)
                .map_err(TargetRegisterEnvironmentValidationError::X86_64)?
        }
        target::Architecture::Aarch64 => {
            isa_aarch64::validate_aarch64_register_constraint_catalog(constraints, &physical)
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

    let raw = isa_aarch64::aarch64_physical_register_model();
    let physical = validate_physical_register_model(raw.clone()).unwrap();
    let catalog = isa_aarch64::aarch64_register_constraint_catalog(&physical);
    let mut inapplicable =
        conservative_baseline_reservation_profile(NativeTarget::macos_arm64(), &raw);
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
    let reduced = validate_target_register_environment_join(
        target,
        physical.clone(),
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
    let changed_layout = validate_target_register_environment_join(
        changed_layout_target,
        physical,
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
