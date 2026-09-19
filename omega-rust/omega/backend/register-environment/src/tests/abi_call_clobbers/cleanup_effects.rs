//! Cleanup family: the declared `cleanup` surface is a closed `NoneV1`
//! vocabulary today — admission fails closed if a second variant ever
//! appears — and the only cleanup-capable encoded surface is the
//! activation-stack lifecycle. An x86-64 call commits the return-address
//! extent below the stack pointer and an x86-64 return releases it;
//! AArch64 transfers control through its link register, so no selected rule
//! owns a lifecycle there. The lifecycle vocabulary itself is
//! architecture-agnostic: structural admission binds its shape — the memory
//! footprint, the stack lifecycle, and the trap surface must agree — while
//! the ISA's canonical replay owns the coordinates, namely which view the
//! stack pointer is and how wide a return address runs.

use super::{
    Architecture, EffectRejection, RegisterConstraintKey, baseline_target_register_environment,
    produced_effects, scalar_abi_cases, validate_effects, validated_effects,
};
use selected_instructions::{
    MachineCleanupEffect, MachineEffectCatalog, MachineEffectCatalogValidationError,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineSemanticKind,
};

/// Which side of the activation-stack lifecycle a selected semantic owns —
/// a call commits the return-address extent below the stack pointer, a
/// return releases it — when the ISA's call pushes the return address at
/// all. Only x86-64 does; AArch64 returns through its link register, so
/// every selected rule there is lifecycle-free.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LifecycleOwner {
    Call,
    Return,
}

fn lifecycle_owner(
    architecture: Architecture,
    semantic: MachineSemanticKind,
) -> Option<LifecycleOwner> {
    if !matches!(architecture, Architecture::X86_64) {
        return None;
    }
    match semantic {
        MachineSemanticKind::CallScalar
        | MachineSemanticKind::CallUnit
        | MachineSemanticKind::CallAggregate
        | MachineSemanticKind::NormalizedForeignCall => Some(LifecycleOwner::Call),
        MachineSemanticKind::ReturnScalar
        | MachineSemanticKind::ReturnUnit
        | MachineSemanticKind::ReturnAggregate => Some(LifecycleOwner::Return),
        _ => None,
    }
}

/// The declaration for one selected (semantic, constraint) pair: several
/// semantics share a constraint key — `copy_i64` backs every integer move
/// and narrow extension, `materialize_boolean` backs every boolean
/// materialization — so a catalog row is addressed by both coordinates.
fn semantic_declaration_mut(
    catalog: &mut MachineEffectCatalog,
    semantic: MachineSemanticKind,
    key: RegisterConstraintKey,
) -> &mut MachineEffectDeclaration {
    catalog
        .declarations
        .iter_mut()
        .find(|declaration| declaration.semantic == semantic && declaration.constraint == key)
        .unwrap_or_else(|| panic!("effect catalog missing {semantic:?} declaration {key:?}"))
}

/// Every selected rule on every declared target/ABI pair binds its cleanup
/// surface: the declared vocabulary stays closed to `NoneV1`, and the
/// encoded activation-stack lifecycle — the only cleanup-capable surface —
/// is owned by exactly the semantics that transfer control through the
/// activation, restating its memory footprint's pointer and extent.
#[test]
fn every_selected_rule_binds_its_activation_stack_lifecycle() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let catalog = validated_effects(case, environment.constraints());
        let stack_pointer = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rsp",
                Architecture::Aarch64 => "sp",
            })
            .expect("the ABI matrix names a stack-pointer view");

        for declaration in &catalog.catalog().declarations {
            let key = declaration.constraint;
            let semantic = declaration.semantic;
            // The declared cleanup vocabulary is closed today: a second
            // `MachineCleanupEffect` variant must name its owning semantics
            // in admission before any declaration may carry it.
            assert_eq!(declaration.cleanup, MachineCleanupEffect::NoneV1, "{key:?}");
            assert!(!declaration.alternatives.is_empty(), "{key:?}");

            let lifecycle = lifecycle_owner(case.target.architecture, semantic);
            for alternative in &declaration.alternatives {
                let encoded = &alternative.encoded;
                match lifecycle {
                    Some(LifecycleOwner::Call) => {
                        // A call writes the return address below the stack
                        // pointer; the lifecycle commits exactly the extent
                        // that footprint writes.
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
                        assert_eq!(
                            encoded.trap,
                            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                            "{key:?}"
                        );
                        assert_eq!(
                            encoded.control,
                            MachineEncodedControlEffect::DirectRelativeCallV1,
                            "{key:?}"
                        );
                    }
                    Some(LifecycleOwner::Return) => {
                        // A return reads the pushed return address; the pop
                        // releases exactly the extent that read consumed.
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
                        assert_eq!(
                            encoded.trap,
                            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                            "{key:?}"
                        );
                        assert_eq!(
                            encoded.control,
                            MachineEncodedControlEffect::ReturnFromActivationStackV1,
                            "{key:?}"
                        );
                    }
                    None => {
                        // Every other rule — including all calls and returns
                        // on the link-register ISA — leaves the stack
                        // untouched and touches no activation-stack
                        // footprint.
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

            // The constraint row holds custody of the pointer a lifecycle
            // moves: the call and return rows implicitly use and define the
            // stack pointer the commit or release adjusts.
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

/// No selected rule may claim a stack lifecycle its encoding does not own:
/// rules without an activation-stack footprint reject a borrowed lifecycle,
/// and the call/return rows that own one reject its loss, its cross-class
/// sibling, its fault surface, and forged coordinates — on every declared
/// target/ABI pair.
#[test]
fn every_selected_rule_rejects_activation_stack_lifecycle_forgery() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let catalog = produced_effects(case, environment.constraints());
        let stack_pointer = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rsp",
                Architecture::Aarch64 => "sp",
            })
            .unwrap();
        let foreign_pointer = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rax",
                Architecture::Aarch64 => "x0",
            })
            .unwrap();
        let rules = catalog
            .declarations
            .iter()
            .map(|declaration| (declaration.semantic, declaration.constraint))
            .collect::<Vec<_>>();
        for (semantic, key) in rules {
            let forgeries: Vec<(&'static str, MachineEncodedStackEffect)> =
                match lifecycle_owner(case.target.architecture, semantic) {
                    // A rule that touches no activation-stack footprint cannot
                    // borrow either lifecycle shape: the vocabulary is closed
                    // to the semantics that own it. This also covers AArch64's
                    // calls and returns, whose link-register control performs
                    // no stack lifecycle.
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
                        let owned = |view, count| match owner {
                            LifecycleOwner::Call => {
                                MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                                    stack_pointer: view,
                                    return_address_byte_count: count,
                                }
                            }
                            LifecycleOwner::Return => MachineEncodedStackEffect::PopBytesV1 {
                                stack_pointer: view,
                                byte_count: count,
                            },
                        };
                        vec![
                            ("dropped lifecycle", MachineEncodedStackEffect::UnchangedV1),
                            // The sibling class's lifecycle does not fit this
                            // row's memory footprint.
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
            // Every forgery above fails structural admission: the
            // (memory, stack, trap) triple no arm owns rejects before the
            // ISA's canonical replay ever runs, so the raw validator is
            // enough.
            for (label, stack) in forgeries {
                let mut corrupted = catalog.clone();
                semantic_declaration_mut(&mut corrupted, semantic, key).alternatives[0]
                    .encoded
                    .stack = stack;
                assert_eq!(
                    selected_instructions::validate_machine_effect_catalog(
                        environment.constraints(),
                        corrupted,
                    ),
                    Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                        semantic
                    )),
                    "{key:?} {label} must reject"
                );
            }

            // A lifecycle dereferences the stack: understating its fault
            // surface leaves a (memory, stack, trap) triple no semantic owns.
            if lifecycle_owner(case.target.architecture, semantic).is_some() {
                let mut corrupted = catalog.clone();
                semantic_declaration_mut(&mut corrupted, semantic, key).alternatives[0]
                    .encoded
                    .trap = MachineEncodedTrapBehavior::NeverV1;
                assert_eq!(
                    selected_instructions::validate_machine_effect_catalog(
                        environment.constraints(),
                        corrupted,
                    ),
                    Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                        semantic
                    )),
                    "{key:?} understated lifecycle fault must reject"
                );
            }
        }
    }
}

/// Structural admission binds the lifecycle's shape — its pointer and
/// extent must restate the memory footprint's — but only the ISA's
/// canonical replay knows which view the stack pointer is and how wide a
/// return address runs. A consistent forgery that renames both surfaces'
/// pointer, or resizes both extents, survives admission and must still
/// reject for every lifecycle-owning rule on every declared target.
#[test]
fn every_selected_lifecycle_rule_rejects_consistent_coordinate_forgery() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let catalog = produced_effects(case, environment.constraints());
        let stack_pointer = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rsp",
                Architecture::Aarch64 => "sp",
            })
            .unwrap();
        let foreign_pointer = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rax",
                Architecture::Aarch64 => "x0",
            })
            .unwrap();
        let rules = catalog
            .declarations
            .iter()
            .map(|declaration| (declaration.semantic, declaration.constraint))
            .collect::<Vec<_>>();
        let mut saw_lifecycle = false;
        for (semantic, key) in rules {
            let Some(owner) = lifecycle_owner(case.target.architecture, semantic) else {
                continue;
            };
            saw_lifecycle = true;

            // Rename the lifecycle pointer on both surfaces consistently:
            // admission cannot tell a register view from the stack pointer,
            // so canonical replay must reject it.
            let mut corrupted = catalog.clone();
            let encoded = &mut semantic_declaration_mut(&mut corrupted, semantic, key).alternatives
                [0]
            .encoded;
            encoded.memory = match owner {
                LifecycleOwner::Call => {
                    MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                        stack_pointer: foreign_pointer.id,
                        byte_count: 8,
                    }
                }
                LifecycleOwner::Return => MachineEncodedMemoryEffect::ReadActivationStackV1 {
                    stack_pointer: foreign_pointer.id,
                    byte_count: 8,
                },
            };
            encoded.stack = match owner {
                LifecycleOwner::Call => MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                    stack_pointer: foreign_pointer.id,
                    return_address_byte_count: 8,
                },
                LifecycleOwner::Return => MachineEncodedStackEffect::PopBytesV1 {
                    stack_pointer: foreign_pointer.id,
                    byte_count: 8,
                },
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::SemanticMismatch),
                "{key:?} consistently renamed lifecycle pointer must reject"
            );

            // Halve the lifecycle extent on both surfaces consistently:
            // admission sees a coherent lifecycle, so canonical replay must
            // reject the half-width return address.
            let mut corrupted = catalog.clone();
            let encoded = &mut semantic_declaration_mut(&mut corrupted, semantic, key).alternatives
                [0]
            .encoded;
            encoded.memory = match owner {
                LifecycleOwner::Call => {
                    MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                        stack_pointer: stack_pointer.id,
                        byte_count: 4,
                    }
                }
                LifecycleOwner::Return => MachineEncodedMemoryEffect::ReadActivationStackV1 {
                    stack_pointer: stack_pointer.id,
                    byte_count: 4,
                },
            };
            encoded.stack = match owner {
                LifecycleOwner::Call => MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                    stack_pointer: stack_pointer.id,
                    return_address_byte_count: 4,
                },
                LifecycleOwner::Return => MachineEncodedStackEffect::PopBytesV1 {
                    stack_pointer: stack_pointer.id,
                    byte_count: 4,
                },
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::SemanticMismatch),
                "{key:?} consistently narrowed lifecycle extent must reject"
            );
        }
        // The lifecycle is real on the call-pushes-return-address ISA and
        // genuinely absent on the link-register ISA: the sweep must be
        // non-vacuous on x86-64 and must not invent one on AArch64.
        assert_eq!(
            saw_lifecycle,
            matches!(case.target.architecture, Architecture::X86_64),
            "{} must expose exactly its lifecycle rows",
            case.convention
        );
    }
}
