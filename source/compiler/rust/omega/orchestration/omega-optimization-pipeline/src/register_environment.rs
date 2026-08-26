use omega_register_model::{
    PhysicalRegisterModel, RegisterConstraintCatalog, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterModelValidationError, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, validate_physical_register_model,
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
    selected_keys: TerminalSelectedConstraintKeys,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRegisterEnvironmentValidationError {
    Physical(RegisterModelValidationError),
    TargetArchitectureMismatch {
        target: Architecture,
        model: Architecture,
    },
    X86_64(X86_64RegisterConstraintCatalogValidationError),
    Aarch64(Aarch64RegisterConstraintCatalogValidationError),
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
    validate_target_register_environment(target, physical, constraints)
}

/// Independently join raw physical and constraint declarations for one exact
/// native target. The architecture-neutral structural validators run first;
/// the selected clean ISA owner then checks every target-semantic row.
pub fn validate_target_register_environment(
    target: NativeTarget,
    physical: PhysicalRegisterModel,
    constraints: RegisterConstraintCatalog,
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
    Ok(ValidatedTargetRegisterEnvironment {
        target,
        physical,
        constraints,
        selected_keys,
    })
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
        }
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
