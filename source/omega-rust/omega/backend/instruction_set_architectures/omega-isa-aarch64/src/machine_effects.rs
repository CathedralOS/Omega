use omega_register_model::ValidatedRegisterConstraintCatalog;
use omega_selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
    MachineCallEffect, MachineCleanupEffect, MachineEffectCatalog,
    MachineEffectCatalogValidationError, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior, SelectedConstraintKeys,
    ValidatedMachineEffectCatalog, validate_machine_effect_catalog,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};

mod scalar_call;

use scalar_call::declaration as scalar_call_declaration;

use crate::{
    AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, AARCH64_AAPCS64_RETURN, AARCH64_AAPCS64_RETURN_UNIT,
    AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64, AARCH64_COMPARE_I64_ZERO,
    AARCH64_CONDITIONAL_BRANCH, AARCH64_COPY_I64, AARCH64_DARWIN_RETURN,
    AARCH64_DARWIN_RETURN_UNIT, AARCH64_MATERIALIZE_I64, AARCH64_SUBTRACT_I64,
    AARCH64_SUBTRACT_I64_IMMEDIATE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64MachineEffectCatalogValidationError {
    TargetArchitectureMismatch,
    UnsupportedTargetAbi,
    Structural(MachineEffectCatalogValidationError),
    TargetSemanticMismatch,
}

impl std::fmt::Display for Aarch64MachineEffectCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid AArch64 machine effects: {self:?}")
    }
}

impl std::error::Error for Aarch64MachineEffectCatalogValidationError {}

pub fn aarch64_machine_effect_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<MachineEffectCatalog, Aarch64MachineEffectCatalogValidationError> {
    if target.architecture != Architecture::Aarch64
        || constraints.architecture() != Architecture::Aarch64
    {
        return Err(Aarch64MachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    let selected_keys = selected_keys(target)?;
    Ok(MachineEffectCatalog {
        target,
        register_constraints: constraints.identity(),
        selected_keys,
        structural_unit_call: None,
        declarations: MachineSemanticKind::ALL
            .into_iter()
            .filter_map(|semantic| {
                selected_keys
                    .for_semantic(semantic)
                    .map(|_| declaration(semantic, selected_keys, constraints))
            })
            .collect(),
    })
}

pub fn validate_aarch64_machine_effect_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: MachineEffectCatalog,
) -> Result<ValidatedMachineEffectCatalog, Aarch64MachineEffectCatalogValidationError> {
    if target.architecture != Architecture::Aarch64
        || constraints.architecture() != Architecture::Aarch64
    {
        return Err(Aarch64MachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    let canonical = aarch64_machine_effect_catalog(target, constraints)?;
    let validated = validate_machine_effect_catalog(constraints, catalog)
        .map_err(Aarch64MachineEffectCatalogValidationError::Structural)?;
    if validated.catalog() != &canonical {
        return Err(Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch);
    }
    Ok(validated)
}

fn selected_keys(
    target: NativeTarget,
) -> Result<SelectedConstraintKeys, Aarch64MachineEffectCatalogValidationError> {
    let return_i64 = match target.object_format {
        ObjectFormat::Elf => AARCH64_AAPCS64_RETURN,
        ObjectFormat::MachO => AARCH64_DARWIN_RETURN,
        ObjectFormat::Coff => {
            return Err(Aarch64MachineEffectCatalogValidationError::UnsupportedTargetAbi);
        }
    };
    let return_unit = match target.object_format {
        ObjectFormat::Elf => AARCH64_AAPCS64_RETURN_UNIT,
        ObjectFormat::MachO => AARCH64_DARWIN_RETURN_UNIT,
        ObjectFormat::Coff => {
            return Err(Aarch64MachineEffectCatalogValidationError::UnsupportedTargetAbi);
        }
    };
    Ok(SelectedConstraintKeys {
        structural_unit_call: None,
        call_i64_2_u64_to_u64: matches!(target.object_format, ObjectFormat::Elf)
            .then_some(AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64),
        materialize_i64: AARCH64_MATERIALIZE_I64,
        copy_i64: AARCH64_COPY_I64,
        add_i64: AARCH64_ADD_I64,
        subtract_i64: AARCH64_SUBTRACT_I64,
        add_i64_immediate: AARCH64_ADD_I64_IMMEDIATE,
        subtract_i64_immediate: AARCH64_SUBTRACT_I64_IMMEDIATE,
        compare_i64_zero: AARCH64_COMPARE_I64_ZERO,
        compare_i64: AARCH64_COMPARE_I64,
        conditional_branch: AARCH64_CONDITIONAL_BRANCH,
        return_i64,
        return_unit,
    })
}

fn declaration(
    semantic: MachineSemanticKind,
    keys: SelectedConstraintKeys,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> MachineEffectDeclaration {
    if semantic == MachineSemanticKind::CallI64 {
        return scalar_call_declaration(keys, constraints);
    }
    MachineEffectDeclaration {
        semantic,
        constraint: keys
            .for_semantic(semantic)
            .expect("required AArch64 machine semantic has a constraint"),
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::NeverV1,
        barrier: if matches!(
            semantic,
            MachineSemanticKind::ConditionalBranchNonZero
                | MachineSemanticKind::ConditionalBranchU64LessThan
                | MachineSemanticKind::ConditionalBranchI64LessThan
                | MachineSemanticKind::ReturnI64
                | MachineSemanticKind::ReturnUnit
        ) {
            MachineBarrier::ControlFlow
        } else {
            MachineBarrier::None
        },
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: size(semantic),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: encoded_effects(semantic),
        }],
    }
}

fn encoded_effects(semantic: MachineSemanticKind) -> MachineEncodedEffects {
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
        MachineSemanticKind::CompareI64Zero => (vec![0], vec![]),
        MachineSemanticKind::CompareI64 => (vec![0, 1], vec![]),
        MachineSemanticKind::MaterializeI64 => (vec![], vec![0]),
        MachineSemanticKind::CopyI64 => (vec![0], vec![1]),
        MachineSemanticKind::ExactAddI64 | MachineSemanticKind::ExactSubtractI64 => {
            (vec![0, 1], vec![2])
        }
        MachineSemanticKind::ExactAddI64Immediate
        | MachineSemanticKind::ExactSubtractI64Immediate => (vec![0], vec![1]),
        MachineSemanticKind::ConditionalBranchNonZero
        | MachineSemanticKind::ConditionalBranchU64LessThan
        | MachineSemanticKind::ConditionalBranchI64LessThan
        | MachineSemanticKind::ReturnI64
        | MachineSemanticKind::ReturnUnit => (vec![], vec![]),
        MachineSemanticKind::CallI64 => {
            panic!("scalar calls use their dedicated declaration")
        }
    };
    let (implicit_uses, implicit_defs, trap, control) = match semantic {
        MachineSemanticKind::CompareI64Zero | MachineSemanticKind::CompareI64 => (
            vec![],
            units("nzcv"),
            MachineEncodedTrapBehavior::NeverV1,
            MachineEncodedControlEffect::FallThroughV1,
        ),
        MachineSemanticKind::ConditionalBranchNonZero
        | MachineSemanticKind::ConditionalBranchU64LessThan
        | MachineSemanticKind::ConditionalBranchI64LessThan => {
            let mut uses = units("nzcv");
            uses.extend(units("pc"));
            uses.sort_unstable();
            uses.dedup();
            (
                uses,
                units("pc"),
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1,
            )
        }
        MachineSemanticKind::ReturnI64 | MachineSemanticKind::ReturnUnit => (
            units("x30"),
            units("pc"),
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                target: view("x30"),
            },
        ),
        _ => (
            vec![],
            vec![],
            MachineEncodedTrapBehavior::NeverV1,
            MachineEncodedControlEffect::FallThroughV1,
        ),
    };
    MachineEncodedEffects {
        external_operand_reads: reads,
        external_operand_writes: writes,
        implicit_unit_uses: implicit_uses,
        implicit_unit_defs: implicit_defs,
        implicit_unit_clobbers: vec![],
        memory: MachineEncodedMemoryEffect::NoneV1,
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap,
        control,
    }
}

const fn size(semantic: MachineSemanticKind) -> MachineSizeKnowledge {
    match semantic {
        MachineSemanticKind::MaterializeI64 => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 4,
            maximum_bytes: Some(16),
        },
        MachineSemanticKind::CallI64 => {
            panic!("scalar calls use their dedicated declaration")
        }
        _ => MachineSizeKnowledge::ExactBytes(4),
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
            let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
            let subtract = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::ExactSubtractI64)
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
                MachineAlternativeApplicability::Always
            );
            assert_eq!(
                subtract.alternatives[0].size,
                MachineSizeKnowledge::ExactBytes(4)
            );
            let less_than_branch = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::ConditionalBranchU64LessThan)
                .unwrap();
            let signed_less_than_branch = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::ConditionalBranchI64LessThan)
                .unwrap();
            assert_eq!(less_than_branch.constraint, AARCH64_CONDITIONAL_BRANCH);
            assert_eq!(less_than_branch.barrier, MachineBarrier::ControlFlow);
            assert_eq!(less_than_branch.alternatives.len(), 1);
            assert_eq!(
                less_than_branch.alternatives[0].size,
                MachineSizeKnowledge::ExactBytes(4)
            );
            assert_eq!(
                less_than_branch.alternatives[0].encoded.control,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
            assert_eq!(
                signed_less_than_branch.constraint,
                AARCH64_CONDITIONAL_BRANCH
            );
            assert_eq!(signed_less_than_branch.alternatives.len(), 1);
            assert_eq!(
                signed_less_than_branch.alternatives[0].key.family,
                omega_selected_instructions::MachineAlternativeFamily::ConditionalBranchI64LessThan
            );
            assert_eq!(
                signed_less_than_branch.alternatives[0].size,
                MachineSizeKnowledge::ExactBytes(4)
            );
            let scalar_call = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::CallI64);
            if target == NativeTarget::linux_arm64() {
                let scalar_call = scalar_call.expect("AAPCS64 target declares scalar call");
                assert_eq!(
                    scalar_call.call,
                    MachineCallEffect::DirectInternalNormalReturnV1 {
                        pre_call_stack_alignment: 16,
                    }
                );
                assert_eq!(
                    scalar_call.alternatives[0].size,
                    MachineSizeKnowledge::ExactBytes(4)
                );
                assert_eq!(
                    scalar_call.alternatives[0].encoded.control,
                    MachineEncodedControlEffect::DirectRelativeCallV1
                );
                assert_eq!(
                    scalar_call.alternatives[0].encoded.stack,
                    MachineEncodedStackEffect::UnchangedV1
                );
            } else {
                assert!(scalar_call.is_none());
            }
            assert!(validate_aarch64_machine_effect_catalog(target, &constraints, catalog).is_ok());
            let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
            let return_unit = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::ReturnUnit)
                .unwrap();
            assert!(
                constraints
                    .catalog()
                    .constraints
                    .iter()
                    .find(|row| row.key == return_unit.constraint)
                    .unwrap()
                    .operands
                    .is_empty()
            );
        }
    }

    #[test]
    fn semantic_validator_rejects_invented_alias_or_size_semantics() {
        let target = NativeTarget::linux_arm64();
        let constraints = constraints();
        let mut wrong = aarch64_machine_effect_catalog(target, &constraints).unwrap();
        let subtract = wrong
            .declarations
            .iter_mut()
            .find(|row| row.semantic == MachineSemanticKind::ExactSubtractI64)
            .unwrap();
        subtract.alternatives[0].applicability =
            MachineAlternativeApplicability::ResultAliasesOperand {
                result: 2,
                operand: 0,
            };
        assert_eq!(
            validate_aarch64_machine_effect_catalog(target, &constraints, wrong),
            Err(Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch)
        );

        let mut wrong = aarch64_machine_effect_catalog(target, &constraints).unwrap();
        wrong
            .declarations
            .iter_mut()
            .find(|row| row.semantic == MachineSemanticKind::MaterializeI64)
            .unwrap()
            .alternatives[0]
            .size = MachineSizeKnowledge::ExactBytes(4);
        assert_eq!(
            validate_aarch64_machine_effect_catalog(target, &constraints, wrong),
            Err(Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch)
        );
    }
}
