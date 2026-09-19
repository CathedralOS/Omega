//! Machine effect catalog tests.

use super::{
    AARCH64_AAPCS64_RETURN, AARCH64_AAPCS64_RETURN_UNIT, AARCH64_CONDITIONAL_BRANCH,
    AARCH64_COPY_I64, AARCH64_DARWIN_RETURN, AARCH64_DARWIN_RETURN_UNIT, AARCH64_SUBTRACT_I64,
    Aarch64MachineEffectCatalogValidationError, Architecture, MachineAlternativeApplicability,
    MachineBarrier, MachineCallEffect, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedStackEffect, MachineSemanticKind, MachineSizeKnowledge, NativeTarget,
    ObjectFormat, ValidatedRegisterConstraintCatalog, aarch64_machine_effect_catalog,
    validate_aarch64_machine_effect_catalog,
};
use crate::{
    Aarch64RegisterConstraintCatalogValidationError, aarch64_physical_register_model,
    aarch64_register_constraint_catalog, validate_aarch64_register_constraint_catalog,
};
use register_model::validate_physical_register_model;

fn constraints() -> ValidatedRegisterConstraintCatalog {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    validate_aarch64_register_constraint_catalog(
        aarch64_register_constraint_catalog(&physical),
        &physical,
    )
    .unwrap_or_else(|error: Aarch64RegisterConstraintCatalogValidationError| panic!("{error}"))
}

#[test]
fn every_saturating_carrier_binds_its_shape_key_size_and_flags() {
    use selected_instructions::{SaturatingCarrier, SaturatingOperation};
    let constraints = constraints();
    let nzcv = crate::aarch64_physical_register_model()
        .view_named("nzcv")
        .unwrap()
        .units
        .clone();
    for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
        let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
        for carrier in SaturatingCarrier::ALL {
            let signed = carrier.is_signed();
            let narrow = carrier.is_narrow();
            for (operation, semantic) in [
                (
                    SaturatingOperation::Add,
                    MachineSemanticKind::SaturatingAdd(carrier),
                ),
                (
                    SaturatingOperation::Subtract,
                    MachineSemanticKind::SaturatingSubtract(carrier),
                ),
                (
                    SaturatingOperation::Divide,
                    MachineSemanticKind::SaturatingDivide(carrier),
                ),
                (
                    SaturatingOperation::Remainder,
                    MachineSemanticKind::SaturatingRemainder(carrier),
                ),
            ] {
                // Operand shape selects the constraint row and the word count.
                let (key, size, writes): (_, u16, &[u16]) = match operation {
                    SaturatingOperation::Add if carrier == SaturatingCarrier::U64 => {
                        (crate::register_model::AARCH64_SATURATING_ADD_U64, 8, &[2])
                    }
                    SaturatingOperation::Add => (
                        crate::register_model::AARCH64_SATURATING_ADD_CLAMPED,
                        if !narrow {
                            16
                        } else if signed {
                            28
                        } else {
                            16
                        },
                        &[2, 3],
                    ),
                    SaturatingOperation::Subtract if !signed => (
                        crate::register_model::AARCH64_SATURATING_SUBTRACT_UNSIGNED,
                        8,
                        &[2],
                    ),
                    SaturatingOperation::Subtract => (
                        crate::register_model::AARCH64_SATURATING_SUBTRACT_CLAMPED,
                        if narrow { 28 } else { 16 },
                        &[2, 3],
                    ),
                    SaturatingOperation::Divide if !signed => {
                        (crate::register_model::AARCH64_DIVIDE_U64, 4, &[2])
                    }
                    SaturatingOperation::Divide => (
                        crate::register_model::AARCH64_SATURATING_DIVIDE_SIGNED,
                        if narrow { 16 } else { 24 },
                        &[2, 3],
                    ),
                    // The divide/MSUB remainder pair keeps the plain
                    // three-operand remainder row on both signs.
                    SaturatingOperation::Remainder if !signed => {
                        (crate::register_model::AARCH64_REMAINDER_U64, 8, &[2])
                    }
                    SaturatingOperation::Remainder => {
                        (crate::register_model::AARCH64_REMAINDER_I64, 8, &[2])
                    }
                };
                let declaration = catalog
                    .declarations
                    .iter()
                    .find(|row| row.semantic == semantic)
                    .unwrap_or_else(|| panic!("{semantic:?} declared"));
                assert_eq!(declaration.constraint, key, "{semantic:?}");
                assert_eq!(declaration.alternatives.len(), 1);
                let alternative = &declaration.alternatives[0];
                assert_eq!(
                    alternative.size,
                    MachineSizeKnowledge::ExactBytes(size),
                    "{semantic:?}"
                );
                assert_eq!(alternative.encoded.external_operand_reads, [0, 1]);
                assert_eq!(alternative.encoded.external_operand_writes, writes);
                let defines_nzcv = !(operation == SaturatingOperation::Divide && !signed)
                    && operation != SaturatingOperation::Remainder;
                assert_eq!(
                    alternative.encoded.implicit_unit_defs,
                    if defines_nzcv {
                        nzcv.clone()
                    } else {
                        Vec::new()
                    },
                    "{semantic:?}"
                );
                for corruption in 0..3 {
                    let mut changed = catalog.clone();
                    let alternative = &mut changed
                        .declarations
                        .iter_mut()
                        .find(|row| row.semantic == semantic)
                        .unwrap()
                        .alternatives[0];
                    match corruption {
                        0 => alternative.encoded.external_operand_writes.clear(),
                        1 => alternative.size = MachineSizeKnowledge::ExactBytes(size + 4),
                        _ => {
                            // The sibling width with the other sign has its own
                            // family; the swapped key must be rejected.
                            let sibling = match carrier {
                                SaturatingCarrier::I8 => SaturatingCarrier::U8,
                                SaturatingCarrier::I16 => SaturatingCarrier::U16,
                                SaturatingCarrier::I32 => SaturatingCarrier::U32,
                                SaturatingCarrier::I64 => SaturatingCarrier::U64,
                                SaturatingCarrier::U8 => SaturatingCarrier::I8,
                                SaturatingCarrier::U16 => SaturatingCarrier::I16,
                                SaturatingCarrier::U32 => SaturatingCarrier::I32,
                                SaturatingCarrier::U64 => SaturatingCarrier::I64,
                            };
                            alternative.key.family = match operation {
                                SaturatingOperation::Add => {
                                    MachineSemanticKind::SaturatingAdd(sibling)
                                }
                                SaturatingOperation::Subtract => {
                                    MachineSemanticKind::SaturatingSubtract(sibling)
                                }
                                SaturatingOperation::Divide => {
                                    MachineSemanticKind::SaturatingDivide(sibling)
                                }
                                SaturatingOperation::Remainder => {
                                    MachineSemanticKind::SaturatingRemainder(sibling)
                                }
                            }
                            .into();
                        }
                    };
                    assert!(
                        validate_aarch64_machine_effect_catalog(target, &constraints, changed)
                            .is_err(),
                        "{semantic:?} corruption {corruption}"
                    );
                }
            }
        }
    }
}

#[test]
fn wrapping_remainder_catalog_preserves_inputs_and_flags() {
    let constraints = constraints();
    for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
        let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
        let declaration = catalog
            .declarations
            .iter()
            .find(|row| row.semantic == MachineSemanticKind::WrappingRemainderI64)
            .unwrap();
        assert_eq!(
            declaration.constraint,
            crate::register_model::AARCH64_REMAINDER_I64
        );
        assert_eq!(
            declaration.alternatives[0].size,
            MachineSizeKnowledge::ExactBytes(8)
        );
        assert_eq!(
            declaration.alternatives[0].encoded,
            MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2])
        );
        for corruption in 0..3 {
            let mut changed = catalog.clone();
            let alternative = &mut changed
                .declarations
                .iter_mut()
                .find(|row| row.semantic == MachineSemanticKind::WrappingRemainderI64)
                .unwrap()
                .alternatives[0];
            match corruption {
                0 => alternative.encoded.external_operand_reads.truncate(1),
                1 => alternative.size = MachineSizeKnowledge::ExactBytes(4),
                _ => alternative.key.family = MachineSemanticKind::ExactDivideU64.into(),
            };
            assert!(
                validate_aarch64_machine_effect_catalog(target, &constraints, changed).is_err()
            );
        }
    }
}

#[test]
fn bitwise_xor_catalog_binds_family_size_and_pure_effects() {
    for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
        let constraints = constraints();
        let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
        validate_aarch64_machine_effect_catalog(target, &constraints, catalog.clone()).unwrap();
        let declaration = catalog
            .declarations
            .iter()
            .find(|row| row.semantic == MachineSemanticKind::BitwiseXorI64)
            .unwrap();
        assert_eq!(declaration.alternatives.len(), 1);
        let alternative = &declaration.alternatives[0];
        assert_eq!(
            alternative.key.family,
            MachineSemanticKind::BitwiseXorI64.into()
        );
        assert_eq!(alternative.size, MachineSizeKnowledge::ExactBytes(4));
        assert_eq!(
            alternative.encoded,
            MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2])
        );
        for corruption in 0..3 {
            let mut changed = catalog.clone();
            let alternative = &mut changed
                .declarations
                .iter_mut()
                .find(|row| row.semantic == MachineSemanticKind::BitwiseXorI64)
                .unwrap()
                .alternatives[0];
            match corruption {
                0 => alternative.key.family = MachineSemanticKind::BitwiseAndI64.into(),
                1 => alternative.encoded.external_operand_reads.truncate(1),
                _ => alternative.size = MachineSizeKnowledge::ExactBytes(8),
            }
            assert!(
                validate_aarch64_machine_effect_catalog(target, &constraints, changed).is_err()
            );
        }
    }
}

#[test]
fn integer_normalization_catalog_binds_identity_size_and_effects() {
    for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
        let constraints = constraints();
        let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
        validate_aarch64_machine_effect_catalog(target, &constraints, catalog.clone()).unwrap();
        for semantic in [
            MachineSemanticKind::ZeroExtendU16,
            MachineSemanticKind::SignExtendI8,
            MachineSemanticKind::SignExtendI16,
            MachineSemanticKind::SignExtendI32,
        ] {
            let declaration = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == semantic)
                .unwrap();
            assert_eq!(declaration.constraint, AARCH64_COPY_I64);
            assert_eq!(declaration.alternatives.len(), 1);
            let alternative = &declaration.alternatives[0];
            assert_eq!(alternative.key.family, semantic.into());
            assert_eq!(alternative.size, MachineSizeKnowledge::ExactBytes(4));
            assert_eq!(
                alternative.encoded,
                MachineEncodedEffects::fallthrough_v1(vec![0], vec![1])
            );
            for corruption in 0..3 {
                let mut changed = catalog.clone();
                let row = changed
                    .declarations
                    .iter_mut()
                    .find(|row| row.semantic == semantic)
                    .unwrap();
                match corruption {
                    0 => row.alternatives[0].key.family = MachineSemanticKind::CopyI64.into(),
                    1 => row.alternatives[0].size = MachineSizeKnowledge::ExactBytes(1),
                    _ => row.alternatives[0].encoded.external_operand_reads.clear(),
                }
                assert!(
                    validate_aarch64_machine_effect_catalog(target, &constraints, changed).is_err()
                );
            }
        }
    }
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
            .find(|row| row.semantic == MachineSemanticKind::CallScalar);
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

#[test]
fn selected_keys_declare_the_supported_aarch64_pairs() {
    let constraints = constraints();
    for (target, darwin, return_i64, return_unit, read, exit, write) in [
        (
            NativeTarget::linux_arm64(),
            false,
            AARCH64_AAPCS64_RETURN,
            AARCH64_AAPCS64_RETURN_UNIT,
            crate::AARCH64_HOSTED_READ_BYTE,
            crate::AARCH64_HOSTED_EXIT_PROCESS_I32,
            crate::AARCH64_HOSTED_WRITE_BYTE_I32,
        ),
        (
            NativeTarget::macos_arm64(),
            true,
            AARCH64_DARWIN_RETURN,
            AARCH64_DARWIN_RETURN_UNIT,
            crate::AARCH64_DARWIN_HOSTED_READ_BYTE,
            crate::AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32,
            crate::AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32,
        ),
    ] {
        let keys = aarch64_machine_effect_catalog(target, &constraints)
            .unwrap()
            .selected_keys;
        assert_eq!(keys.return_i64, return_i64);
        assert_eq!(keys.return_unit, return_unit);
        assert_eq!(keys.hosted_read_byte, Some(read));
        assert_eq!(keys.hosted_exit_process_i32, Some(exit));
        assert_eq!(keys.hosted_write_byte_i32, Some(write));
        assert_eq!(
            keys.call_unit,
            if darwin {
                crate::aarch64_darwin_register_unit_call_keys()
            } else {
                crate::aarch64_aapcs64_register_unit_call_keys()
            }
        );
        assert_eq!(
            keys.call_unit_mixed,
            if darwin {
                crate::aarch64_darwin_mixed_unit_call_keys()
            } else {
                crate::aarch64_aapcs64_mixed_unit_call_keys()
            }
        );
        let mut expected_scalar = if darwin {
            crate::aarch64_darwin_register_call_keys()
        } else {
            crate::aarch64_aapcs64_register_call_keys()
        };
        expected_scalar.extend(crate::aarch64_float_scalar_call_keys(darwin));
        assert_eq!(keys.call_scalar, expected_scalar);
        assert_eq!(
            keys.call_aggregate,
            crate::aarch64_register_aggregate_call_keys(darwin)
                .into_iter()
                .chain(crate::aarch64_mixed_aggregate_call_keys(darwin))
                .chain(crate::aarch64_indirect_aggregate_call_keys(darwin))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            keys.return_aggregate,
            crate::aarch64_register_aggregate_return_keys(darwin)
        );
        assert_eq!(
            keys.return_float,
            crate::aarch64_float_scalar_return_keys(darwin)
        );
    }
}

#[test]
fn selected_keys_fail_closed_on_undeclared_pairs() {
    let constraints = constraints();
    let coff = NativeTarget {
        architecture: Architecture::Aarch64,
        object_format: ObjectFormat::Coff,
        pointer_size: 8,
        pointer_alignment: 8,
    };
    assert_eq!(
        aarch64_machine_effect_catalog(coff, &constraints),
        Err(Aarch64MachineEffectCatalogValidationError::UnsupportedTargetAbi)
    );
}

#[test]
fn hosted_rows_follow_the_declared_target_not_the_format_shape() {
    let constraints = constraints();
    // Same-format AArch64 targets that are not the declared Linux or
    // macOS targets claim no hosted syscall rows.
    for (undeclared, return_i64) in [
        (
            NativeTarget {
                pointer_size: 4,
                pointer_alignment: 4,
                ..NativeTarget::linux_arm64()
            },
            AARCH64_AAPCS64_RETURN,
        ),
        (
            NativeTarget {
                pointer_size: 4,
                pointer_alignment: 4,
                ..NativeTarget::macos_arm64()
            },
            AARCH64_DARWIN_RETURN,
        ),
    ] {
        let keys = aarch64_machine_effect_catalog(undeclared, &constraints)
            .unwrap()
            .selected_keys;
        assert_eq!(keys.return_i64, return_i64);
        assert!(keys.hosted_write_byte_i32.is_none());
        assert!(keys.hosted_read_byte.is_none());
        assert!(keys.hosted_exit_process_i32.is_none());
    }
}
