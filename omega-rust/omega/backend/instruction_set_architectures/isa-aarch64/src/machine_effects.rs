use register_model::ValidatedRegisterConstraintCatalog;
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
    MachineCallEffect, MachineCleanupEffect, MachineEffectCatalog,
    MachineEffectCatalogValidationError, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior, SelectedConstraintKeys,
    ValidatedMachineEffectCatalog, validate_machine_effect_catalog,
};
use target::{Architecture, NativeTarget, ObjectFormat};

mod scalar_call;

use scalar_call::declaration as scalar_call_declaration;

use crate::{
    AARCH64_AAPCS64_RETURN, AARCH64_AAPCS64_RETURN_UNIT, AARCH64_ADD_I64,
    AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64, AARCH64_COMPARE_I64_ZERO,
    AARCH64_CONDITIONAL_BRANCH, AARCH64_COPY_I64, AARCH64_DARWIN_RETURN,
    AARCH64_DARWIN_RETURN_UNIT, AARCH64_MATERIALIZE_I64, AARCH64_SUBTRACT_I64,
    AARCH64_SUBTRACT_I64_IMMEDIATE, aarch64_aapcs64_register_call_keys,
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
        selected_keys: selected_keys.clone(),
        declarations: selected_keys
            .declaration_keys()
            .into_iter()
            .map(|(semantic, constraint)| {
                if semantic == MachineSemanticKind::HostedExitProcessI32 {
                    return crate::selected_form_encoding::hosted_exit_process::declaration(
                        target, constraint,
                    )
                    .map_err(|_| {
                        Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch
                    });
                }
                if semantic == MachineSemanticKind::HostedReadByte {
                    return Ok(
                        crate::selected_form_encoding::hosted_read_byte::declaration(
                            target, constraint,
                        ),
                    );
                }
                if semantic == MachineSemanticKind::HostedWriteByteI32 {
                    return Ok(
                        crate::selected_form_encoding::hosted_write_byte::declaration(
                            target, constraint,
                        ),
                    );
                }
                Ok(
                    if matches!(
                        semantic,
                        MachineSemanticKind::CallI64 | MachineSemanticKind::CallUnit
                    ) {
                        scalar_call_declaration(semantic, constraint, constraints)
                    } else {
                        declaration(semantic, &selected_keys)
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
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
        hosted_read_byte: if target == NativeTarget::linux_arm64() {
            Some(crate::AARCH64_HOSTED_READ_BYTE)
        } else if target == NativeTarget::macos_arm64() {
            Some(crate::AARCH64_DARWIN_HOSTED_READ_BYTE)
        } else {
            None
        },
        hosted_exit_process_i32: if target == NativeTarget::linux_arm64() {
            Some(crate::AARCH64_HOSTED_EXIT_PROCESS_I32)
        } else if target == NativeTarget::macos_arm64() {
            Some(crate::AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32)
        } else {
            None
        },
        hosted_write_byte_i32: if target == NativeTarget::linux_arm64() {
            Some(crate::AARCH64_HOSTED_WRITE_BYTE_I32)
        } else if target == NativeTarget::macos_arm64() {
            Some(crate::AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32)
        } else {
            None
        },
        load64: Some(crate::AARCH64_LOAD64),
        load32: Some(crate::AARCH64_LOAD32),
        load8_indexed: Some(crate::AARCH64_LOAD8_INDEXED),
        store: Some(crate::AARCH64_STORE),
        address_offset: Some(crate::AARCH64_ADDRESS_OFFSET),
        store64: Some(crate::AARCH64_STORE64),
        frame_address: Some(crate::AARCH64_FRAME_ADDRESS),
        call_unit_mixed: if target.object_format == ObjectFormat::Elf {
            crate::aarch64_aapcs64_mixed_unit_call_keys()
        } else {
            crate::aarch64_darwin_mixed_unit_call_keys()
        },
        call_unit: if target.object_format == ObjectFormat::Elf {
            crate::aarch64_aapcs64_register_unit_call_keys()
        } else {
            crate::aarch64_darwin_register_unit_call_keys()
        },
        call_i64: if matches!(target.object_format, ObjectFormat::Elf) {
            aarch64_aapcs64_register_call_keys()
        } else {
            crate::aarch64_darwin_register_call_keys()
        },
        materialize_i64: AARCH64_MATERIALIZE_I64,
        copy_i64: AARCH64_COPY_I64,
        float32_to_bits: Some(crate::AARCH64_FLOAT32_TO_BITS),
        float64_to_bits: Some(crate::AARCH64_FLOAT64_TO_BITS),
        bits_to_float32: Some(crate::AARCH64_BITS_TO_FLOAT32),
        bits_to_float64: Some(crate::AARCH64_BITS_TO_FLOAT64),
        add_i64: AARCH64_ADD_I64,
        subtract_i64: AARCH64_SUBTRACT_I64,
        add_i64_immediate: AARCH64_ADD_I64_IMMEDIATE,
        subtract_i64_immediate: AARCH64_SUBTRACT_I64_IMMEDIATE,
        compare_i64_zero: AARCH64_COMPARE_I64_ZERO,
        compare_i64: AARCH64_COMPARE_I64,
        conditional_branch: AARCH64_CONDITIONAL_BRANCH,
        jump: crate::AARCH64_JUMP,
        return_i64,
        return_unit,
    })
}

fn declaration(
    semantic: MachineSemanticKind,
    keys: &SelectedConstraintKeys,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic,
        constraint: keys
            .for_semantic(semantic)
            .expect("required AArch64 machine semantic has a constraint"),
        memory: if matches!(
            semantic,
            MachineSemanticKind::Load32
                | MachineSemanticKind::Load64
                | MachineSemanticKind::Load8Indexed
        ) {
            MachineMemoryEffect::ReadPointerV1
        } else if semantic == MachineSemanticKind::Store64 {
            MachineMemoryEffect::WriteFrameStorageV1
        } else if semantic == MachineSemanticKind::Store {
            MachineMemoryEffect::WritePointerV1
        } else {
            MachineMemoryEffect::NoneV1
        },
        trap: if matches!(
            semantic,
            MachineSemanticKind::Load32
                | MachineSemanticKind::Load64
                | MachineSemanticKind::Load8Indexed
                | MachineSemanticKind::Store64
                | MachineSemanticKind::Store
        ) {
            MachineTrapBehavior::MayArchitecturalFaultV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        barrier: if matches!(
            semantic,
            MachineSemanticKind::ConditionalBranchNonZero
                | MachineSemanticKind::ConditionalBranchU64LessThan
                | MachineSemanticKind::ConditionalBranchI64LessThan
                | MachineSemanticKind::ReturnI64
                | MachineSemanticKind::Jump
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
        MachineSemanticKind::Load8Indexed => (vec![0, 1], vec![2]),
        MachineSemanticKind::CompareI64Zero => (vec![0], vec![]),
        MachineSemanticKind::CompareI64 => (vec![0, 1], vec![]),
        MachineSemanticKind::MaterializeI64 => (vec![], vec![0]),
        MachineSemanticKind::Float32ToBits
        | MachineSemanticKind::Float64ToBits
        | MachineSemanticKind::BitsToFloat32
        | MachineSemanticKind::BitsToFloat64
        | MachineSemanticKind::CopyI64
        | MachineSemanticKind::Load32
        | MachineSemanticKind::Load64
        | MachineSemanticKind::AddressOffset
        | MachineSemanticKind::ZeroExtendU8
        | MachineSemanticKind::ZeroExtendU32 => (vec![0], vec![1]),
        MachineSemanticKind::ByteViewAddress
        | MachineSemanticKind::ExactAddI64
        | MachineSemanticKind::ExactSubtractI64 => (vec![0, 1], vec![2]),
        MachineSemanticKind::ExactAddI64Immediate
        | MachineSemanticKind::ExactSubtractI64Immediate => (vec![0], vec![1]),
        MachineSemanticKind::ConditionalBranchNonZero
        | MachineSemanticKind::ConditionalBranchU64LessThan
        | MachineSemanticKind::ConditionalBranchI64LessThan
        | MachineSemanticKind::ReturnI64
        | MachineSemanticKind::Jump
        | MachineSemanticKind::ReturnUnit => (vec![], vec![]),
        MachineSemanticKind::Store64 => (vec![0], vec![]),
        MachineSemanticKind::Store => (vec![0, 1], vec![]),
        MachineSemanticKind::FrameAddress => (vec![], vec![0]),
        MachineSemanticKind::HostedExitProcessI32
        | MachineSemanticKind::HostedReadByte
        | MachineSemanticKind::HostedWriteByteI32
        | MachineSemanticKind::CallUnit => {
            panic!("memory and Unit call forms are not admitted on this target")
        }
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
        MachineSemanticKind::Jump => (
            units("pc"),
            units("pc"),
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
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
        MachineSemanticKind::Load32
        | MachineSemanticKind::Load64
        | MachineSemanticKind::Load8Indexed
        | MachineSemanticKind::Store => (
            vec![],
            vec![],
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            MachineEncodedControlEffect::FallThroughV1,
        ),
        MachineSemanticKind::Store64 | MachineSemanticKind::FrameAddress => (
            units("sp"),
            vec![],
            if semantic == MachineSemanticKind::Store64 {
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            } else {
                MachineEncodedTrapBehavior::NeverV1
            },
            MachineEncodedControlEffect::FallThroughV1,
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
        memory: if matches!(
            semantic,
            MachineSemanticKind::Load32 | MachineSemanticKind::Load64
        ) {
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: if semantic == MachineSemanticKind::Load32 {
                    4
                } else {
                    8
                },
            }
        } else if semantic == MachineSemanticKind::Load8Indexed {
            MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                pointer_operand: 0,
                index_operand: 1,
                byte_count: 1,
            }
        } else if semantic == MachineSemanticKind::Store64 {
            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                stack_pointer: view("sp"),
                byte_count: 8,
            }
        } else if semantic == MachineSemanticKind::Store {
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
        } else {
            MachineEncodedMemoryEffect::NoneV1
        },
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
        MachineSemanticKind::HostedExitProcessI32
        | MachineSemanticKind::HostedReadByte
        | MachineSemanticKind::HostedWriteByteI32
        | MachineSemanticKind::CallUnit => {
            panic!("memory and Unit call forms are not admitted on this target")
        }
        MachineSemanticKind::CallI64 => {
            panic!("scalar calls use their dedicated declaration")
        }
        _ => MachineSizeKnowledge::ExactBytes(4),
    }
}

#[cfg(test)]
mod tests {
    use register_model::validate_physical_register_model;

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
                selected_instructions::MachineAlternativeFamily::ConditionalBranchI64LessThan
            );
            assert_eq!(
                signed_less_than_branch.alternatives[0].size,
                MachineSizeKnowledge::ExactBytes(4)
            );
            let scalar_call = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::CallI64);
            {
                let scalar_call = scalar_call.expect("supported target declares scalar call");
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
