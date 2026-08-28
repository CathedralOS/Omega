use omega_register_model::ValidatedRegisterConstraintCatalog;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternative, TerminalMachineAlternativeApplicability,
    TerminalMachineAlternativeKey, TerminalMachineBarrier, TerminalMachineCallEffect,
    TerminalMachineCleanupEffect, TerminalMachineEffectCatalog,
    TerminalMachineEffectCatalogValidationError, TerminalMachineEffectDeclaration,
    TerminalMachineEncodedControlEffect, TerminalMachineEncodedEffects,
    TerminalMachineEncodedMemoryEffect, TerminalMachineEncodedStackEffect,
    TerminalMachineEncodedTrapBehavior, TerminalMachineLatencyKnowledge,
    TerminalMachineMemoryEffect, TerminalMachineSemanticKind, TerminalMachineSizeKnowledge,
    TerminalMachineTrapBehavior, TerminalSelectedConstraintKeys,
    ValidatedTerminalMachineEffectCatalog, validate_terminal_machine_effect_catalog,
};

use crate::{
    AARCH64_AAPCS64_RETURN, AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64_ZERO,
    AARCH64_CONDITIONAL_BRANCH, AARCH64_COPY_I64, AARCH64_DARWIN_RETURN, AARCH64_MATERIALIZE_I64,
    AARCH64_SUBTRACT_I64,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64TerminalMachineEffectCatalogValidationError {
    TargetArchitectureMismatch,
    UnsupportedTargetAbi,
    Structural(TerminalMachineEffectCatalogValidationError),
    TargetSemanticMismatch,
}

impl std::fmt::Display for Aarch64TerminalMachineEffectCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid AArch64 Terminal machine effects: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64TerminalMachineEffectCatalogValidationError {}

pub fn aarch64_terminal_machine_effect_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalMachineEffectCatalog, Aarch64TerminalMachineEffectCatalogValidationError> {
    if target.architecture != Architecture::Aarch64
        || constraints.architecture() != Architecture::Aarch64
    {
        return Err(Aarch64TerminalMachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    let selected_keys = selected_keys(target)?;
    Ok(TerminalMachineEffectCatalog {
        target,
        register_constraints: constraints.identity(),
        selected_keys,
        declarations: TerminalMachineSemanticKind::ALL
            .into_iter()
            .map(|semantic| declaration(semantic, selected_keys))
            .collect(),
    })
}

pub fn validate_aarch64_terminal_machine_effect_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: TerminalMachineEffectCatalog,
) -> Result<ValidatedTerminalMachineEffectCatalog, Aarch64TerminalMachineEffectCatalogValidationError>
{
    if target.architecture != Architecture::Aarch64
        || constraints.architecture() != Architecture::Aarch64
    {
        return Err(Aarch64TerminalMachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    let canonical = aarch64_terminal_machine_effect_catalog(target, constraints)?;
    let validated = validate_terminal_machine_effect_catalog(constraints, catalog)
        .map_err(Aarch64TerminalMachineEffectCatalogValidationError::Structural)?;
    if validated.catalog() != &canonical {
        return Err(Aarch64TerminalMachineEffectCatalogValidationError::TargetSemanticMismatch);
    }
    Ok(validated)
}

fn selected_keys(
    target: NativeTarget,
) -> Result<TerminalSelectedConstraintKeys, Aarch64TerminalMachineEffectCatalogValidationError> {
    let return_i64 = match target.object_format {
        ObjectFormat::Elf => AARCH64_AAPCS64_RETURN,
        ObjectFormat::MachO => AARCH64_DARWIN_RETURN,
        ObjectFormat::Coff => {
            return Err(Aarch64TerminalMachineEffectCatalogValidationError::UnsupportedTargetAbi);
        }
    };
    Ok(TerminalSelectedConstraintKeys {
        materialize_i64: AARCH64_MATERIALIZE_I64,
        copy_i64: AARCH64_COPY_I64,
        add_i64: AARCH64_ADD_I64,
        subtract_i64: AARCH64_SUBTRACT_I64,
        add_i64_immediate: AARCH64_ADD_I64_IMMEDIATE,
        compare_i64_zero: AARCH64_COMPARE_I64_ZERO,
        conditional_branch: AARCH64_CONDITIONAL_BRANCH,
        return_i64,
    })
}

fn declaration(
    semantic: TerminalMachineSemanticKind,
    keys: TerminalSelectedConstraintKeys,
) -> TerminalMachineEffectDeclaration {
    TerminalMachineEffectDeclaration {
        semantic,
        constraint: keys.for_semantic(semantic),
        memory: TerminalMachineMemoryEffect::NoneV1,
        trap: TerminalMachineTrapBehavior::NeverV1,
        barrier: if matches!(
            semantic,
            TerminalMachineSemanticKind::ConditionalBranchNonZero
                | TerminalMachineSemanticKind::ReturnI64
        ) {
            TerminalMachineBarrier::ControlFlow
        } else {
            TerminalMachineBarrier::None
        },
        call: TerminalMachineCallEffect::NoneV1,
        cleanup: TerminalMachineCleanupEffect::NoneV1,
        alternatives: vec![TerminalMachineAlternative {
            key: TerminalMachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            },
            applicability: TerminalMachineAlternativeApplicability::Always,
            size: size(semantic),
            latency: TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: encoded_effects(semantic),
        }],
    }
}

fn encoded_effects(semantic: TerminalMachineSemanticKind) -> TerminalMachineEncodedEffects {
    let physical = crate::aarch64_physical_register_model();
    let units = |name: &str| {
        physical
            .view_named(name)
            .unwrap_or_else(|| panic!("canonical AArch64 model declares {name}"))
            .units
            .clone()
    };
    let view = |name: &str| {
        physical
            .view_named(name)
            .unwrap_or_else(|| panic!("canonical AArch64 model declares {name}"))
            .id
    };
    let (reads, writes) = match semantic {
        TerminalMachineSemanticKind::CompareI64Zero => (vec![0], vec![]),
        TerminalMachineSemanticKind::MaterializeI64 => (vec![], vec![0]),
        TerminalMachineSemanticKind::CopyI64 => (vec![0], vec![1]),
        TerminalMachineSemanticKind::ExactAddI64
        | TerminalMachineSemanticKind::ExactSubtractI64 => (vec![0, 1], vec![2]),
        TerminalMachineSemanticKind::ExactAddI64Immediate => (vec![0], vec![1]),
        TerminalMachineSemanticKind::ConditionalBranchNonZero
        | TerminalMachineSemanticKind::ReturnI64 => (vec![], vec![]),
    };
    let (implicit_uses, implicit_defs, trap, control) = match semantic {
        TerminalMachineSemanticKind::CompareI64Zero => (
            vec![],
            units("nzcv"),
            TerminalMachineEncodedTrapBehavior::NeverV1,
            TerminalMachineEncodedControlEffect::FallThroughV1,
        ),
        TerminalMachineSemanticKind::ConditionalBranchNonZero => {
            let mut uses = units("nzcv");
            uses.extend(units("pc"));
            uses.sort_unstable();
            uses.dedup();
            (
                uses,
                units("pc"),
                TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1,
            )
        }
        TerminalMachineSemanticKind::ReturnI64 => (
            units("x30"),
            units("pc"),
            TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            TerminalMachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                target: view("x30"),
            },
        ),
        _ => (
            vec![],
            vec![],
            TerminalMachineEncodedTrapBehavior::NeverV1,
            TerminalMachineEncodedControlEffect::FallThroughV1,
        ),
    };
    TerminalMachineEncodedEffects {
        external_operand_reads: reads,
        external_operand_writes: writes,
        implicit_unit_uses: implicit_uses,
        implicit_unit_defs: implicit_defs,
        implicit_unit_clobbers: vec![],
        memory: TerminalMachineEncodedMemoryEffect::NoneV1,
        stack: TerminalMachineEncodedStackEffect::UnchangedV1,
        trap,
        control,
    }
}

const fn size(semantic: TerminalMachineSemanticKind) -> TerminalMachineSizeKnowledge {
    match semantic {
        TerminalMachineSemanticKind::MaterializeI64 => {
            TerminalMachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(16),
            }
        }
        _ => TerminalMachineSizeKnowledge::ExactBytes(4),
    }
}

#[cfg(test)]
mod tests {
    use omega_register_model::validate_physical_register_model;

    use super::*;
    use crate::{
        Aarch64RegisterConstraintCatalogValidationError, aarch64_physical_register_model,
        aarch64_register_constraint_catalog, validate_aarch64_register_constraint_catalog,
    };

    fn constraints() -> ValidatedRegisterConstraintCatalog {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        validate_aarch64_register_constraint_catalog(
            aarch64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap_or_else(|error: Aarch64RegisterConstraintCatalogValidationError| panic!("{error}"))
    }

    #[test]
    fn catalog_declares_one_flag_transparent_subtraction_alternative() {
        for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
            let constraints = constraints();
            let catalog = aarch64_terminal_machine_effect_catalog(target, &constraints).unwrap();
            let subtract = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == TerminalMachineSemanticKind::ExactSubtractI64)
                .unwrap();
            assert_eq!(subtract.constraint, AARCH64_SUBTRACT_I64);
            let register_effects = constraints
                .catalog()
                .constraints
                .iter()
                .find(|row| row.key == subtract.constraint)
                .unwrap();
            assert!(register_effects.implicit_uses.is_empty());
            assert!(register_effects.implicit_defs.is_empty());
            assert!(register_effects.clobbers.is_empty());
            assert_eq!(subtract.alternatives.len(), 1);
            assert_eq!(
                subtract.alternatives[0].applicability,
                TerminalMachineAlternativeApplicability::Always
            );
            assert_eq!(
                subtract.alternatives[0].size,
                TerminalMachineSizeKnowledge::ExactBytes(4)
            );
            assert!(
                validate_aarch64_terminal_machine_effect_catalog(target, &constraints, catalog)
                    .is_ok()
            );
        }
    }

    #[test]
    fn semantic_validator_rejects_invented_alias_or_size_semantics() {
        let target = NativeTarget::linux_arm64();
        let constraints = constraints();
        let mut wrong = aarch64_terminal_machine_effect_catalog(target, &constraints).unwrap();
        let subtract = wrong
            .declarations
            .iter_mut()
            .find(|row| row.semantic == TerminalMachineSemanticKind::ExactSubtractI64)
            .unwrap();
        subtract.alternatives[0].applicability =
            TerminalMachineAlternativeApplicability::ResultAliasesOperand {
                result: 2,
                operand: 0,
            };
        assert_eq!(
            validate_aarch64_terminal_machine_effect_catalog(target, &constraints, wrong),
            Err(Aarch64TerminalMachineEffectCatalogValidationError::TargetSemanticMismatch)
        );

        let mut wrong = aarch64_terminal_machine_effect_catalog(target, &constraints).unwrap();
        wrong
            .declarations
            .iter_mut()
            .find(|row| row.semantic == TerminalMachineSemanticKind::MaterializeI64)
            .unwrap()
            .alternatives[0]
            .size = TerminalMachineSizeKnowledge::ExactBytes(4);
        assert_eq!(
            validate_aarch64_terminal_machine_effect_catalog(target, &constraints, wrong),
            Err(Aarch64TerminalMachineEffectCatalogValidationError::TargetSemanticMismatch)
        );
    }
}
