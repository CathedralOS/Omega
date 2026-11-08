/*
We verify the closed `NoneV1` cleanup vocabulary and activation-stack lifecycle
invariants across all supported targets: x86-64 user-space calls commit the
return-address extent below the stack pointer and returns release it, whereas
AArch64 link-register transfers and supervisor entries remain lifecycle-free.
*/

use super::{
    Architecture, EffectRejection, RegisterConstraintKey, assert_faulting_trap,
    baseline_target_register_environment, encoded_mut, named_view, produced_effects,
    scalar_abi_cases, stack_pointer_view, validate_effects, validated_effects,
};
use register_model::RegisterViewId;
use selected_instructions::MachineEffectCatalogValidationError::{
    BarrierMismatch, InvalidEncodedEffects,
};
use selected_instructions::validate_machine_effect_catalog;
use selected_instructions::{
    MachineCleanupEffect, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineSemanticKind,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum LifecycleOwner {
    Call,
    Return,
}

fn lifecycle_owner(
    architecture: Architecture,
    semantic: MachineSemanticKind,
    key: RegisterConstraintKey,
) -> Option<LifecycleOwner> {
    if !matches!(architecture, Architecture::X86_64)
        || key.family == register_model::RegisterConstraintFamily::SystemCall
    {
        return None;
    }
    match semantic {
        MachineSemanticKind::CallScalar
        | MachineSemanticKind::CallUnit
        | MachineSemanticKind::CallAggregate
        | MachineSemanticKind::InvokeNoNormal
        | MachineSemanticKind::NormalizedForeignCall => Some(LifecycleOwner::Call),
        MachineSemanticKind::ReturnScalar
        | MachineSemanticKind::ReturnUnit
        | MachineSemanticKind::ReturnAggregate => Some(LifecycleOwner::Return),
        _ => None,
    }
}

fn temporary_stack_shape(
    architecture: Architecture,
    key: RegisterConstraintKey,
    stack_pointer: RegisterViewId,
) -> Option<(u16, MachineEncodedMemoryEffect)> {
    if architecture != Architecture::X86_64 {
        return None;
    }
    match key {
        isa_x86_64::X86_64_IEEE_FLOAT_DIRECTED_BINARY
        | isa_x86_64::X86_64_IEEE_FLOAT_DIRECTED_UNARY => Some((
            16,
            MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                stack_pointer,
                byte_count: 16,
            },
        )),
        isa_x86_64::X86_64_FLAGS_SNAPSHOT | isa_x86_64::X86_64_FLAGS_RESTORE => Some((
            8,
            MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                stack_pointer,
                byte_count: 8,
            },
        )),
        isa_x86_64::X86_64_TASK_REGISTER_ACTIVATE => Some((
            8,
            MachineEncodedMemoryEffect::TaskRegisterActivationV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                selector_operand: 2,
                scratch_pointer_operand: 3,
                table_byte_count_operand: 4,
                stack_pointer,
            },
        )),
        isa_x86_64::X86_64_CODE_SEGMENT_RELOAD => Some((
            16,
            MachineEncodedMemoryEffect::SegmentDescriptorReloadV1 {
                table_pointer_operand: 0,
                selector_operand: 1,
                scratch_pointer_operand: 2,
                table_byte_count_operand: 3,
                stack_pointer,
            },
        )),
        isa_x86_64::X86_64_DATA_SEGMENT_RELOAD => Some((
            8,
            MachineEncodedMemoryEffect::SegmentDescriptorReloadV1 {
                table_pointer_operand: 0,
                selector_operand: 1,
                scratch_pointer_operand: 2,
                table_byte_count_operand: 3,
                stack_pointer,
            },
        )),
        isa_x86_64::X86_64_INSTALLED_INTERRUPT_CPU_CHECK => Some((
            8,
            MachineEncodedMemoryEffect::InstalledInterruptCpuCheckV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                scratch_pointer_operand: 2,
                stack_pointer,
            },
        )),
        isa_x86_64::X86_64_INSTALLED_INTERRUPT_TASK_COMMIT => Some((
            8,
            MachineEncodedMemoryEffect::InstalledInterruptTaskCommitV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                scratch_pointer_operand: 2,
                stack_pointer,
            },
        )),
        _ => None,
    }
}

fn lifecycle_memory(
    owner: LifecycleOwner,
    view: RegisterViewId,
    byte_count: u16,
) -> MachineEncodedMemoryEffect {
    match owner {
        LifecycleOwner::Call => MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
            stack_pointer: view,
            byte_count,
        },
        LifecycleOwner::Return => MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer: view,
            byte_count,
        },
    }
}

fn lifecycle_stack(
    owner: LifecycleOwner,
    view: RegisterViewId,
    byte_count: u16,
) -> MachineEncodedStackEffect {
    match owner {
        LifecycleOwner::Call => MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer: view,
            return_address_byte_count: byte_count,
        },
        LifecycleOwner::Return => MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer: view,
            byte_count,
        },
    }
}

#[test]
fn every_selected_rule_binds_its_activation_stack_lifecycle() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let catalog = validated_effects(case, environment.constraints());
        let stack_pointer = stack_pointer_view(model, case);

        for declaration in &catalog.catalog().declarations {
            let key = declaration.constraint;
            let semantic = declaration.semantic;
            assert_eq!(declaration.cleanup, MachineCleanupEffect::NoneV1, "{key:?}");
            assert!(!declaration.alternatives.is_empty(), "{key:?}");

            let lifecycle = lifecycle_owner(case.target.architecture, semantic, key);
            for alternative in &declaration.alternatives {
                let encoded = &alternative.encoded;
                match lifecycle {
                    Some(LifecycleOwner::Call) => {
                        assert_eq!(
                            encoded.memory,
                            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                                stack_pointer: stack_pointer.id,
                                byte_count: 8,
                            },
                            "{key:?}"
                        );
                        assert_eq!(
                            encoded.stack,
                            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                                stack_pointer: stack_pointer.id,
                                return_address_byte_count: 8,
                            },
                            "{key:?}"
                        );
                        assert_faulting_trap(encoded, key);
                        assert_eq!(
                            encoded.control,
                            MachineEncodedControlEffect::DirectRelativeCallV1,
                            "{key:?}"
                        );
                    }
                    Some(LifecycleOwner::Return) => {
                        assert_eq!(
                            encoded.memory,
                            MachineEncodedMemoryEffect::ReadActivationStackV1 {
                                stack_pointer: stack_pointer.id,
                                byte_count: 8,
                            },
                            "{key:?}"
                        );
                        assert_eq!(
                            encoded.stack,
                            MachineEncodedStackEffect::PopBytesV1 {
                                stack_pointer: stack_pointer.id,
                                byte_count: 8,
                            },
                            "{key:?}"
                        );
                        assert_faulting_trap(encoded, key);
                        assert_eq!(
                            encoded.control,
                            MachineEncodedControlEffect::ReturnFromActivationStackV1,
                            "{key:?}"
                        );
                    }
                    None if temporary_stack_shape(
                        case.target.architecture,
                        key,
                        stack_pointer.id,
                    )
                    .is_some() =>
                    {
                        let (byte_count, expected_memory) =
                            temporary_stack_shape(case.target.architecture, key, stack_pointer.id)
                                .unwrap();
                        assert_eq!(
                            encoded.stack,
                            MachineEncodedStackEffect::TemporaryBytesV1 {
                                stack_pointer: stack_pointer.id,
                                byte_count,
                            },
                            "{key:?}"
                        );
                        assert_eq!(encoded.memory, expected_memory, "{key:?}");
                        assert_faulting_trap(encoded, key);
                        let row = environment.constraint(key).unwrap();
                        assert!(
                            stack_pointer
                                .units
                                .iter()
                                .all(|unit| row.implicit_uses.contains(unit))
                        );
                        assert!(
                            stack_pointer
                                .units
                                .iter()
                                .all(|unit| !row.implicit_defs.contains(unit))
                        );
                    }
                    None => {
                        assert_eq!(
                            encoded.stack,
                            MachineEncodedStackEffect::UnchangedV1,
                            "{key:?}"
                        );
                        assert!(
                            !matches!(
                                encoded.memory,
                                MachineEncodedMemoryEffect::ReadActivationStackV1 { .. }
                                    | MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                                        ..
                                    }
                            ),
                            "{key:?} claims an activation-stack footprint without its lifecycle"
                        );
                    }
                }
            }

            if lifecycle.is_some() {
                let row = environment
                    .constraint(key)
                    .unwrap_or_else(|| panic!("environment missing selected row {key:?}"));
                assert!(
                    stack_pointer
                        .units
                        .iter()
                        .all(|unit| row.implicit_uses.contains(unit)),
                    "{key:?} lifecycle must restate its stack-pointer use"
                );
                assert!(
                    stack_pointer
                        .units
                        .iter()
                        .all(|unit| row.implicit_defs.contains(unit)),
                    "{key:?} lifecycle must restate its stack-pointer definition"
                );
            }
        }
    }
}

#[test]
fn every_selected_rule_rejects_activation_stack_lifecycle_forgery() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let catalog = produced_effects(case, environment.constraints());
        let stack_pointer = stack_pointer_view(model, case);
        let foreign_pointer =
            named_view(model, case.target, "rax", "x0", "target names no such view");
        let rules = catalog
            .declarations
            .iter()
            .map(|declaration| (declaration.semantic, declaration.constraint))
            .collect::<Vec<_>>();
        for (semantic, key) in rules {
            let forgeries: Vec<(&'static str, MachineEncodedStackEffect)> =
                match lifecycle_owner(case.target.architecture, semantic, key) {
                    None => vec![
                        (
                            "borrowed return cleanup",
                            MachineEncodedStackEffect::PopBytesV1 {
                                stack_pointer: stack_pointer.id,
                                byte_count: 8,
                            },
                        ),
                        (
                            "borrowed call lifecycle",
                            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                                stack_pointer: stack_pointer.id,
                                return_address_byte_count: 8,
                            },
                        ),
                    ],
                    Some(owner) => {
                        let owned = |view, count| lifecycle_stack(owner, view, count);
                        vec![
                            ("dropped lifecycle", MachineEncodedStackEffect::UnchangedV1),
                            (
                                "borrowed cross-class lifecycle",
                                match owner {
                                    LifecycleOwner::Call => MachineEncodedStackEffect::PopBytesV1 {
                                        stack_pointer: stack_pointer.id,
                                        byte_count: 8,
                                    },
                                    LifecycleOwner::Return => {
                                        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                                            stack_pointer: stack_pointer.id,
                                            return_address_byte_count: 8,
                                        }
                                    }
                                },
                            ),
                            ("zero-extent lifecycle", owned(stack_pointer.id, 0)),
                            ("mismatched lifecycle extent", owned(stack_pointer.id, 16)),
                            ("foreign lifecycle pointer", owned(foreign_pointer.id, 8)),
                        ]
                    }
                };
            let expected_rejection = if matches!(
                semantic,
                MachineSemanticKind::MemoryFence | MachineSemanticKind::AtomicFence
            ) {
                BarrierMismatch(semantic)
            } else {
                InvalidEncodedEffects(semantic)
            };
            for (label, stack) in forgeries {
                let mut corrupted = catalog.clone();
                encoded_mut(&mut corrupted, key, semantic, 0).stack = stack;
                assert_eq!(
                    validate_machine_effect_catalog(environment.constraints(), corrupted),
                    Err(expected_rejection),
                    "{key:?} {label} must reject"
                );
            }

            if lifecycle_owner(case.target.architecture, semantic, key).is_some() {
                let mut corrupted = catalog.clone();
                encoded_mut(&mut corrupted, key, semantic, 0).trap =
                    MachineEncodedTrapBehavior::NeverV1;
                assert_eq!(
                    validate_machine_effect_catalog(environment.constraints(), corrupted),
                    Err(InvalidEncodedEffects(semantic)),
                    "{key:?} understated lifecycle fault must reject"
                );
            }
        }
    }
}

#[test]
fn every_selected_lifecycle_rule_rejects_consistent_coordinate_forgery() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let catalog = produced_effects(case, environment.constraints());
        let stack_pointer = stack_pointer_view(model, case);
        let foreign_pointer =
            named_view(model, case.target, "rax", "x0", "target names no such view");
        let rules = catalog
            .declarations
            .iter()
            .map(|declaration| (declaration.semantic, declaration.constraint))
            .collect::<Vec<_>>();
        let mut saw_lifecycle = false;
        for (semantic, key) in rules {
            let Some(owner) = lifecycle_owner(case.target.architecture, semantic, key) else {
                continue;
            };
            saw_lifecycle = true;

            let mut corrupted = catalog.clone();
            let encoded = encoded_mut(&mut corrupted, key, semantic, 0);
            encoded.memory = lifecycle_memory(owner, foreign_pointer.id, 8);
            encoded.stack = lifecycle_stack(owner, foreign_pointer.id, 8);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::SemanticMismatch),
                "{key:?} consistently renamed lifecycle pointer must reject"
            );

            let mut corrupted = catalog.clone();
            let encoded = encoded_mut(&mut corrupted, key, semantic, 0);
            encoded.memory = lifecycle_memory(owner, stack_pointer.id, 4);
            encoded.stack = lifecycle_stack(owner, stack_pointer.id, 4);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::SemanticMismatch),
                "{key:?} consistently narrowed lifecycle extent must reject"
            );
        }
        assert_eq!(
            saw_lifecycle,
            matches!(case.target.architecture, Architecture::X86_64),
            "{} must expose exactly its lifecycle rows",
            case.convention
        );
    }
}
