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
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH,
    X86_64_COPY_I64, X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_RETURN, X86_64_SUBTRACT_I64,
    X86_64_SUBTRACT_I64_IMMEDIATE, X86_64_SYSTEM_V_RETURN,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64TerminalMachineEffectCatalogValidationError {
    TargetArchitectureMismatch,
    UnsupportedTargetAbi,
    Structural(TerminalMachineEffectCatalogValidationError),
    TargetSemanticMismatch,
}

impl std::fmt::Display for X86_64TerminalMachineEffectCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 Terminal machine effects: {self:?}"
        )
    }
}

impl std::error::Error for X86_64TerminalMachineEffectCatalogValidationError {}

pub fn x86_64_terminal_machine_effect_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalMachineEffectCatalog, X86_64TerminalMachineEffectCatalogValidationError> {
    if target.architecture != Architecture::X86_64
        || constraints.architecture() != Architecture::X86_64
    {
        return Err(X86_64TerminalMachineEffectCatalogValidationError::TargetArchitectureMismatch);
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

pub fn validate_x86_64_terminal_machine_effect_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: TerminalMachineEffectCatalog,
) -> Result<ValidatedTerminalMachineEffectCatalog, X86_64TerminalMachineEffectCatalogValidationError>
{
    if target.architecture != Architecture::X86_64
        || constraints.architecture() != Architecture::X86_64
    {
        return Err(X86_64TerminalMachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    let canonical = x86_64_terminal_machine_effect_catalog(target, constraints)?;
    let validated = validate_terminal_machine_effect_catalog(constraints, catalog)
        .map_err(X86_64TerminalMachineEffectCatalogValidationError::Structural)?;
    if validated.catalog() != &canonical {
        return Err(X86_64TerminalMachineEffectCatalogValidationError::TargetSemanticMismatch);
    }
    Ok(validated)
}

fn selected_keys(
    target: NativeTarget,
) -> Result<TerminalSelectedConstraintKeys, X86_64TerminalMachineEffectCatalogValidationError> {
    let return_i64 = match target.object_format {
        ObjectFormat::Elf => X86_64_SYSTEM_V_RETURN,
        ObjectFormat::Coff => X86_64_MICROSOFT_RETURN,
        ObjectFormat::MachO => {
            return Err(X86_64TerminalMachineEffectCatalogValidationError::UnsupportedTargetAbi);
        }
    };
    Ok(TerminalSelectedConstraintKeys {
        materialize_i64: X86_64_MATERIALIZE_I64,
        copy_i64: X86_64_COPY_I64,
        add_i64: X86_64_ADD_I64,
        subtract_i64: X86_64_SUBTRACT_I64,
        add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
        subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
        compare_i64_zero: X86_64_COMPARE_I64_ZERO,
        conditional_branch: X86_64_CONDITIONAL_BRANCH,
        return_i64,
    })
}

fn declaration(
    semantic: TerminalMachineSemanticKind,
    keys: TerminalSelectedConstraintKeys,
) -> TerminalMachineEffectDeclaration {
    let alternatives = match semantic {
        TerminalMachineSemanticKind::ExactAddI64 => vec![alternative(
            semantic,
            0,
            TerminalMachineAlternativeApplicability::Always,
            size(semantic),
        )],
        TerminalMachineSemanticKind::ExactSubtractI64 => vec![
            alternative(
                semantic,
                0,
                TerminalMachineAlternativeApplicability::ResultAliasesOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                },
                TerminalMachineSizeKnowledge::ExactBytes(3),
            ),
            alternative(
                semantic,
                1,
                TerminalMachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 0,
                    distinct_operand: 1,
                },
                TerminalMachineSizeKnowledge::ExactBytes(3),
            ),
            alternative(
                semantic,
                2,
                TerminalMachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 1,
                    distinct_operand: 0,
                },
                TerminalMachineSizeKnowledge::ExactBytes(6),
            ),
            alternative(
                semantic,
                3,
                TerminalMachineAlternativeApplicability::ResultDistinctFromOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                },
                TerminalMachineSizeKnowledge::ExactBytes(6),
            ),
        ],
        _ => vec![alternative(
            semantic,
            0,
            TerminalMachineAlternativeApplicability::Always,
            size(semantic),
        )],
    };
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
        alternatives,
    }
}

fn alternative(
    semantic: TerminalMachineSemanticKind,
    variant: u32,
    applicability: TerminalMachineAlternativeApplicability,
    size: TerminalMachineSizeKnowledge,
) -> TerminalMachineAlternative {
    TerminalMachineAlternative {
        key: TerminalMachineAlternativeKey {
            family: semantic.into(),
            variant,
        },
        applicability,
        size,
        latency: TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
        encoded: encoded_effects(semantic, variant),
    }
}

fn encoded_effects(
    semantic: TerminalMachineSemanticKind,
    variant: u32,
) -> TerminalMachineEncodedEffects {
    let physical = crate::x86_64_physical_register_model();
    let units = |name: &str| {
        physical
            .view_named(name)
            .unwrap_or_else(|| panic!("canonical x86-64 model declares {name}"))
            .units
            .clone()
    };
    let view = |name: &str| {
        physical
            .view_named(name)
            .unwrap_or_else(|| panic!("canonical x86-64 model declares {name}"))
            .id
    };
    let (reads, writes) = match semantic {
        TerminalMachineSemanticKind::CompareI64Zero => (vec![0], vec![]),
        TerminalMachineSemanticKind::MaterializeI64 => (vec![], vec![0]),
        TerminalMachineSemanticKind::CopyI64 => (vec![0], vec![1]),
        TerminalMachineSemanticKind::ExactAddI64 => (vec![0, 1], vec![2]),
        TerminalMachineSemanticKind::ExactAddI64Immediate
        | TerminalMachineSemanticKind::ExactSubtractI64Immediate => (vec![0], vec![1]),
        TerminalMachineSemanticKind::ExactSubtractI64 if variant == 0 => (vec![], vec![2]),
        TerminalMachineSemanticKind::ExactSubtractI64 => (vec![0, 1], vec![2]),
        TerminalMachineSemanticKind::ConditionalBranchNonZero
        | TerminalMachineSemanticKind::ReturnI64 => (vec![], vec![]),
    };
    let (implicit_uses, implicit_defs, implicit_clobbers, memory, stack, trap, control) =
        match semantic {
            TerminalMachineSemanticKind::CompareI64Zero => (
                vec![],
                units("rflags"),
                vec![],
                TerminalMachineEncodedMemoryEffect::NoneV1,
                TerminalMachineEncodedStackEffect::UnchangedV1,
                TerminalMachineEncodedTrapBehavior::NeverV1,
                TerminalMachineEncodedControlEffect::FallThroughV1,
            ),
            TerminalMachineSemanticKind::ExactSubtractI64 => (
                vec![],
                vec![],
                units("rflags"),
                TerminalMachineEncodedMemoryEffect::NoneV1,
                TerminalMachineEncodedStackEffect::UnchangedV1,
                TerminalMachineEncodedTrapBehavior::NeverV1,
                TerminalMachineEncodedControlEffect::FallThroughV1,
            ),
            TerminalMachineSemanticKind::ConditionalBranchNonZero => {
                let mut uses = units("rflags");
                uses.extend(units("rip"));
                uses.sort_unstable();
                uses.dedup();
                (
                    uses,
                    units("rip"),
                    vec![],
                    TerminalMachineEncodedMemoryEffect::NoneV1,
                    TerminalMachineEncodedStackEffect::UnchangedV1,
                    TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1,
                )
            }
            TerminalMachineSemanticKind::ReturnI64 => {
                let stack_pointer = view("rsp");
                let mut defs = units("rsp");
                defs.extend(units("rip"));
                defs.sort_unstable();
                defs.dedup();
                (
                    units("rsp"),
                    defs,
                    vec![],
                    TerminalMachineEncodedMemoryEffect::ReadActivationStackV1 {
                        stack_pointer,
                        byte_count: 8,
                    },
                    TerminalMachineEncodedStackEffect::PopBytesV1 {
                        stack_pointer,
                        byte_count: 8,
                    },
                    TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    TerminalMachineEncodedControlEffect::ReturnFromActivationStackV1,
                )
            }
            _ => (
                vec![],
                vec![],
                vec![],
                TerminalMachineEncodedMemoryEffect::NoneV1,
                TerminalMachineEncodedStackEffect::UnchangedV1,
                TerminalMachineEncodedTrapBehavior::NeverV1,
                TerminalMachineEncodedControlEffect::FallThroughV1,
            ),
        };
    TerminalMachineEncodedEffects {
        external_operand_reads: reads,
        external_operand_writes: writes,
        implicit_unit_uses: implicit_uses,
        implicit_unit_defs: implicit_defs,
        implicit_unit_clobbers: implicit_clobbers,
        memory,
        stack,
        trap,
        control,
    }
}

fn size(semantic: TerminalMachineSemanticKind) -> TerminalMachineSizeKnowledge {
    match semantic {
        TerminalMachineSemanticKind::CompareI64Zero | TerminalMachineSemanticKind::CopyI64 => {
            TerminalMachineSizeKnowledge::ExactBytes(3)
        }
        TerminalMachineSemanticKind::MaterializeI64 => TerminalMachineSizeKnowledge::ExactBytes(10),
        TerminalMachineSemanticKind::ExactAddI64 => TerminalMachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 4,
            maximum_bytes: Some(5),
        },
        TerminalMachineSemanticKind::ExactAddI64Immediate => {
            TerminalMachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(8),
            }
        }
        TerminalMachineSemanticKind::ExactSubtractI64Immediate => {
            TerminalMachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(8),
            }
        }
        TerminalMachineSemanticKind::ConditionalBranchNonZero => {
            TerminalMachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 2,
                maximum_bytes: Some(6),
            }
        }
        TerminalMachineSemanticKind::ReturnI64 => TerminalMachineSizeKnowledge::ExactBytes(1),
        TerminalMachineSemanticKind::ExactSubtractI64 => {
            unreachable!("subtraction declares alias-dependent alternatives")
        }
    }
}

#[cfg(test)]
mod tests {
    use omega_register_model::validate_physical_register_model;

    use super::*;
    use crate::{
        X86_64RegisterConstraintCatalogValidationError,
        validate_x86_64_register_constraint_catalog, x86_64_physical_register_model,
        x86_64_register_constraint_catalog,
    };

    fn constraints() -> ValidatedRegisterConstraintCatalog {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        validate_x86_64_register_constraint_catalog(
            x86_64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap_or_else(|error: X86_64RegisterConstraintCatalogValidationError| panic!("{error}"))
    }

    #[test]
    fn catalog_declares_alias_safe_subtraction_and_control_barriers() {
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let constraints = constraints();
            let catalog = x86_64_terminal_machine_effect_catalog(target, &constraints).unwrap();
            let subtract = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == TerminalMachineSemanticKind::ExactSubtractI64)
                .unwrap();
            let add = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == TerminalMachineSemanticKind::ExactAddI64)
                .unwrap();
            assert_eq!(
                add.alternatives[0].applicability,
                TerminalMachineAlternativeApplicability::Always
            );
            assert_eq!(subtract.constraint, X86_64_SUBTRACT_I64);
            let register_effects = constraints
                .catalog()
                .constraints
                .iter()
                .find(|row| row.key == subtract.constraint)
                .unwrap();
            assert!(register_effects.implicit_uses.is_empty());
            assert!(register_effects.implicit_defs.is_empty());
            assert!(!register_effects.clobbers.is_empty());
            assert_eq!(subtract.alternatives.len(), 4);
            assert_eq!(
                subtract.alternatives[0].applicability,
                TerminalMachineAlternativeApplicability::ResultAliasesOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                }
            );
            assert_eq!(
                subtract.alternatives[1].applicability,
                TerminalMachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 0,
                    distinct_operand: 1,
                }
            );
            assert_eq!(
                subtract.alternatives[2].applicability,
                TerminalMachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 1,
                    distinct_operand: 0,
                }
            );
            assert_eq!(
                subtract.alternatives[3].applicability,
                TerminalMachineAlternativeApplicability::ResultDistinctFromOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                }
            );
            assert!(catalog.declarations.iter().all(|row| {
                row.barrier
                    == if matches!(
                        row.semantic,
                        TerminalMachineSemanticKind::ConditionalBranchNonZero
                            | TerminalMachineSemanticKind::ReturnI64
                    ) {
                        TerminalMachineBarrier::ControlFlow
                    } else {
                        TerminalMachineBarrier::None
                    }
            }));
            assert!(
                validate_x86_64_terminal_machine_effect_catalog(target, &constraints, catalog)
                    .is_ok()
            );
        }
    }

    #[test]
    fn semantic_validator_rejects_structural_and_target_specific_corruption() {
        let target = NativeTarget::linux_x64();
        let constraints = constraints();
        let mut missing = x86_64_terminal_machine_effect_catalog(target, &constraints).unwrap();
        missing.declarations.pop();
        assert!(matches!(
            validate_x86_64_terminal_machine_effect_catalog(target, &constraints, missing),
            Err(
                X86_64TerminalMachineEffectCatalogValidationError::Structural(
                    TerminalMachineEffectCatalogValidationError::DeclarationRosterMismatch
                )
            )
        ));

        let mut wrong_size = x86_64_terminal_machine_effect_catalog(target, &constraints).unwrap();
        wrong_size
            .declarations
            .iter_mut()
            .find(|row| row.semantic == TerminalMachineSemanticKind::ExactSubtractI64)
            .unwrap()
            .alternatives[0]
            .size = TerminalMachineSizeKnowledge::ExactBytes(4);
        assert_eq!(
            validate_x86_64_terminal_machine_effect_catalog(target, &constraints, wrong_size),
            Err(X86_64TerminalMachineEffectCatalogValidationError::TargetSemanticMismatch)
        );

        let mut bad_alias = x86_64_terminal_machine_effect_catalog(target, &constraints).unwrap();
        bad_alias
            .declarations
            .iter_mut()
            .find(|row| row.semantic == TerminalMachineSemanticKind::ExactSubtractI64)
            .unwrap()
            .alternatives[0]
            .applicability = TerminalMachineAlternativeApplicability::ResultAliasesOperand {
            result: 9,
            operand: 0,
        };
        assert!(matches!(
            validate_x86_64_terminal_machine_effect_catalog(target, &constraints, bad_alias),
            Err(
                X86_64TerminalMachineEffectCatalogValidationError::Structural(
                    TerminalMachineEffectCatalogValidationError::InvalidAlternativeApplicability(
                        TerminalMachineSemanticKind::ExactSubtractI64
                    )
                )
            )
        ));
    }
}
