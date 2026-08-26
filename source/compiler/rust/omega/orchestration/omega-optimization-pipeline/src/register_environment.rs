use omega_register_model::{
    PhysicalRegisterModel, RegisterConstraintCatalog, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterModelValidationError, RegisterReservationProfile,
    RegisterReservationProfileValidationError, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity, validate_physical_register_model,
    validate_register_reservation_profile,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_isa_aarch64::{
    AARCH64_AAPCS64_RETURN, AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH,
    AARCH64_DARWIN_RETURN, AARCH64_MATERIALIZE_I64,
    Aarch64RegisterConstraintCatalogValidationError, aarch64_fixed_register_view,
    aarch64_physical_register_model, aarch64_register_constraint_catalog,
    validate_aarch64_register_constraint_catalog,
};
use omega_terminal_isa_x86_64::{
    X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH, X86_64_MATERIALIZE_I64,
    X86_64_MICROSOFT_RETURN, X86_64_SYSTEM_V_RETURN,
    X86_64RegisterConstraintCatalogValidationError, validate_x86_64_register_constraint_catalog,
    x86_64_fixed_register_view, x86_64_physical_register_model, x86_64_register_constraint_catalog,
};
use omega_terminal_selected_instructions::TerminalSelectedConstraintKeys;

/// Clean-lane custody of the exact target, independently validated physical
/// register model, and target-semantic instruction constraint catalog.
///
/// This is allocator input, not allocator output. It grants no physical-home,
/// machine-emission, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTargetRegisterEnvironment {
    target: NativeTarget,
    physical: ValidatedPhysicalRegisterModel,
    constraints: ValidatedRegisterConstraintCatalog,
    reservations: ValidatedRegisterReservationProfile,
    selected_keys: TerminalSelectedConstraintKeys,
    identity: TargetRegisterEnvironmentIdentity,
}

impl ValidatedTargetRegisterEnvironment {
    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn physical(&self) -> &ValidatedPhysicalRegisterModel {
        &self.physical
    }

    pub const fn constraints(&self) -> &ValidatedRegisterConstraintCatalog {
        &self.constraints
    }

    pub const fn reservations(&self) -> &ValidatedRegisterReservationProfile {
        &self.reservations
    }

    pub const fn identity(&self) -> TargetRegisterEnvironmentIdentity {
        self.identity
    }

    pub fn constraint(&self, key: RegisterConstraintKey) -> Option<&RegisterInstructionConstraint> {
        self.constraints
            .catalog()
            .constraints
            .iter()
            .find(|constraint| constraint.key == key)
    }

    pub const fn selected_keys(&self) -> TerminalSelectedConstraintKeys {
        self.selected_keys
    }

    pub const fn allocation_constraint_keys(&self) -> TargetRegisterEnvironmentConstraintKeys {
        selected_environment_keys(self.selected_keys)
    }

    pub fn fixed_register_view(
        &self,
        register: omega_terminal_target_operations::MachineRegister,
    ) -> Option<omega_register_model::RegisterViewId> {
        match self.target.architecture {
            Architecture::X86_64 => x86_64_fixed_register_view(&self.physical, register),
            Architecture::Aarch64 => aarch64_fixed_register_view(&self.physical, register),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetRegisterEnvironmentValidationError {
    Physical(RegisterModelValidationError),
    TargetArchitectureMismatch {
        target: Architecture,
        model: Architecture,
    },
    X86_64(X86_64RegisterConstraintCatalogValidationError),
    Aarch64(Aarch64RegisterConstraintCatalogValidationError),
    Reservations(RegisterReservationProfileValidationError),
    InapplicableReservationOverlay,
    UnsupportedSelectedInstructionAbi,
}

impl std::fmt::Display for TargetRegisterEnvironmentValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid target register environment: {self:?}")
    }
}

impl std::error::Error for TargetRegisterEnvironmentValidationError {}

/// Build the current target-owned baseline environment and pass it through the
/// same independently callable join validator used for decoded/cached models.
pub fn baseline_target_register_environment(
    target: NativeTarget,
) -> Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
    let physical = match target.architecture {
        Architecture::X86_64 => x86_64_physical_register_model(),
        Architecture::Aarch64 => aarch64_physical_register_model(),
    };
    let validated = validate_physical_register_model(physical.clone())
        .map_err(TargetRegisterEnvironmentValidationError::Physical)?;
    let constraints = match target.architecture {
        Architecture::X86_64 => x86_64_register_constraint_catalog(&validated),
        Architecture::Aarch64 => aarch64_register_constraint_catalog(&validated),
    };
    let reservations = conservative_baseline_reservation_profile(target, &physical);
    validate_target_register_environment_with_reservations(
        target,
        physical,
        constraints,
        reservations,
    )
}

/// Independently join raw physical and constraint declarations for one exact
/// native target. The architecture-neutral structural validators run first;
/// the selected clean ISA owner then checks every target-semantic row.
pub fn validate_target_register_environment(
    target: NativeTarget,
    physical: PhysicalRegisterModel,
    constraints: RegisterConstraintCatalog,
) -> Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
    let reservations = conservative_baseline_reservation_profile(target, &physical);
    validate_target_register_environment_with_reservations(
        target,
        physical,
        constraints,
        reservations,
    )
}

/// Join the exact raw artifacts and an explicit active reservation profile.
/// This is the cache/decode validation boundary; no reservation declaration
/// becomes active merely by appearing in the physical model.
pub fn validate_target_register_environment_with_reservations(
    target: NativeTarget,
    physical: PhysicalRegisterModel,
    constraints: RegisterConstraintCatalog,
    reservations: RegisterReservationProfile,
) -> Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
    let physical = validate_physical_register_model(physical)
        .map_err(TargetRegisterEnvironmentValidationError::Physical)?;
    if target.architecture != physical.model().architecture {
        return Err(
            TargetRegisterEnvironmentValidationError::TargetArchitectureMismatch {
                target: target.architecture,
                model: physical.model().architecture,
            },
        );
    }
    let constraints = match target.architecture {
        Architecture::X86_64 => validate_x86_64_register_constraint_catalog(constraints, &physical)
            .map_err(TargetRegisterEnvironmentValidationError::X86_64)?,
        Architecture::Aarch64 => {
            validate_aarch64_register_constraint_catalog(constraints, &physical)
                .map_err(TargetRegisterEnvironmentValidationError::Aarch64)?
        }
    };
    let selected_keys = selected_constraint_keys(target)
        .ok_or(TargetRegisterEnvironmentValidationError::UnsupportedSelectedInstructionAbi)?;
    if reservations
        .active_overlays
        .iter()
        .any(|name| name == "darwin.aarch64.platform")
        && target.object_format != ObjectFormat::MachO
    {
        return Err(TargetRegisterEnvironmentValidationError::InapplicableReservationOverlay);
    }
    let reservations = validate_register_reservation_profile(reservations, target, &physical)
        .map_err(TargetRegisterEnvironmentValidationError::Reservations)?;
    let identity = target_register_environment_identity(
        target,
        &physical,
        &constraints,
        &reservations,
        selected_environment_keys(selected_keys),
    );
    Ok(ValidatedTargetRegisterEnvironment {
        target,
        physical,
        constraints,
        reservations,
        selected_keys,
        identity,
    })
}

fn conservative_baseline_reservation_profile(
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

const fn selected_environment_keys(
    keys: TerminalSelectedConstraintKeys,
) -> TargetRegisterEnvironmentConstraintKeys {
    TargetRegisterEnvironmentConstraintKeys {
        materialize_i64: keys.materialize_i64,
        compare_i64_zero: keys.compare_i64_zero,
        conditional_branch: keys.conditional_branch,
        return_i64: keys.return_i64,
    }
}

fn selected_constraint_keys(target: NativeTarget) -> Option<TerminalSelectedConstraintKeys> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) => Some(TerminalSelectedConstraintKeys {
            materialize_i64: X86_64_MATERIALIZE_I64,
            compare_i64_zero: X86_64_COMPARE_I64_ZERO,
            conditional_branch: X86_64_CONDITIONAL_BRANCH,
            return_i64: X86_64_SYSTEM_V_RETURN,
        }),
        (Architecture::X86_64, ObjectFormat::Coff) => Some(TerminalSelectedConstraintKeys {
            materialize_i64: X86_64_MATERIALIZE_I64,
            compare_i64_zero: X86_64_COMPARE_I64_ZERO,
            conditional_branch: X86_64_CONDITIONAL_BRANCH,
            return_i64: X86_64_MICROSOFT_RETURN,
        }),
        (Architecture::Aarch64, ObjectFormat::Elf) => Some(TerminalSelectedConstraintKeys {
            materialize_i64: AARCH64_MATERIALIZE_I64,
            compare_i64_zero: AARCH64_COMPARE_I64_ZERO,
            conditional_branch: AARCH64_CONDITIONAL_BRANCH,
            return_i64: AARCH64_AAPCS64_RETURN,
        }),
        (Architecture::Aarch64, ObjectFormat::MachO) => Some(TerminalSelectedConstraintKeys {
            materialize_i64: AARCH64_MATERIALIZE_I64,
            compare_i64_zero: AARCH64_COMPARE_I64_ZERO,
            conditional_branch: AARCH64_CONDITIONAL_BRANCH,
            return_i64: AARCH64_DARWIN_RETURN,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use omega_register_model::validate_physical_register_model;
    use omega_terminal_isa_x86_64::{
        X86_64_COMPARE_I64_ZERO, x86_64_physical_register_model, x86_64_register_constraint_catalog,
    };

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

        let raw = omega_terminal_isa_aarch64::aarch64_physical_register_model();
        let physical = validate_physical_register_model(raw.clone()).unwrap();
        let catalog = omega_terminal_isa_aarch64::aarch64_register_constraint_catalog(&physical);
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
            validate_target_register_environment(
                NativeTarget::linux_x64(),
                forged,
                canonical_catalog,
            ),
            Err(TargetRegisterEnvironmentValidationError::X86_64(
                X86_64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel,
            ))
        );
    }
}
