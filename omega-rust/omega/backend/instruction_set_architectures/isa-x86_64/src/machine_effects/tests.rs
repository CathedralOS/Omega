//! Machine effect catalog tests.

use super::{
    Architecture, MachineAlternativeApplicability, MachineBarrier, MachineCallEffect,
    MachineEffectCatalogValidationError, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, NativeTarget, ObjectFormat,
    ValidatedRegisterConstraintCatalog, X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64,
    X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64,
    X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT, X86_64MachineEffectCatalogValidationError,
    validate_x86_64_machine_effect_catalog, x86_64_machine_effect_catalog,
    x86_64_system_v_register_call_keys,
};
use crate::{
    X86_64RegisterConstraintCatalogValidationError, validate_x86_64_register_constraint_catalog,
    x86_64_physical_register_model, x86_64_register_constraint_catalog,
};
use register_model::validate_physical_register_model;
use selected_instructions::{SaturatingCarrier, SaturatingOperation};

fn constraints() -> ValidatedRegisterConstraintCatalog {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    validate_x86_64_register_constraint_catalog(
        x86_64_register_constraint_catalog(&physical),
        &physical,
    )
    .unwrap_or_else(|error: X86_64RegisterConstraintCatalogValidationError| panic!("{error}"))
}

#[test]
fn saturating_catalog_binds_key_size_and_effects_for_every_carrier() {
    use SaturatingCarrier::{I64, U64};
    use SaturatingOperation::{Add, Divide, Subtract};
    let constraints = constraints();
    let physical = x86_64_physical_register_model();
    let rflags = physical.view_named("rflags").unwrap().units.clone();
    let rdx = physical.view_named("rdx").unwrap().units.clone();
    for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
        let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        for operation in [Add, Subtract, Divide] {
            for carrier in SaturatingCarrier::ALL {
                let semantic = match operation {
                    Add => MachineSemanticKind::SaturatingAdd(carrier),
                    Subtract => MachineSemanticKind::SaturatingSubtract(carrier),
                    Divide => MachineSemanticKind::SaturatingDivide(carrier),
                };
                // The constraint row follows the operand shape and the size
                // follows the realization: u64 add 19, unsigned subtract 13,
                // unsigned divide 3, signed narrow clamps 40 (add/subtract)
                // and 22 (divide), unsigned narrow add 23, i64 overflow
                // select 25 and guarded divide 26.
                let (key, size) = match (operation, carrier.is_signed(), carrier) {
                    (Add, _, U64) => (crate::register_model::X86_64_SATURATING_ADD_U64, 19),
                    (Add, true, I64) => (crate::register_model::X86_64_SATURATING_ADD_CLAMPED, 25),
                    (Add, true, _) => (crate::register_model::X86_64_SATURATING_ADD_CLAMPED, 40),
                    (Add, false, _) => (crate::register_model::X86_64_SATURATING_ADD_CLAMPED, 23),
                    (Subtract, false, _) => (
                        crate::register_model::X86_64_SATURATING_SUBTRACT_UNSIGNED,
                        13,
                    ),
                    (Subtract, true, I64) => (
                        crate::register_model::X86_64_SATURATING_SUBTRACT_CLAMPED,
                        25,
                    ),
                    (Subtract, true, _) => (
                        crate::register_model::X86_64_SATURATING_SUBTRACT_CLAMPED,
                        40,
                    ),
                    (Divide, false, _) => (crate::register_model::X86_64_DIVIDE_U64, 3),
                    (Divide, true, I64) => {
                        (crate::register_model::X86_64_SATURATING_DIVIDE_SIGNED, 26)
                    }
                    (Divide, true, _) => {
                        (crate::register_model::X86_64_SATURATING_DIVIDE_SIGNED, 22)
                    }
                };
                let division = operation == Divide;
                let three_operand = (operation, carrier) == (Add, U64)
                    || (operation == Subtract && !carrier.is_signed());
                let (reads, writes): (Vec<u16>, Vec<u16>) = if division {
                    (vec![0, 1, 3], vec![2])
                } else if three_operand {
                    (vec![0, 1], vec![2])
                } else {
                    (vec![0, 1], vec![2, 3])
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
                    alternative.key.family,
                    selected_instructions::MachineAlternativeFamily::from(semantic)
                );
                assert_eq!(
                    alternative.size,
                    MachineSizeKnowledge::ExactBytes(size),
                    "{semantic:?}"
                );
                assert_eq!(
                    alternative.applicability,
                    MachineAlternativeApplicability::Always
                );
                assert_eq!(
                    alternative.encoded.external_operand_reads, reads,
                    "{semantic:?}"
                );
                assert_eq!(
                    alternative.encoded.external_operand_writes, writes,
                    "{semantic:?}"
                );
                let mut clobbers = rflags.clone();
                if division {
                    // Division redefines RDX through CQO (or the unsigned
                    // zero convention) and reuses it for the clamp or guard.
                    clobbers.extend(rdx.iter().copied());
                    clobbers.sort_unstable();
                    clobbers.dedup();
                }
                assert_eq!(alternative.encoded.implicit_unit_clobbers, clobbers);
                assert_eq!(
                    alternative.encoded.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    division,
                    "{semantic:?}"
                );
                // The saturating rows do not depend on the target, so the
                // rebuild-and-compare corruptions run under one target only.
                if target != NativeTarget::linux_x64() {
                    continue;
                }
                for corruption in 0..4 {
                    let mut changed = catalog.clone();
                    let alternative = &mut changed
                        .declarations
                        .iter_mut()
                        .find(|row| row.semantic == semantic)
                        .unwrap()
                        .alternatives[0];
                    match corruption {
                        // Drop the RDX read of division, the scratch write of
                        // a clamped form, or the right input of a three-operand form.
                        0 if division => alternative.encoded.external_operand_reads.truncate(2),
                        0 if three_operand => {
                            alternative.encoded.external_operand_reads.truncate(1);
                        }
                        0 => alternative.encoded.external_operand_writes.truncate(1),
                        1 => alternative.encoded.implicit_unit_clobbers.clear(),
                        2 => alternative.size = MachineSizeKnowledge::ExactBytes(size + 1),
                        // A sibling carrier's family on this semantic's row.
                        _ => {
                            let sibling = SaturatingCarrier::ALL[(usize::from(carrier.ordinal())
                                + 1)
                                % SaturatingCarrier::ALL.len()];
                            alternative.key.family = match operation {
                                Add => selected_instructions::MachineAlternativeFamily::SaturatingAdd(sibling),
                                Subtract => selected_instructions::MachineAlternativeFamily::SaturatingSubtract(sibling),
                                Divide => selected_instructions::MachineAlternativeFamily::SaturatingDivide(sibling),
                            };
                        }
                    }
                    assert!(
                        validate_x86_64_machine_effect_catalog(target, &constraints, changed)
                            .is_err(),
                        "{semantic:?} corruption {corruption}"
                    );
                }
            }
        }
    }
}

#[test]
fn wrapping_remainder_catalog_defines_scratch_and_clobbers_flags() {
    let constraints = constraints();
    for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
        let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        let declaration = catalog
            .declarations
            .iter()
            .find(|row| row.semantic == MachineSemanticKind::WrappingRemainderI64)
            .unwrap();
        assert_eq!(
            declaration.constraint,
            crate::register_model::X86_64_REMAINDER_I64
        );
        let alternative = &declaration.alternatives[0];
        assert_eq!(alternative.size, MachineSizeKnowledge::ExactBytes(17));
        assert_eq!(alternative.encoded.external_operand_reads, [0, 1]);
        assert_eq!(alternative.encoded.external_operand_writes, [2, 3]);
        assert_eq!(
            alternative.encoded.implicit_unit_clobbers,
            x86_64_physical_register_model()
                .view_named("rflags")
                .unwrap()
                .units
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
                0 => alternative.encoded.external_operand_writes.truncate(1),
                1 => alternative.encoded.implicit_unit_clobbers.clear(),
                _ => alternative.size = MachineSizeKnowledge::ExactBytes(3),
            }
            assert!(validate_x86_64_machine_effect_catalog(target, &constraints, changed).is_err());
        }
    }
}

#[test]
fn integer_normalization_catalog_binds_identity_size_and_effects() {
    for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
        let constraints = constraints();
        let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        validate_x86_64_machine_effect_catalog(target, &constraints, catalog.clone()).unwrap();
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
            assert_eq!(declaration.constraint, X86_64_COPY_I64);
            assert_eq!(declaration.alternatives.len(), 1);
            let alternative = &declaration.alternatives[0];
            assert_eq!(alternative.key.family, semantic.into());
            assert_eq!(
                alternative.size,
                MachineSizeKnowledge::ExactBytes(
                    if semantic == MachineSemanticKind::SignExtendI32 {
                        3
                    } else {
                        4
                    }
                )
            );
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
                    validate_x86_64_machine_effect_catalog(target, &constraints, changed).is_err()
                );
            }
        }
    }
}

#[test]
fn catalog_declares_alias_safe_subtraction_and_control_barriers() {
    for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
        let constraints = constraints();
        let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        let subtract = catalog
            .declarations
            .iter()
            .find(|row| row.semantic == MachineSemanticKind::ExactSubtractI64)
            .unwrap();
        let add = catalog
            .declarations
            .iter()
            .find(|row| row.semantic == MachineSemanticKind::ExactAddI64)
            .unwrap();
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
        assert_eq!(less_than_branch.constraint, X86_64_CONDITIONAL_BRANCH);
        assert_eq!(less_than_branch.alternatives.len(), 1);
        assert_eq!(
            less_than_branch.alternatives[0].size,
            MachineSizeKnowledge::ExactBytes(6)
        );
        assert_eq!(
            less_than_branch.alternatives[0].encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
        assert_eq!(
            signed_less_than_branch.constraint,
            X86_64_CONDITIONAL_BRANCH
        );
        assert_eq!(signed_less_than_branch.alternatives.len(), 1);
        assert_eq!(
            signed_less_than_branch.alternatives[0].key.family,
            selected_instructions::MachineAlternativeFamily::ConditionalBranchI64LessThan
        );
        assert_eq!(
            signed_less_than_branch.alternatives[0].size,
            MachineSizeKnowledge::ExactBytes(6)
        );
        assert_eq!(
            add.alternatives[0].applicability,
            MachineAlternativeApplicability::Always
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
            MachineAlternativeApplicability::ResultAliasesOperands {
                result: 2,
                left: 0,
                right: 1,
            }
        );
        assert_eq!(
            subtract.alternatives[1].applicability,
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 2,
                aliased_operand: 0,
                distinct_operand: 1,
            }
        );
        assert_eq!(
            subtract.alternatives[2].applicability,
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 2,
                aliased_operand: 1,
                distinct_operand: 0,
            }
        );
        assert_eq!(
            subtract.alternatives[3].applicability,
            MachineAlternativeApplicability::ResultDistinctFromOperands {
                result: 2,
                left: 0,
                right: 1,
            }
        );
        assert!(catalog.declarations.iter().all(|row| {
            row.barrier
                == if matches!(
                    row.semantic,
                    MachineSemanticKind::ConditionalBranchNonZero
                        | MachineSemanticKind::ConditionalBranchU64LessThan
                        | MachineSemanticKind::ConditionalBranchI64LessThan
                        | MachineSemanticKind::ReturnScalar
                        | MachineSemanticKind::ReturnAggregate
                        | MachineSemanticKind::Jump
                        | MachineSemanticKind::ReturnUnit
                ) {
                    MachineBarrier::ControlFlow
                } else if matches!(
                    row.semantic,
                    MachineSemanticKind::CallScalar
                        | MachineSemanticKind::CallUnit
                        | MachineSemanticKind::CallAggregate
                        | MachineSemanticKind::NormalizedForeignCall
                ) {
                    MachineBarrier::Call
                } else if matches!(
                    row.semantic,
                    MachineSemanticKind::HostedReadByte
                        | MachineSemanticKind::HostedWriteByteI32
                        | MachineSemanticKind::HostedExitProcessI32
                ) {
                    MachineBarrier::ExternalEffect
                } else {
                    MachineBarrier::None
                }
        }));
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
                MachineSizeKnowledge::ExactBytes(5)
            );
            assert!(matches!(
                scalar_call.alternatives[0].encoded.memory,
                MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                    byte_count: 8,
                    ..
                }
            ));
            assert!(matches!(
                scalar_call.alternatives[0].encoded.stack,
                MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                    return_address_byte_count: 8,
                    ..
                }
            ));
        }
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
        assert!(validate_x86_64_machine_effect_catalog(target, &constraints, catalog).is_ok());
        let ordinary = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        for semantic in [
            MachineSemanticKind::Load64,
            MachineSemanticKind::Store64,
            MachineSemanticKind::FrameAddress,
            MachineSemanticKind::CallUnit,
        ] {
            assert!(
                ordinary
                    .declarations
                    .iter()
                    .any(|row| row.semantic == semantic)
            );
        }
        let load = ordinary
            .declarations
            .iter()
            .find(|row| row.semantic == MachineSemanticKind::Load64)
            .unwrap();
        assert_eq!(load.memory, MachineMemoryEffect::ReadPointerV1);
        assert_eq!(
            load.alternatives[0].encoded.memory,
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 8
            }
        );
        assert_eq!(
            load.alternatives[0].encoded.trap,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        );
    }
}

#[test]
fn semantic_validator_rejects_structural_and_target_specific_corruption() {
    let target = NativeTarget::linux_x64();
    let constraints = constraints();
    let mut missing = x86_64_machine_effect_catalog(target, &constraints).unwrap();
    missing.declarations.pop();
    assert!(matches!(
        validate_x86_64_machine_effect_catalog(target, &constraints, missing),
        Err(X86_64MachineEffectCatalogValidationError::Structural(
            MachineEffectCatalogValidationError::DeclarationRosterMismatch
        ))
    ));

    let mut wrong_size = x86_64_machine_effect_catalog(target, &constraints).unwrap();
    wrong_size
        .declarations
        .iter_mut()
        .find(|row| row.semantic == MachineSemanticKind::ExactSubtractI64)
        .unwrap()
        .alternatives[0]
        .size = MachineSizeKnowledge::ExactBytes(4);
    assert_eq!(
        validate_x86_64_machine_effect_catalog(target, &constraints, wrong_size),
        Err(X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch)
    );

    let mut bad_alias = x86_64_machine_effect_catalog(target, &constraints).unwrap();
    bad_alias
        .declarations
        .iter_mut()
        .find(|row| row.semantic == MachineSemanticKind::ExactSubtractI64)
        .unwrap()
        .alternatives[0]
        .applicability = MachineAlternativeApplicability::ResultAliasesOperand {
        result: 9,
        operand: 0,
    };
    assert!(matches!(
        validate_x86_64_machine_effect_catalog(target, &constraints, bad_alias),
        Err(X86_64MachineEffectCatalogValidationError::Structural(
            MachineEffectCatalogValidationError::InvalidAlternativeApplicability(
                MachineSemanticKind::ExactSubtractI64
            )
        ))
    ));

    let target = NativeTarget::windows_x64();
    let mut wrong_memory = x86_64_machine_effect_catalog(target, &constraints).unwrap();
    wrong_memory
        .declarations
        .iter_mut()
        .find(|row| row.semantic == MachineSemanticKind::Store64)
        .unwrap()
        .alternatives[0]
        .encoded
        .memory = MachineEncodedMemoryEffect::NoneV1;
    assert!(validate_x86_64_machine_effect_catalog(target, &constraints, wrong_memory).is_err());
}

#[test]
fn selected_keys_declare_the_supported_x86_64_pairs() {
    let constraints = constraints();
    let sysv_call = x86_64_system_v_register_call_keys()[0];
    let microsoft_call = crate::x86_64_microsoft_register_call_keys()[0];
    for (target, return_i64, return_unit, call) in [
        (
            NativeTarget::linux_x64(),
            X86_64_SYSTEM_V_RETURN,
            X86_64_SYSTEM_V_RETURN_UNIT,
            sysv_call,
        ),
        (
            NativeTarget::windows_x64(),
            X86_64_MICROSOFT_RETURN,
            X86_64_MICROSOFT_RETURN_UNIT,
            microsoft_call,
        ),
        (
            NativeTarget::uefi_x64(),
            X86_64_MICROSOFT_RETURN,
            X86_64_MICROSOFT_RETURN_UNIT,
            microsoft_call,
        ),
    ] {
        let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        let keys = &catalog.selected_keys;
        assert_eq!(keys.return_i64, return_i64);
        assert_eq!(keys.return_unit, return_unit);
        assert!(keys.call_scalar.contains(&call));
        assert_eq!(keys.call_scalar[0], call);
        let linux = target == NativeTarget::linux_x64();
        assert_eq!(
            keys.hosted_write_byte_i32.is_some(),
            linux,
            "hosted write byte is declared only for the exact Linux x86-64 target"
        );
        assert_eq!(keys.hosted_read_byte.is_some(), linux);
        assert_eq!(keys.hosted_exit_process_i32.is_some(), linux);
    }
}

#[test]
fn selected_keys_fail_closed_on_undeclared_pairs() {
    let constraints = constraints();
    let macho = NativeTarget {
        architecture: Architecture::X86_64,
        object_format: ObjectFormat::MachO,
        pointer_size: 8,
        pointer_alignment: 8,
    };
    assert_eq!(
        x86_64_machine_effect_catalog(macho, &constraints),
        Err(X86_64MachineEffectCatalogValidationError::UnsupportedTargetAbi)
    );
}

#[test]
fn hosted_rows_follow_the_declared_target_not_the_format_shape() {
    let constraints = constraints();
    // A same-format x86-64 target that is not the declared Linux target
    // claims no Linux syscall rows.
    let undeclared_elf = NativeTarget {
        pointer_size: 4,
        pointer_alignment: 4,
        ..NativeTarget::linux_x64()
    };
    let keys = x86_64_machine_effect_catalog(undeclared_elf, &constraints)
        .unwrap()
        .selected_keys;
    assert_eq!(keys.return_i64, X86_64_SYSTEM_V_RETURN);
    assert!(keys.hosted_write_byte_i32.is_none());
    assert!(keys.hosted_read_byte.is_none());
    assert!(keys.hosted_exit_process_i32.is_none());
}
