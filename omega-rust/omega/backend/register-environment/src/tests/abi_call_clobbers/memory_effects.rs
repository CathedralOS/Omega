use super::{
    Architecture, BTreeSet, EffectRejection, RegisterConstraintKey, assert_target_semantic_error,
    baseline_target_register_environment, convention_for, produced_effects, row_mut,
    scalar_abi_cases, target_constraint_catalog, target_physical_register_model, validate_effects,
    validate_physical_register_model, validate_target_register_environment, validated_effects,
};
use register_model::{RegisterOperandAccess, RegisterViewId};
use selected_instructions::{
    MachineAlternativeApplicability, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalog, MachineEffectCatalogValidationError, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior,
};

/// Every selected memory rule key with the semantic family the effect
/// catalog must declare for it. The sweep is keyed off the validated
/// environment's selection, never off the effect producer's own roster; the
/// single packed-load key carries one declaration per packed width.
fn selected_memory_rules(
    environment: &crate::ValidatedTargetRegisterEnvironment,
) -> Vec<(RegisterConstraintKey, MachineSemanticKind)> {
    let keys = environment.selected_keys();
    [
        (keys.copy_bytes, MachineSemanticKind::CopyBytes),
        (keys.store, MachineSemanticKind::Store),
        (keys.address_offset, MachineSemanticKind::AddressOffset),
        (keys.load64, MachineSemanticKind::Load64),
        (keys.load8, MachineSemanticKind::Load8),
        (keys.load16, MachineSemanticKind::Load16),
        (keys.load32, MachineSemanticKind::Load32),
        (keys.load8_indexed, MachineSemanticKind::Load8Indexed),
        (keys.store_packed, MachineSemanticKind::StorePacked),
        (keys.store64, MachineSemanticKind::Store64),
        (keys.frame_address, MachineSemanticKind::FrameAddress),
    ]
    .into_iter()
    .flat_map(|(key, semantic)| key.map(|key| (key, semantic)))
    .chain(keys.load_packed.into_iter().flat_map(|key| {
        [
            MachineSemanticKind::LoadPacked3,
            MachineSemanticKind::LoadPacked5,
            MachineSemanticKind::LoadPacked6,
            MachineSemanticKind::LoadPacked7,
        ]
        .into_iter()
        .map(move |semantic| (key, semantic))
    }))
    .collect()
}

/// The declared memory contract one selected memory semantic must carry on
/// every target/ABI pair: the constraint row's operand access pattern, its
/// early-clobber scratch positions, the encoded external custody, the
/// declaration-level footprint, and the trap surface.
struct MemoryContract {
    accesses: &'static [RegisterOperandAccess],
    early_clobbers: &'static [u16],
    reads: &'static [u16],
    writes: &'static [u16],
    memory: MachineMemoryEffect,
    faulting: bool,
    /// The constraint row implicitly uses the ABI stack-pointer view.
    uses_stack_pointer: bool,
}

fn memory_contract(semantic: MachineSemanticKind) -> MemoryContract {
    use MachineSemanticKind::*;
    use RegisterOperandAccess::{Def, Use};
    match semantic {
        CopyBytes => MemoryContract {
            accesses: &[Use, Use, Use, Def, Def],
            early_clobbers: &[3, 4],
            reads: &[0, 1, 2],
            writes: &[3, 4],
            memory: MachineMemoryEffect::CopyBytesV1,
            faulting: true,
            uses_stack_pointer: false,
        },
        LoadPacked3 | LoadPacked5 | LoadPacked6 | LoadPacked7 => MemoryContract {
            accesses: &[Use, Def, Def],
            early_clobbers: &[1, 2],
            reads: &[0],
            writes: &[1, 2],
            memory: MachineMemoryEffect::ReadPointerV1,
            faulting: true,
            uses_stack_pointer: false,
        },
        StorePacked => MemoryContract {
            accesses: &[Use, Use, Def],
            early_clobbers: &[2],
            reads: &[0, 1],
            writes: &[2],
            memory: MachineMemoryEffect::WritePointerV1,
            faulting: true,
            uses_stack_pointer: false,
        },
        Store => MemoryContract {
            accesses: &[Use, Use],
            early_clobbers: &[],
            reads: &[0, 1],
            writes: &[],
            memory: MachineMemoryEffect::WritePointerV1,
            faulting: true,
            uses_stack_pointer: false,
        },
        AddressOffset => MemoryContract {
            accesses: &[Use, Def],
            early_clobbers: &[],
            reads: &[0],
            writes: &[1],
            memory: MachineMemoryEffect::NoneV1,
            faulting: false,
            uses_stack_pointer: false,
        },
        Load8Indexed => MemoryContract {
            accesses: &[Use, Use, Def],
            early_clobbers: &[],
            reads: &[0, 1],
            writes: &[2],
            memory: MachineMemoryEffect::ReadPointerV1,
            faulting: true,
            uses_stack_pointer: false,
        },
        Load8 | Load16 | Load32 | Load64 => MemoryContract {
            accesses: &[Use, Def],
            early_clobbers: &[],
            reads: &[0],
            writes: &[1],
            memory: MachineMemoryEffect::ReadPointerV1,
            faulting: true,
            uses_stack_pointer: false,
        },
        Store64 => MemoryContract {
            accesses: &[Use],
            early_clobbers: &[],
            reads: &[0],
            writes: &[],
            memory: MachineMemoryEffect::WriteFrameStorageV1,
            faulting: true,
            uses_stack_pointer: true,
        },
        FrameAddress => MemoryContract {
            accesses: &[Def],
            early_clobbers: &[],
            reads: &[],
            writes: &[0],
            memory: MachineMemoryEffect::NoneV1,
            faulting: false,
            uses_stack_pointer: true,
        },
        other => panic!("{other:?} is not a selected memory rule"),
    }
}

/// Whether the row's declared clobber set is exactly the architecture flags
/// view: multi-instruction transport declares it on both ISAs, while packed
/// transport needs it only for the x86-64 flag-writing forms.
fn row_clobbers_flags(semantic: MachineSemanticKind, architecture: Architecture) -> bool {
    match semantic {
        MachineSemanticKind::CopyBytes => true,
        MachineSemanticKind::LoadPacked3
        | MachineSemanticKind::LoadPacked5
        | MachineSemanticKind::LoadPacked6
        | MachineSemanticKind::LoadPacked7
        | MachineSemanticKind::StorePacked => architecture == Architecture::X86_64,
        _ => false,
    }
}

const fn packed_load_width(semantic: MachineSemanticKind) -> u16 {
    match semantic {
        MachineSemanticKind::LoadPacked3 => 3,
        MachineSemanticKind::LoadPacked5 => 5,
        MachineSemanticKind::LoadPacked6 => 6,
        MachineSemanticKind::LoadPacked7 => 7,
        _ => 0,
    }
}

/// The encoded memory footprint each selected memory semantic must declare;
/// `stack_pointer` is the pair's declared ABI stack-pointer view.
fn expected_encoded_memory(
    semantic: MachineSemanticKind,
    stack_pointer: RegisterViewId,
) -> MachineEncodedMemoryEffect {
    use MachineSemanticKind::*;
    match semantic {
        CopyBytes => MachineEncodedMemoryEffect::CopyBytesV1 {
            source_pointer_operand: 0,
            destination_pointer_operand: 1,
            count_operand: 2,
        },
        LoadPacked3 | LoadPacked5 | LoadPacked6 | LoadPacked7 => {
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: packed_load_width(semantic),
            }
        }
        StorePacked | Store => MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
        Load8Indexed => MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
            pointer_operand: 0,
            index_operand: 1,
            byte_count: 1,
        },
        Load8 | Load16 | Load32 | Load64 => MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand: 0,
            byte_count: match semantic {
                Load8 => 1,
                Load16 => 2,
                Load32 => 4,
                _ => 8,
            },
        },
        Store64 => MachineEncodedMemoryEffect::WriteFrameStorageV1 {
            stack_pointer,
            byte_count: 8,
        },
        AddressOffset | FrameAddress => MachineEncodedMemoryEffect::NoneV1,
        other => panic!("{other:?} is not a selected memory rule"),
    }
}

/// The declared size knowledge for one selected memory semantic on each ISA:
/// the x86-64 encoder-resolved envelopes and exact AArch64 encodings.
fn expected_size(
    semantic: MachineSemanticKind,
    architecture: Architecture,
) -> MachineSizeKnowledge {
    use MachineSemanticKind::*;
    let resolved = |minimum_bytes: u16, maximum_bytes: u16| MachineSizeKnowledge::EncoderResolved {
        minimum_bytes,
        maximum_bytes: Some(maximum_bytes),
    };
    match architecture {
        Architecture::X86_64 => match semantic {
            CopyBytes => MachineSizeKnowledge::ExactBytes(36),
            LoadPacked3 | LoadPacked5 | LoadPacked6 | LoadPacked7 => {
                let width = packed_load_width(semantic);
                resolved(width * 8 + (width - 1) * 7, width * 9 + (width - 1) * 7)
            }
            StorePacked => resolved(32, 83),
            Store => resolved(7, 9),
            AddressOffset | Load32 | Load64 => resolved(7, 8),
            Load8 | Load16 => resolved(8, 9),
            Load8Indexed => MachineSizeKnowledge::ExactBytes(9),
            Store64 | FrameAddress => MachineSizeKnowledge::ExactBytes(8),
            other => panic!("{other:?} is not a selected memory rule"),
        },
        Architecture::Aarch64 => match semantic {
            CopyBytes => MachineSizeKnowledge::ExactBytes(28),
            LoadPacked3 | LoadPacked5 | LoadPacked6 | LoadPacked7 => {
                MachineSizeKnowledge::ExactBytes((2 * packed_load_width(semantic) - 1) * 4)
            }
            StorePacked => resolved(24, 56),
            FrameAddress => resolved(4, 8),
            Store | AddressOffset | Load8Indexed | Load8 | Load16 | Load32 | Load64 | Store64 => {
                MachineSizeKnowledge::ExactBytes(4)
            }
            other => panic!("{other:?} is not a selected memory rule"),
        },
    }
}

/// Packed-load semantics share one constraint key, so memory declarations are
/// found by (key, semantic) rather than by key alone.
fn memory_declaration_mut(
    catalog: &mut MachineEffectCatalog,
    key: RegisterConstraintKey,
    semantic: MachineSemanticKind,
) -> &mut MachineEffectDeclaration {
    catalog
        .declarations
        .iter_mut()
        .find(|declaration| declaration.constraint == key && declaration.semantic == semantic)
        .unwrap_or_else(|| panic!("effect catalog missing memory declaration {key:?}/{semantic:?}"))
}

/// Every selected memory rule on every declared target/ABI pair is bound to
/// the ordinary memory mechanism, trap surface, and register custody that
/// pair's ABI declares.
#[test]
fn every_selected_memory_rule_binds_the_declared_abi_memory_contract() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let convention = convention_for(case, environment.physical());
        let catalog = validated_effects(case, environment.constraints());
        let keys = environment.selected_keys();
        let memory_rules = selected_memory_rules(&environment);
        assert_eq!(
            memory_rules.len(),
            15,
            "{} selects an unexpected memory roster",
            case.convention
        );

        // Ordinary-memory authority is declared for exactly the selected
        // memory rules that touch memory: hosted operations carry their own
        // hosted memory shapes, and no other declaration may claim a plain
        // footprint.
        let declared_memory = catalog
            .catalog()
            .declarations
            .iter()
            .filter(|declaration| {
                matches!(
                    declaration.memory,
                    MachineMemoryEffect::CopyBytesV1
                        | MachineMemoryEffect::ReadPointerV1
                        | MachineMemoryEffect::WritePointerV1
                        | MachineMemoryEffect::WriteFrameStorageV1
                )
            })
            .map(|declaration| (declaration.constraint, declaration.semantic))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            declared_memory,
            memory_rules
                .iter()
                .copied()
                .filter(|(_, semantic)| {
                    memory_contract(*semantic).memory != MachineMemoryEffect::NoneV1
                })
                .collect::<BTreeSet<_>>(),
            "{}",
            case.convention
        );

        // The catalog is bound to this exact validated constraint catalog,
        // target, and key selection — provenance, not a re-derived roster.
        assert_eq!(catalog.catalog().target, case.target);
        assert_eq!(
            catalog.catalog().register_constraints,
            environment.constraints().identity()
        );
        assert_eq!(catalog.catalog().selected_keys, keys);

        let stack_pointer = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rsp",
                Architecture::Aarch64 => "sp",
            })
            .expect("memory matrix names a stack-pointer view");
        let flags = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rflags",
                Architecture::Aarch64 => "nzcv",
            })
            .expect("memory matrix names a flags view");
        // Every memory operand travels in the class the ABI declares for its
        // integer arguments, and frame custody reads the ABI-fixed stack
        // pointer.
        let integer_class = model.view_named(case.arguments[0]).unwrap().class;
        assert!(
            stack_pointer
                .units
                .iter()
                .all(|unit| convention.fixed.contains(unit)),
            "{} must fix the stack pointer",
            case.convention
        );

        for (key, semantic) in &memory_rules {
            let contract = memory_contract(*semantic);
            let row = environment
                .constraint(*key)
                .unwrap_or_else(|| panic!("environment missing selected memory row {key:?}"));
            let declaration = catalog
                .catalog()
                .declarations
                .iter()
                .find(|entry| entry.constraint == *key && entry.semantic == *semantic)
                .unwrap_or_else(|| panic!("effect catalog missing memory declaration {key:?}"));
            assert_eq!(declaration.semantic, *semantic, "{key:?}");
            assert_eq!(declaration.barrier, MachineBarrier::None, "{key:?}");
            assert_eq!(declaration.call, MachineCallEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.cleanup, MachineCleanupEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.memory, contract.memory, "{key:?}");
            assert_eq!(
                declaration.trap,
                if contract.faulting {
                    MachineTrapBehavior::MayArchitecturalFaultV1
                } else {
                    MachineTrapBehavior::NeverV1
                },
                "{key:?}"
            );

            // ABI operand structure: dense operand numbers with the declared
            // access pattern; memory operands stay allocatable in the ABI's
            // integer class and are never pinned to a named ABI view.
            assert_eq!(row.operands.len(), contract.accesses.len(), "{key:?}");
            for (position, operand) in row.operands.iter().enumerate() {
                assert_eq!(operand.operand as usize, position, "{key:?}");
                assert_eq!(operand.access, contract.accesses[position], "{key:?}");
                assert_eq!(
                    operand.early_clobber,
                    contract.early_clobbers.contains(&(position as u16)),
                    "{key:?}"
                );
                assert!(operand.tied_to.is_none(), "{key:?}");
                assert!(operand.fixed_view.is_none(), "{key:?}");
                assert_eq!(operand.class, integer_class, "{key:?}");
            }

            // Implicit custody is limited to the ABI-fixed stack pointer and,
            // for multi-instruction transport, the volatile flags view; no
            // memory rule may clobber state the convention preserves or fixes.
            assert_eq!(
                row.implicit_uses,
                if contract.uses_stack_pointer {
                    stack_pointer.units.clone()
                } else {
                    Vec::new()
                },
                "{key:?}"
            );
            assert!(row.implicit_defs.is_empty(), "{key:?}");
            assert_eq!(
                row.clobbers,
                if row_clobbers_flags(*semantic, case.target.architecture) {
                    flags.units.clone()
                } else {
                    Vec::new()
                },
                "{key:?}"
            );
            assert!(
                row.implicit_uses
                    .iter()
                    .chain(&row.implicit_defs)
                    .all(|unit| convention.fixed.contains(unit)),
                "{key:?} implicit state must stay inside ABI-fixed units"
            );
            assert!(
                row.clobbers.iter().all(|unit| {
                    convention.caller_saved.contains(unit)
                        && !convention.callee_saved.contains(unit)
                        && !convention.fixed.contains(unit)
                }),
                "{key:?} clobbers must stay inside the caller-saved set"
            );

            // One canonical alternative replays the exact ABI memory
            // mechanism for this target.
            assert_eq!(declaration.alternatives.len(), 1, "{key:?}");
            let alternative = &declaration.alternatives[0];
            assert_eq!(alternative.key.family, (*semantic).into(), "{key:?}");
            assert_eq!(alternative.key.variant, 0, "{key:?}");
            assert_eq!(
                alternative.applicability,
                MachineAlternativeApplicability::Always,
                "{key:?}"
            );
            assert_eq!(
                alternative.size,
                expected_size(*semantic, case.target.architecture),
                "{key:?}"
            );
            let encoded = &alternative.encoded;
            assert_eq!(encoded.external_operand_reads, contract.reads, "{key:?}");
            assert_eq!(encoded.external_operand_writes, contract.writes, "{key:?}");
            // Encoded implicit state refines exactly to the constraint row's
            // declared custody for every selected memory rule.
            assert_eq!(encoded.implicit_unit_uses, row.implicit_uses, "{key:?}");
            assert_eq!(encoded.implicit_unit_defs, row.implicit_defs, "{key:?}");
            assert_eq!(encoded.implicit_unit_clobbers, row.clobbers, "{key:?}");
            assert_eq!(
                encoded.memory,
                expected_encoded_memory(*semantic, stack_pointer.id),
                "{key:?}"
            );
            assert_eq!(
                encoded.stack,
                MachineEncodedStackEffect::UnchangedV1,
                "{key:?}"
            );
            assert_eq!(
                encoded.trap,
                if contract.faulting {
                    MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                } else {
                    MachineEncodedTrapBehavior::NeverV1
                },
                "{key:?}"
            );
            assert_eq!(
                encoded.control,
                MachineEncodedControlEffect::FallThroughV1,
                "{key:?}"
            );
        }
    }
}

/// Every selected memory rule replays its encoded footprint: understating or
/// reshaping the declared memory effect, forging stack or control effects, or
/// dropping encoded custody must reject for every rule on every declared
/// target/ABI pair.
#[test]
fn every_selected_memory_rule_rejects_encoded_memory_forgery_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let catalog = produced_effects(case, environment.constraints());
        let stack_pointer = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rsp",
                Architecture::Aarch64 => "sp",
            })
            .unwrap()
            .id;
        let foreign_unit = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rflags",
                Architecture::Aarch64 => "nzcv",
            })
            .unwrap()
            .units[0];
        for (key, semantic) in selected_memory_rules(&environment) {
            let contract = memory_contract(semantic);
            let row = environment.constraint(key).unwrap();

            // The declared footprint cannot be silently replaced: a real
            // access encoded as none (or none encoded as a pointer write)
            // fails structural admission.
            let mut corrupted = catalog.clone();
            memory_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .memory = if contract.memory == MachineMemoryEffect::NoneV1 {
                MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
            } else {
                MachineEncodedMemoryEffect::NoneV1
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} memory-footprint substitution must reject"
            );

            // A drifted footprint of the same shape still names the wrong
            // operand or byte count for the declared semantic.
            let drifted = match expected_encoded_memory(semantic, stack_pointer) {
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand,
                    byte_count,
                } => Some(MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand,
                    byte_count: byte_count + 1,
                }),
                MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                    pointer_operand,
                    index_operand,
                    ..
                } => Some(MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                    pointer_operand,
                    index_operand,
                    byte_count: 2,
                }),
                MachineEncodedMemoryEffect::WritePointerV1 { .. } => {
                    Some(MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 1 })
                }
                MachineEncodedMemoryEffect::CopyBytesV1 { .. } => {
                    Some(MachineEncodedMemoryEffect::CopyBytesV1 {
                        source_pointer_operand: 1,
                        destination_pointer_operand: 0,
                        count_operand: 2,
                    })
                }
                MachineEncodedMemoryEffect::WriteFrameStorageV1 { stack_pointer, .. } => {
                    Some(MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                        stack_pointer,
                        byte_count: 4,
                    })
                }
                _ => None,
            };
            if let Some(drifted) = drifted {
                let mut corrupted = catalog.clone();
                memory_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                    .encoded
                    .memory = drifted;
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} drifted footprint must reject"
                );
            }

            // Trap drift is rejected at the layer that owns it: understating
            // a faulting access fails structural admission, while overstating
            // a pure rule survives the fallthrough shape and needs canonical
            // ISA replay.
            let mut corrupted = catalog.clone();
            memory_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .trap = if contract.faulting {
                MachineEncodedTrapBehavior::NeverV1
            } else {
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            };
            let expected = if contract.faulting {
                EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic),
                )
            } else {
                EffectRejection::SemanticMismatch
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(expected),
                "{key:?} encoded trap drift must reject"
            );

            // A memory rule encoded as a conditional branch fails the
            // control/barrier join.
            let mut corrupted = catalog.clone();
            memory_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .control = MachineEncodedControlEffect::ConditionalRelativeBranchV1;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} branch-encoded memory rule must reject"
            );

            // A stack lifecycle cannot be forged onto a memory rule: no
            // admitted shape joins a byte pop with these semantics.
            let mut corrupted = catalog.clone();
            memory_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .stack = MachineEncodedStackEffect::PopBytesV1 {
                stack_pointer,
                byte_count: 8,
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} forged stack lifecycle must reject"
            );

            // Dropping encoded stack-pointer custody understates the row's
            // contracted uses and fails structural admission.
            if contract.uses_stack_pointer {
                let mut corrupted = catalog.clone();
                let encoded = &mut memory_declaration_mut(&mut corrupted, key, semantic)
                    .alternatives[0]
                    .encoded;
                assert!(
                    !encoded.implicit_unit_uses.is_empty(),
                    "{key:?} declares no stack-pointer use"
                );
                encoded.implicit_unit_uses.remove(0);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} lost stack-pointer use must reject"
                );
            }

            // Dropping an encoded clobber understates the interference the
            // row declares; the encoded list restates it exactly and fails
            // structural admission.
            if !row.clobbers.is_empty() {
                let mut corrupted = catalog.clone();
                memory_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                    .encoded
                    .implicit_unit_clobbers
                    .remove(0);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} lost encoded clobber must reject"
                );
            }

            // A forged implicit definition outside the row's empty def set
            // fails structural admission before canonical replay runs.
            let mut corrupted = catalog.clone();
            memory_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .implicit_unit_defs
                .push(foreign_unit);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} forged implicit def must reject"
            );

            // Dropping encoded operand custody understates the constraint
            // row's contracted definitions and fails structural admission on
            // every memory rule.
            if let Some(&dropped) = contract.writes.last() {
                let mut corrupted = catalog.clone();
                let writes = &mut memory_declaration_mut(&mut corrupted, key, semantic)
                    .alternatives[0]
                    .encoded
                    .external_operand_writes;
                writes.retain(|write| *write != dropped);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} lost operand write must reject"
                );
            }

            // Dropping an encoded operand read understates the contracted
            // input surface the same way: the pure address and frame-store
            // rows no longer admit the smaller set either.
            if let Some(&dropped) = contract.reads.last() {
                let mut corrupted = catalog.clone();
                let reads = &mut memory_declaration_mut(&mut corrupted, key, semantic).alternatives
                    [0]
                .encoded
                .external_operand_reads;
                reads.retain(|read| *read != dropped);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} lost operand read must reject"
                );
            }

            // A size-knowledge drift keeps a structurally valid bound, so
            // only canonical ISA replay may reject it.
            let mut corrupted = catalog.clone();
            memory_declaration_mut(&mut corrupted, key, semantic).alternatives[0].size =
                MachineSizeKnowledge::ExactBytes(997);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::SemanticMismatch),
                "{key:?} size drift must reject"
            );
        }
    }
}

/// The remaining memory-contract corruptions are checked once per selected
/// memory key on every declared target/ABI pair: each constraint row proves
/// the environment join sees its exact ABI facts, each declaration proves the
/// effect join sees its barrier, footprint, and trap surface, and a borrowed
/// sibling row still fails canonical re-derivation.
#[test]
fn every_memory_family_rejects_memory_contract_corruption_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let convention = convention_for(case, environment.physical());
        let catalog = produced_effects(case, environment.constraints());
        let raw = target_physical_register_model(case.target);
        let physical = validate_physical_register_model(raw.clone()).unwrap();
        let constraints = target_constraint_catalog(case.target, &physical);
        let abi_argument_view = model.view_named(case.arguments[0]).unwrap().id;
        let stack_pointer = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rsp",
                Architecture::Aarch64 => "sp",
            })
            .unwrap();
        let caller_unit = convention.caller_saved[0];

        let memory_rules = selected_memory_rules(&environment);
        for (key, semantic) in &memory_rules {
            let contract = memory_contract(*semantic);

            // A declaration cannot borrow the control-flow barrier class.
            let mut corrupted = catalog.clone();
            memory_declaration_mut(&mut corrupted, *key, *semantic).barrier =
                MachineBarrier::ControlFlow;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::BarrierMismatch(*semantic)
                )),
                "{key:?} borrowed control barrier must reject"
            );

            // Declaration-level memory drift: understating a real access or
            // inventing one on a pure rule both fail structural admission.
            let mut corrupted = catalog.clone();
            memory_declaration_mut(&mut corrupted, *key, *semantic).memory =
                if contract.memory == MachineMemoryEffect::NoneV1 {
                    MachineMemoryEffect::ReadPointerV1
                } else {
                    MachineMemoryEffect::NoneV1
                };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(*semantic)
                )),
                "{key:?} declaration memory drift must reject"
            );

            // Declaration-level trap drift fails structural admission in
            // both directions: understating a faulting rule drops the
            // declared fault it owns, and overstating a pure rule claims a
            // surface its semantic cannot carry.
            let mut corrupted = catalog.clone();
            memory_declaration_mut(&mut corrupted, *key, *semantic).trap = if contract.faulting {
                MachineTrapBehavior::NeverV1
            } else {
                MachineTrapBehavior::MayArchitecturalFaultV1
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(*semantic)
                )),
                "{key:?} declaration trap drift must reject"
            );
        }

        // The constraint rows themselves are exercised through the
        // environment join once per selected memory key; the four packed-load
        // declarations share one row.
        let mut seen = BTreeSet::new();
        for (key, _) in &memory_rules {
            if !seen.insert(*key) {
                continue;
            }
            let row = environment.constraint(*key).unwrap();
            let contract = memory_contract(
                memory_rules
                    .iter()
                    .find(|(rule_key, _)| rule_key == key)
                    .unwrap()
                    .1,
            );

            // Pinning an allocatable memory operand to the ABI's own argument
            // view stays class-valid, so only canonical re-derivation rejects it.
            let mut corrupted = constraints.clone();
            row_mut(&mut corrupted, *key).operands[0].fixed_view = Some(abi_argument_view);
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("an ABI-pinned memory operand must reject");
            assert_target_semantic_error(case.target, *key, error);

            // Flipping the leading operand's access direction is invisible to
            // structure but not to the owning ISA's canonical row.
            let mut corrupted = constraints.clone();
            let operand = &mut row_mut(&mut corrupted, *key).operands[0];
            operand.access = match operand.access {
                RegisterOperandAccess::Use => RegisterOperandAccess::Def,
                _ => RegisterOperandAccess::Use,
            };
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("a flipped memory operand access must reject");
            assert_target_semantic_error(case.target, *key, error);

            if row.implicit_uses.is_empty() {
                // Inventing implicit stack-pointer custody on a plain row
                // fails the same canonical join.
                let mut corrupted = constraints.clone();
                let uses = &mut row_mut(&mut corrupted, *key).implicit_uses;
                uses.extend(stack_pointer.units.iter().copied());
                uses.sort_unstable();
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("a plain row cannot gain stack-pointer custody");
                assert_target_semantic_error(case.target, *key, error);
            } else {
                // Stack-pointer custody cannot be dropped from a frame row.
                let mut corrupted = constraints.clone();
                row_mut(&mut corrupted, *key).implicit_uses.remove(0);
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("a frame row must retain its stack-pointer use");
                assert_target_semantic_error(case.target, *key, error);
            }

            if row.clobbers.is_empty() {
                // A pure row cannot silently claim a caller-saved clobber.
                let mut corrupted = constraints.clone();
                let clobbers = &mut row_mut(&mut corrupted, *key).clobbers;
                clobbers.push(caller_unit);
                clobbers.sort_unstable();
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("an invented memory-row clobber must reject");
                assert_target_semantic_error(case.target, *key, error);
            } else {
                // Dropping one declared transport clobber rejects.
                let mut corrupted = constraints.clone();
                row_mut(&mut corrupted, *key).clobbers.remove(0);
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("a transport row must retain its flags clobber");
                assert_target_semantic_error(case.target, *key, error);
            }

            // Losing declared early-clobber scratch marking rejects.
            if !contract.early_clobbers.is_empty() {
                let mut corrupted = constraints.clone();
                row_mut(&mut corrupted, *key).operands[contract.early_clobbers[0] as usize]
                    .early_clobber = false;
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("a transport row must retain early-clobber scratch");
                assert_target_semantic_error(case.target, *key, error);
            }
        }

        // Sibling-row borrowing: copying the store row's custody under the
        // address-offset key, or the frame-address row under store64, keeps
        // structurally valid rows that canonical re-derivation must reject.
        let keys = environment.selected_keys();
        for (donor_key, borrowed_key) in [
            (keys.store, keys.address_offset),
            (keys.frame_address, keys.store64),
        ] {
            let (Some(donor_key), Some(borrowed_key)) = (donor_key, borrowed_key) else {
                continue;
            };
            let donor = environment.constraint(donor_key).unwrap().clone();
            let mut corrupted = constraints.clone();
            let row = row_mut(&mut corrupted, borrowed_key);
            row.operands = donor.operands;
            row.implicit_uses = donor.implicit_uses;
            row.implicit_defs = donor.implicit_defs;
            row.clobbers = donor.clobbers;
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("borrowing a sibling memory row must reject");
            assert_target_semantic_error(case.target, borrowed_key, error);
        }
    }
}
