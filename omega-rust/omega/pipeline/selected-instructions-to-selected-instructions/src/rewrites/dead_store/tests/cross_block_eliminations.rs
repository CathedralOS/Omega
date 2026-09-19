use super::{
    BETWEEN, DEAD_SEQUENCE_INDEX, DEAD_SPAN_COUNT, KILLER, MATERIALIZE_COUNT, MATERIALIZE_INDEX,
    PACKED_SCRATCH, POINTER, SCRATCH, SEQUENCE_INDEX, SPAN_COUNT, STORE, VALUE, access, budget,
    chained, crossed_edge, dead_byte, dead_span_copy, dead_span_length, define_count,
    define_count_as, eliminate, fixture, instruction, make_packed_dead, mutated_chained,
    packed_dead_chained, place, runtime_count, sequence_store, settlement, settlement_at,
    span_copy, span_length, successor,
};
use crate::rewrites::dead_store::{
    DeadStoreEliminationError, eliminate_selected_dead_store, validate_dead_store_elimination,
};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, PackedByteWidth, SelectedBlock, SelectedBlockId,
    SelectedBlockOrigin, SelectedCasePayloadBinding, SelectedCasePayloadTransport,
    SelectedInstructionId, SelectedInstructionKind, SelectedLocalStorageSlot, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedStructuralBinding, SelectedStructuralCaseEdge,
    SelectedStructuralTransport, SelectedTerminator, SelectedValueBinding, SelectedValueTransport,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralCaseId,
    StructuralFieldId, ValueId,
};
use target::NativeTarget;

#[test]
fn cross_block_covering_store_eliminates_across_the_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = chained(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let result =
            eliminate_selected_dead_store(&source, 0, STORE, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        // The covering store stays at the head of the successor block.
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![KILLER]
        );
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WritePlace)]
        );
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The covering write may equally be a `Store64` into the dead place's own
/// local storage — here a block-parameter slot — at the head of the crossed
/// successor block, carrying the `WriteLocal` row for that slot.
#[test]
fn cross_block_local_slot_write_covers() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let slot = LocalStorageSlotId::StructuralBlockParameter {
            block: BlockId::new(2).unwrap(),
            place: place(),
        };
        let source = mutated_chained(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            function.blocks[1].instructions[0] = instruction(
                KILLER,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                store64,
                &[SCRATCH],
            );
            function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        let result =
            eliminate_selected_dead_store(&source, 0, STORE, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WriteLocal { slot })]
        );
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// A staging-slot dead store crosses edges like a place dead store: the
/// covering `Store64` on the same slot in the successor block rewrites the
/// staged bytes unobserved. On the crossed edge a structural transport or
/// case custody naming that very slot interferes, while a transport into a
/// different slot — or a custody discard of the staged place, which retires
/// the place's storage and never the operation-owned slot — walks past.
#[test]
fn cross_block_staging_slot_dead_store_dies_across_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // No structural contract declares the operation the place's producer:
    // the slot only stages bytes naming it.
    let slot = LocalStorageSlotId::Structural {
        operation: OperationId::new(9).unwrap(),
        place: place(),
    };
    let staged_pair = |edit: Option<&dyn Fn(&mut selected_instructions::SelectedFunction)>| {
        mutated_chained(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            function.blocks[0].instructions[1] = instruction(
                STORE,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                store64,
                &[VALUE],
            );
            function.blocks[1].instructions[0] = instruction(
                KILLER,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                store64,
                &[SCRATCH],
            );
            function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
            function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
            if let Some(edit) = edit {
                edit(function);
            }
        })
    };
    let result = eliminate(&staged_pair(None), &environment).unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN]
    );
    assert_eq!(
        function
            .memory_accesses
            .iter()
            .map(|access| (access.instruction, access.role))
            .collect::<Vec<_>>(),
        vec![(KILLER, SelectedMemoryAccessRole::WriteLocal { slot })]
    );
    validate_dead_store_elimination(
        &staged_pair(None),
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A structural transport writing that very slot on the crossed edge
    // touches the staged bytes inside the dead interval.
    let transported = staged_pair(Some(&|function| {
        crossed_edge(function)
            .structural_bindings
            .push(SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(2).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(2).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: SCRATCH,
                    destination: slot,
                    byte_size: 8,
                    alignment: 8,
                },
            });
    }));
    assert_eq!(
        eliminate(&transported, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // The same transport into another staging slot of the same place moves
    // other bytes and walks past.
    let disjoint_transport = staged_pair(Some(&|function| {
        crossed_edge(function)
            .structural_bindings
            .push(SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(2).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(2).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: SCRATCH,
                    destination: LocalStorageSlotId::Structural {
                        operation: OperationId::new(10).unwrap(),
                        place: place(),
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    }));
    eliminate(&disjoint_transport, &environment).unwrap();
    // Case custody staged through the dead slot writes it inside the
    // interval.
    let custody = staged_pair(Some(&|function| {
        crossed_edge(function).structural_case = Some(SelectedStructuralCaseEdge {
            slot,
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: Vec::new(),
            trivial_affine_discards: Vec::new(),
        });
    }));
    assert_eq!(
        eliminate(&custody, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A custody discard of the staged place retires the place's storage,
    // never the operation-owned staging slot — the bytes the slot holds are
    // untouched, so the dead store still dies under its cover.
    let discarded = staged_pair(Some(&|function| {
        crossed_edge(function).structural_case = Some(SelectedStructuralCaseEdge {
            slot: LocalStorageSlotId::Structural {
                operation: OperationId::new(9).unwrap(),
                place: PlaceId::new(2).unwrap(),
            },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: Vec::new(),
            trivial_affine_discards: vec![place()],
        });
    }));
    eliminate(&discarded, &environment).unwrap();
}

#[test]
fn cross_block_walk_crosses_intermediate_blocks_and_converging_legs() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Store in block 0, the between-copy alone in a middle bridge block 2,
    // the covering store in block 1: the walk crosses two edges and one full
    // body.
    let three = mutated_chained(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let mid_terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(2),
            },
        );
        let middle = function.blocks[0].instructions.remove(2);
        function.blocks.insert(
            1,
            SelectedBlock {
                id: SelectedBlockId(2),
                origin: SelectedBlockOrigin::EdgeTransfer {
                    edge: EdgeId::new(2).unwrap(),
                    target: BlockId::new(2).unwrap(),
                },
                instructions: vec![middle],
                terminator: mid_terminator,
            },
        );
    });
    eliminate(&three, &environment).unwrap();
    // A conditional whose legs both reach the covering block still covers
    // every path forward.
    let converged = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let SelectedTerminator::Jump {
            successor: edge, ..
        } = &function.blocks[0].terminator
        else {
            unreachable!()
        };
        let edge = edge.clone();
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch,
                &[],
            ),
            when_nonzero: edge,
            when_zero: successor(1),
        };
    });
    eliminate(&converged, &environment).unwrap();
    // A second predecessor into the covering block is harmless: coverage
    // looks forward, so a join cannot open an uncovered path.
    let join = mutated_chained(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(1),
            },
        });
    });
    eliminate(&join, &environment).unwrap();
}

/// A conditional whose legs fork to distinct blocks still eliminates when
/// every path forward reaches a covering write: each leg may open with its
/// own covering store, and the removed store drops with its roster row
/// while both killers stay.
#[test]
fn cross_block_fork_legs_covering_through_distinct_writes_eliminate() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = mutated_chained(target, |function, environment| {
            let branch = environment
                .constraint(environment.selected_keys().conditional_branch)
                .unwrap();
            let store = environment
                .constraint(environment.selected_keys().store.unwrap())
                .unwrap();
            let return_row = environment
                .constraint(environment.selected_keys().return_unit)
                .unwrap();
            let SelectedTerminator::Jump {
                successor: edge, ..
            } = &function.blocks[0].terminator
            else {
                unreachable!()
            };
            let edge = edge.clone();
            function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    SelectedInstructionId(6),
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    branch,
                    &[],
                ),
                when_nonzero: edge,
                when_zero: successor(2),
            };
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(2),
                origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
                instructions: vec![instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 8,
                    },
                    store,
                    &[POINTER, SCRATCH],
                )],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        SelectedInstructionId(9),
                        SelectedInstructionKind::ReturnUnit,
                        return_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(3).unwrap(),
                },
            });
            function.memory_accesses.push(access(
                SelectedInstructionId(8),
                4,
                place(),
                0,
                SelectedMemoryAccessRole::WritePlace,
            ));
        });
        let result = eliminate(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        // Both covering stores survive with their rows.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![
                (KILLER, SelectedMemoryAccessRole::WritePlace),
                (
                    SelectedInstructionId(8),
                    SelectedMemoryAccessRole::WritePlace
                ),
            ]
        );
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // Deterministic over an identical source, and the published output
        // is terminal: the removed store no longer exists, and each leg's
        // covering store ends a return-terminated path never rewritten.
        let repeat = eliminate(&source, &environment).unwrap();
        assert_eq!(result, repeat);
        assert_eq!(
            eliminate_selected_dead_store(&result, 0, STORE, &environment, budget()).unwrap_err(),
            DeadStoreEliminationError::SourceMismatch
        );
        for killer in [KILLER, SelectedInstructionId(8)] {
            assert_eq!(
                eliminate_selected_dead_store(&result, 0, killer, &environment, budget())
                    .unwrap_err(),
                DeadStoreEliminationError::UnsupportedPair
            );
        }
    }
}

/// Forked legs may also reconverge on one covering store: both successors
/// are clear blocks that jump to the block opening with the killer, so the
/// join's first interfering access covers every path.
#[test]
fn cross_block_fork_legs_reconverging_on_one_cover_eliminate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let SelectedTerminator::Jump {
            successor: edge, ..
        } = &function.blocks[0].terminator
        else {
            unreachable!()
        };
        let edge = edge.clone();
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch,
                &[],
            ),
            when_nonzero: successor(2),
            when_zero: successor(3),
        };
        for (id, copy_id, jump_id) in [(2, 8, 10), (3, 9, 11)] {
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(id),
                origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(id) + 1).unwrap()),
                instructions: vec![instruction(
                    SelectedInstructionId(copy_id),
                    SelectedInstructionKind::CopyI64,
                    environment
                        .constraint(environment.selected_keys().copy_i64)
                        .unwrap(),
                    &[SCRATCH, SCRATCH],
                )],
                terminator: SelectedTerminator::Jump {
                    instruction: instruction(
                        SelectedInstructionId(jump_id),
                        SelectedInstructionKind::Jump,
                        jump,
                        &[],
                    ),
                    successor: edge.clone(),
                },
            });
        }
    });
    let result = eliminate(&source, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN]
    );
    validate_dead_store_elimination(
        &source,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// The same fork rejects when any leg escapes or observes: a clear leg into
/// a return leaves the bytes observable at the boundary, and an interfering
/// access or a settlement on one leg ends the walk. A clear cycle that can
/// never observe the bytes admits instead — see `clear_cycles`.
#[test]
fn cross_block_fork_legs_escaping_or_observing_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let branch_to = |function: &mut selected_instructions::SelectedFunction,
                     environment: &register_environment::ValidatedTargetRegisterEnvironment,
                     zero: u32| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let SelectedTerminator::Jump {
            successor: edge, ..
        } = &function.blocks[0].terminator
        else {
            unreachable!()
        };
        let edge = edge.clone();
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch,
                &[],
            ),
            when_nonzero: edge,
            when_zero: successor(zero),
        };
    };
    // One leg reaches the covering store while the other returns uncovered.
    let escaped = mutated_chained(target, |function, environment| {
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        branch_to(function, environment, 2);
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::ReturnUnit,
                    return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    assert_eq!(
        eliminate(&escaped, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A read of the dead range on one leg observes the bytes.
    let observed = mutated_chained(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        branch_to(function, environment, 2);
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: vec![instruction(
                SelectedInstructionId(8),
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                load,
                &[POINTER, SCRATCH],
            )],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(9),
                    SelectedInstructionKind::ReturnUnit,
                    return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
        function.memory_accesses.push(access(
            SelectedInstructionId(8),
            4,
            place(),
            0,
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    assert_eq!(
        eliminate(&observed, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A structural transport on one crossed fork edge writes the dead
    // place's storage inside the interval — every crossed edge stays
    // unobserved even when another leg covers.
    let transported = mutated_chained(target, |function, environment| {
        branch_to(function, environment, 2);
        let SelectedTerminator::ConditionalBranch { when_zero, .. } =
            &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        when_zero
            .structural_bindings
            .push(SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(2).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(2).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: SCRATCH,
                    destination: LocalStorageSlotId::Structural {
                        operation: OperationId::new(9).unwrap(),
                        place: place(),
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    });
    assert_eq!(
        eliminate(&transported, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A settlement inside a clear leg sits inside the dead interval.
    let settled = mutated_chained(target, |function, environment| {
        branch_to(function, environment, 2);
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::Jump,
                    environment
                        .constraint(environment.selected_keys().jump)
                        .unwrap(),
                    &[],
                ),
                successor: successor(1),
            },
        });
        function
            .boundary_settlements
            .push(settlement_at(SelectedBlockId(2), 0));
    });
    assert_eq!(
        eliminate(&settled, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

#[test]
fn cross_block_forks_returns_and_cycles_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Edges fanning out to distinct blocks admit a path the covering store
    // never runs on.
    let forked = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        let SelectedTerminator::Jump {
            successor: edge, ..
        } = &function.blocks[0].terminator
        else {
            unreachable!()
        };
        let edge = edge.clone();
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch,
                &[],
            ),
            when_nonzero: edge,
            when_zero: successor(2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::ReturnUnit,
                    return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    assert_eq!(
        eliminate(&forked, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // Without the covering store the walk reaches the return's boundary,
    // where the bytes stay observable.
    let open = mutated_chained(target, |function, _| {
        function.blocks[1].instructions.clear();
        function.memory_accesses.remove(1);
    });
    assert_eq!(
        eliminate(&open, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // An edge back into the store's own block re-executes the removed store
    // from its top — the cycle stays unproven.
    let cycled = mutated_chained(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        function.blocks[1].instructions.clear();
        function.memory_accesses.remove(1);
        function.blocks[1].terminator = SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(8),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            successor: successor(0),
        };
    });
    assert_eq!(
        eliminate(&cycled, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A successor naming no block cannot be walked.
    let dangling = mutated_chained(target, |function, _| {
        crossed_edge(function).block = SelectedBlockId(99);
    });
    assert_eq!(
        eliminate(&dangling, &environment).unwrap_err(),
        DeadStoreEliminationError::SourceMismatch
    );
}

#[test]
fn cross_block_edge_transports_and_terminator_rows_decide() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Register transports cannot reach memory; carrying one across the edge
    // still eliminates.
    let carried = mutated_chained(target, |function, _| {
        crossed_edge(function).bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(5).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            transport: SelectedValueTransport::Registers {
                argument: SCRATCH,
                parameter: VALUE,
            },
        });
    });
    eliminate(&carried, &environment).unwrap();
    // A structural write into the dead place's storage observes or retires
    // the bytes inside the interval.
    let aliased = mutated_chained(target, |function, _| {
        crossed_edge(function)
            .structural_bindings
            .push(SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(2).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(2).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: SCRATCH,
                    destination: LocalStorageSlotId::Structural {
                        operation: OperationId::new(9).unwrap(),
                        place: place(),
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    });
    assert_eq!(
        eliminate(&aliased, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // The same transport into a different place's slot stays harmless.
    let disjoint = mutated_chained(target, |function, _| {
        crossed_edge(function)
            .structural_bindings
            .push(SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(2).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(2).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: SCRATCH,
                    destination: LocalStorageSlotId::Structural {
                        operation: OperationId::new(9).unwrap(),
                        place: PlaceId::new(2).unwrap(),
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    });
    eliminate(&disjoint, &environment).unwrap();
    // Case custody on the dead place writes its slot inside the interval.
    let custody = mutated_chained(target, |function, _| {
        crossed_edge(function).structural_case = Some(SelectedStructuralCaseEdge {
            slot: LocalStorageSlotId::Structural {
                operation: OperationId::new(9).unwrap(),
                place: place(),
            },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: Vec::new(),
            trivial_affine_discards: Vec::new(),
        });
    });
    assert_eq!(
        eliminate(&custody, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A trivially discarded case binding on the dead place retires storage.
    let discarded = mutated_chained(target, |function, _| {
        crossed_edge(function).structural_case = Some(SelectedStructuralCaseEdge {
            slot: LocalStorageSlotId::Structural {
                operation: OperationId::new(9).unwrap(),
                place: PlaceId::new(2).unwrap(),
            },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: Vec::new(),
            trivial_affine_discards: vec![place()],
        });
    });
    assert_eq!(
        eliminate(&discarded, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A case payload's register parameter is a register transport only.
    let payload = mutated_chained(target, |function, _| {
        crossed_edge(function).structural_case = Some(SelectedStructuralCaseEdge {
            slot: LocalStorageSlotId::Structural {
                operation: OperationId::new(9).unwrap(),
                place: PlaceId::new(2).unwrap(),
            },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: vec![SelectedCasePayloadBinding {
                semantic: legalized_operations::LegalizedStructuralCasePayload {
                    field: StructuralFieldId::new(1).unwrap(),
                    field_byte_offset: 0,
                    parameter: legalized_operations::LegalizedValueDefinition {
                        value: ValueId::new(5).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        ),
                        definition_site: ValueDefinitionSite::BlockParameter {
                            block: BlockId::new(2).unwrap(),
                            position: 0,
                        },
                    },
                },
                transport: SelectedCasePayloadTransport::Registers {
                    argument: SCRATCH,
                    parameter: VALUE,
                },
            }],
            trivial_affine_discards: Vec::new(),
        });
    });
    eliminate(&payload, &environment).unwrap();
    // A roster row on the crossed terminator's instruction decides first.
    let terminator_write = mutated_chained(target, |function, _| {
        function.memory_accesses.insert(
            1,
            access(
                SelectedInstructionId(6),
                3,
                place(),
                0,
                SelectedMemoryAccessRole::WritePlace,
            ),
        );
    });
    assert_eq!(
        eliminate(&terminator_write, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// A dynamic-extent row reaches only upward from its fixed offset, so one
/// carried on a crossed terminator or inside a crossed block walks past when
/// that offset begins at or past the dead range's end — and still interferes
/// one byte earlier, where the dead range's last byte stays reachable.
#[test]
fn cross_block_dynamic_extent_rows_past_the_dead_range_walk_past() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let sequence_write = |instruction, byte_offset| SelectedMemoryAccess {
        byte_count: 1,
        ..access(
            instruction,
            3,
            place(),
            byte_offset,
            SelectedMemoryAccessRole::WriteByteSequence {
                index: ValueId::new(5).unwrap(),
                value: ValueId::new(6).unwrap(),
                length: ValueId::new(7).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [3; 32],
                ),
            },
        )
    };
    // A dynamic write row on the crossed terminator starting at the dead
    // range's end cannot touch the dead bytes; the walk still crosses.
    let terminator_above = mutated_chained(target, |function, _| {
        function
            .memory_accesses
            .insert(1, sequence_write(SelectedInstructionId(6), 8));
    });
    eliminate(&terminator_above, &environment).unwrap();
    // One byte earlier the dead range's last byte stays reachable, so the
    // terminator row still decides against the pair.
    let terminator_inside = mutated_chained(target, |function, _| {
        function
            .memory_accesses
            .insert(1, sequence_write(SelectedInstructionId(6), 7));
    });
    assert_eq!(
        eliminate(&terminator_inside, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A dynamic write inside the crossed block — before the covering store —
    // walks past on the same reach argument once its offset starts at the
    // dead range's end.
    let body_above = mutated_chained(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[1].instructions.insert(
            0,
            instruction(
                SelectedInstructionId(8),
                SelectedInstructionKind::CopyI64,
                copy,
                &[SCRATCH, VALUE],
            ),
        );
        function
            .memory_accesses
            .insert(1, sequence_write(SelectedInstructionId(8), 8));
    });
    eliminate(&body_above, &environment).unwrap();
    let body_inside = mutated_chained(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[1].instructions.insert(
            0,
            instruction(
                SelectedInstructionId(8),
                SelectedInstructionKind::CopyI64,
                copy,
                &[SCRATCH, VALUE],
            ),
        );
        function
            .memory_accesses
            .insert(1, sequence_write(SelectedInstructionId(8), 7));
    });
    assert_eq!(
        eliminate(&body_inside, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// The byte-sequence dead store crosses edges the same way an exact store
/// does: a covering byte-sequence write at the crossed block's head spells
/// the same `offset + index` byte and removes it. A dynamic-extent row on
/// the dead place interferes with the dynamic dead extent wherever its own
/// offset starts — two unbounded-upward reaches can always share a byte —
/// so a sequence row on the crossed terminator still rejects however far
/// past the payload base it begins.
#[test]
fn cross_block_byte_sequence_dead_store_eliminates_across_the_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let covered = mutated_chained(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
        });
        let result =
            eliminate_selected_dead_store(&covered, 0, STORE, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        // The covering byte-sequence store stays at the successor's head.
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![KILLER]
        );
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.byte_offset, access.role))
                .collect::<Vec<_>>(),
            vec![(
                KILLER,
                8,
                SelectedMemoryAccessRole::WriteByteSequence {
                    index: ValueId::new(5).unwrap(),
                    value: ValueId::new(6).unwrap(),
                    length: ValueId::new(7).unwrap(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32]
                    ),
                }
            )]
        );
        validate_dead_store_elimination(
            &covered,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A covering sequence write through a different runtime index may
        // land on a different byte.
        let other_index = mutated_chained(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
        });
        assert_eq!(
            eliminate(&other_index, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
        // A sequence row on the crossed terminator meets the dynamic dead
        // extent wherever it starts — two reaches unbounded upward on one
        // place always share a byte — so the walk ends at the terminator.
        let terminator_sequence = mutated_chained(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            function.memory_accesses.insert(
                1,
                SelectedMemoryAccess {
                    byte_count: 1,
                    ..access(
                        SelectedInstructionId(6),
                        3,
                        place(),
                        64,
                        SelectedMemoryAccessRole::WriteByteSequence {
                            index: ValueId::new(9).unwrap(),
                            value: ValueId::new(6).unwrap(),
                            length: ValueId::new(7).unwrap(),
                            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                            accepted_fact:
                                optimization_core::AcceptedObligationFactIdentity::from_bytes(
                                    [3; 32],
                                ),
                        },
                    )
                },
            );
        });
        assert_eq!(
            eliminate(&terminator_sequence, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
        // A boundary settlement inside the crossed interval could still
        // observe the dead byte before the covering write lands.
        let settled = mutated_chained(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            function
                .boundary_settlements
                .push(settlement_at(SelectedBlockId(1), 0));
        });
        assert_eq!(
            eliminate(&settled, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
        // No covering write at all lets the runtime-placed byte escape at
        // the return — dead or not is then unproven.
        let uncovered = mutated_chained(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap();
            function.blocks[1].instructions[0] = instruction(
                KILLER,
                SelectedInstructionKind::CopyI64,
                copy,
                &[POINTER, SCRATCH],
            );
            function.memory_accesses.remove(1);
        });
        assert_eq!(
            eliminate(&uncovered, &environment).unwrap_err(),
            DeadStoreEliminationError::UnsupportedPair
        );
    }
}

/// The `CopyBytes` covering route crosses edges like any other covering
/// write: the copy opens the successor block and its constant-count
/// destination span rewrites the dead bytes on every path forward. But a
/// count that an edge transport also defines escapes the instruction audit
/// — the materialized constant no longer pins the span's extent.
#[test]
fn cross_block_byte_span_copy_covers_across_the_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let covered = mutated_chained(target, |function, environment| {
            span_copy(
                function,
                environment,
                KILLER,
                1,
                0,
                SPAN_COUNT,
                span_length(),
            );
            define_count(function, environment, 1, 0, SPAN_COUNT, span_length(), 16);
        });
        let result = eliminate(&covered, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![MATERIALIZE_COUNT, KILLER]
        );
        validate_dead_store_elimination(
            &covered,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // An edge binding that defines the count register alongside the
        // materialize is a second definition the operand audit cannot see —
        // the constant no longer bounds the span.
        let transported = mutated_chained(target, |function, environment| {
            span_copy(
                function,
                environment,
                KILLER,
                1,
                0,
                SPAN_COUNT,
                span_length(),
            );
            define_count(function, environment, 1, 0, SPAN_COUNT, span_length(), 16);
            crossed_edge(function).bindings.push(SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: ValueId::new(5).unwrap(),
                    argument: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                },
                transport: SelectedValueTransport::Registers {
                    argument: SCRATCH,
                    parameter: SPAN_COUNT,
                },
            });
        });
        assert_eq!(
            eliminate(&transported, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
    }
}

/// The constant-index byte-sequence cover crosses edges like any other
/// covering write: the sequence store opens the successor block and its
/// materialized index lands the write on the dead byte on every path
/// forward. But an index register an edge transport also defines escapes
/// the instruction audit — the materialized constant no longer pins the
/// written byte.
#[test]
fn cross_block_byte_sequence_store_covers_across_the_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Dead byte at offset 8 in block 0; the covering sequence write
        // opens block 1 with payload base 4 and a materialized index of 4.
        let covered = mutated_chained(target, |function, environment| {
            dead_byte(function, environment, 8);
            sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
            define_count(
                function,
                environment,
                1,
                0,
                SEQUENCE_INDEX,
                ValueId::new(5).unwrap(),
                4,
            );
        });
        let result = eliminate(&covered, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![MATERIALIZE_COUNT, KILLER]
        );
        validate_dead_store_elimination(
            &covered,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // An edge binding that defines the index register alongside the
        // materialize is a second definition the operand audit cannot see —
        // the constant no longer pins the written byte.
        let transported = mutated_chained(target, |function, environment| {
            dead_byte(function, environment, 8);
            sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
            define_count(
                function,
                environment,
                1,
                0,
                SEQUENCE_INDEX,
                ValueId::new(5).unwrap(),
                4,
            );
            crossed_edge(function).bindings.push(SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: ValueId::new(5).unwrap(),
                    argument: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                },
                transport: SelectedValueTransport::Registers {
                    argument: SCRATCH,
                    parameter: SEQUENCE_INDEX,
                },
            });
        });
        assert_eq!(
            eliminate(&transported, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
    }
}

/// A byte-sequence dead store is covered across an edge by a distinct
/// index value too: each index's sole `InstructionResult` carrier holds a
/// clean `MaterializeI64` somewhere in the function, so equal
/// `byte_offset + index` sums spell the dead byte on every path forward.
/// An edge transport redefining the dead index's carrier escapes the
/// instruction audit, so the constant no longer pins the dead byte.
#[test]
fn cross_block_byte_sequence_dead_store_covers_under_equal_constants() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Dead byte at `8 + 3` in block 0; the covering sequence write
        // opens block 1 at base 8 with a distinct index value materialized
        // to 3.
        let covered = mutated_chained(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
            define_count_as(
                function,
                environment,
                0,
                1,
                MATERIALIZE_INDEX,
                DEAD_SEQUENCE_INDEX,
                ValueId::new(5).unwrap(),
                3,
            );
            define_count(
                function,
                environment,
                1,
                0,
                SEQUENCE_INDEX,
                ValueId::new(9).unwrap(),
                3,
            );
        });
        let result = eliminate(&covered, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), MATERIALIZE_INDEX, BETWEEN]
        );
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![MATERIALIZE_COUNT, KILLER]
        );
        validate_dead_store_elimination(
            &covered,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // An edge binding that defines the dead index's carrier is a
        // second definition the operand audit cannot see — the materialized
        // constant no longer pins the dead byte.
        let transported = mutated_chained(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
            define_count_as(
                function,
                environment,
                0,
                1,
                MATERIALIZE_INDEX,
                DEAD_SEQUENCE_INDEX,
                ValueId::new(5).unwrap(),
                3,
            );
            define_count(
                function,
                environment,
                1,
                0,
                SEQUENCE_INDEX,
                ValueId::new(9).unwrap(),
                3,
            );
            crossed_edge(function).bindings.push(SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: ValueId::new(5).unwrap(),
                    argument: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                },
                transport: SelectedValueTransport::Registers {
                    argument: SCRATCH,
                    parameter: DEAD_SEQUENCE_INDEX,
                },
            });
        });
        assert_eq!(
            eliminate(&transported, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
    }
}

/// The resolved-landing walk crosses edges the same way: a byte-sequence
/// row opening the crossed block resolves its index through the carrier
/// audit — defined in that same block, defined on the store's side of the
/// edge, or defined on the crossed block — and lands off the collapsed
/// dead byte, so the walk steps over it to the covering write behind it.
/// An index the audit leaves unresolved, or a carrier an edge transport
/// redefines, keeps the row's reach unbounded upward from its payload
/// base: the walk still decides against the pair there.
#[test]
fn cross_block_constant_index_rows_landing_off_the_dead_byte_walk_past() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Dead byte at `8 + 3` = 11 in block 0. A byte-sequence write
        // opens the crossed block — its own index materializes to 2
        // against payload base 4, landing on byte 6 — and the covering
        // write behind it lands on the dead byte through the dead index's
        // own value.
        let covered = mutated_chained(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            define_count_as(
                function,
                environment,
                0,
                1,
                MATERIALIZE_INDEX,
                DEAD_SEQUENCE_INDEX,
                ValueId::new(5).unwrap(),
                3,
            );
            let store = environment
                .constraint(environment.selected_keys().store.unwrap())
                .unwrap();
            function.blocks[1].instructions.insert(
                0,
                instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, SCRATCH],
                ),
            );
            function.memory_accesses.insert(
                1,
                access(
                    SelectedInstructionId(8),
                    3,
                    place(),
                    4,
                    SelectedMemoryAccessRole::WritePlace,
                ),
            );
            sequence_store(
                function,
                environment,
                SelectedInstructionId(8),
                1,
                4,
                9,
                SCRATCH,
            );
            define_count(
                function,
                environment,
                1,
                0,
                SEQUENCE_INDEX,
                ValueId::new(9).unwrap(),
                2,
            );
        });
        let result = eliminate(&covered, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), MATERIALIZE_INDEX, BETWEEN]
        );
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![MATERIALIZE_COUNT, SelectedInstructionId(8), KILLER]
        );
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.byte_offset))
                .collect::<Vec<_>>(),
            vec![(SelectedInstructionId(8), 4), (KILLER, 8)]
        );
        validate_dead_store_elimination(
            &covered,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // The intervening index's carrier never materializes: the row's
        // reach is unbounded upward from payload base 4, so it still meets
        // the fixed dead byte at 11.
        let unresolved = mutated_chained(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            define_count_as(
                function,
                environment,
                0,
                1,
                MATERIALIZE_INDEX,
                DEAD_SEQUENCE_INDEX,
                ValueId::new(5).unwrap(),
                3,
            );
            let store = environment
                .constraint(environment.selected_keys().store.unwrap())
                .unwrap();
            function.blocks[1].instructions.insert(
                0,
                instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, SCRATCH],
                ),
            );
            function.memory_accesses.insert(
                1,
                access(
                    SelectedInstructionId(8),
                    3,
                    place(),
                    4,
                    SelectedMemoryAccessRole::WritePlace,
                ),
            );
            sequence_store(
                function,
                environment,
                SelectedInstructionId(8),
                1,
                4,
                9,
                SCRATCH,
            );
        });
        assert_eq!(
            eliminate(&unresolved, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
        // An edge transport redefining the intervening index's carrier is
        // a second definition the operand audit cannot see: the constant
        // no longer pins the landing, so the row's unbounded reach still
        // interferes.
        let transported = mutated_chained(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            define_count_as(
                function,
                environment,
                0,
                1,
                MATERIALIZE_INDEX,
                DEAD_SEQUENCE_INDEX,
                ValueId::new(5).unwrap(),
                3,
            );
            let store = environment
                .constraint(environment.selected_keys().store.unwrap())
                .unwrap();
            function.blocks[1].instructions.insert(
                0,
                instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, SCRATCH],
                ),
            );
            function.memory_accesses.insert(
                1,
                access(
                    SelectedInstructionId(8),
                    3,
                    place(),
                    4,
                    SelectedMemoryAccessRole::WritePlace,
                ),
            );
            sequence_store(
                function,
                environment,
                SelectedInstructionId(8),
                1,
                4,
                9,
                SCRATCH,
            );
            define_count(
                function,
                environment,
                1,
                0,
                SEQUENCE_INDEX,
                ValueId::new(9).unwrap(),
                2,
            );
            crossed_edge(function).bindings.push(SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: ValueId::new(9).unwrap(),
                    argument: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                },
                transport: SelectedValueTransport::Registers {
                    argument: SCRATCH,
                    parameter: SEQUENCE_INDEX,
                },
            });
        });
        assert_eq!(
            eliminate(&transported, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
    }
}

#[test]
fn cross_block_intervening_accesses_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A read of the dead range in the covering block observes the bytes.
    let read = mutated_chained(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[1].instructions.insert(
            0,
            instruction(
                SelectedInstructionId(9),
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                load,
                &[POINTER, SCRATCH],
            ),
        );
        function.memory_accesses.insert(
            1,
            access(
                SelectedInstructionId(9),
                3,
                place(),
                0,
                SelectedMemoryAccessRole::ReadPlace,
            ),
        );
    });
    assert_eq!(
        eliminate(&read, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A partial overwrite in a crossed middle block is an access, not a
    // covering store.
    let middle = mutated_chained(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let mid_terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(2),
            },
        );
        function.blocks.insert(
            1,
            SelectedBlock {
                id: SelectedBlockId(2),
                origin: SelectedBlockOrigin::EdgeTransfer {
                    edge: EdgeId::new(2).unwrap(),
                    target: BlockId::new(2).unwrap(),
                },
                instructions: vec![instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::Store {
                        byte_offset: 4,
                        byte_size: 4,
                    },
                    store,
                    &[POINTER, VALUE],
                )],
                terminator: mid_terminator,
            },
        );
        function.memory_accesses.push(SelectedMemoryAccess {
            byte_count: 4,
            ..access(
                SelectedInstructionId(10),
                3,
                place(),
                4,
                SelectedMemoryAccessRole::WritePlace,
            )
        });
    });
    assert_eq!(
        eliminate(&middle, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A shifted covering store in the successor block still rejects.
    let elsewhere = mutated_chained(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[1].instructions[0] = instruction(
            KILLER,
            SelectedInstructionKind::Store {
                byte_offset: 8,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses[1].byte_offset = 8;
    });
    assert_eq!(
        eliminate(&elsewhere, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
}

#[test]
fn cross_block_settlement_positions_decide() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A settlement before the covering store in its block is inside the dead
    // interval.
    let inside = mutated_chained(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement_at(SelectedBlockId(1), 0));
    });
    assert_eq!(
        eliminate(&inside, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A settlement after the covering store's body position observes only
    // covered bytes and stays put.
    let after = mutated_chained(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement_at(SelectedBlockId(1), 1));
    });
    let result = eliminate(&after, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0]
            .boundary_settlements
            .iter()
            .map(|settlement| (settlement.block, settlement.instruction_index))
            .collect::<Vec<_>>(),
        vec![(SelectedBlockId(1), 1)]
    );
    // Any settlement in a fully crossed block sits inside the interval.
    let bridged = mutated_chained(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let mid_terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(2),
            },
        );
        let middle = function.blocks[0].instructions.remove(2);
        function.blocks.insert(
            1,
            SelectedBlock {
                id: SelectedBlockId(2),
                origin: SelectedBlockOrigin::EdgeTransfer {
                    edge: EdgeId::new(2).unwrap(),
                    target: BlockId::new(2).unwrap(),
                },
                instructions: vec![middle],
                terminator: mid_terminator,
            },
        );
        function
            .boundary_settlements
            .push(settlement_at(SelectedBlockId(2), 0));
    });
    assert_eq!(
        eliminate(&bridged, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // Every position after the store in its own block is inside the interval
    // once the covering store crosses an edge.
    let trailing = mutated_chained(target, |function, _| {
        function.boundary_settlements.push(settlement(3));
    });
    assert_eq!(
        eliminate(&trailing, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A settlement before the removed store is outside and stays put.
    let before = mutated_chained(target, |function, _| {
        function.boundary_settlements.push(settlement(1));
    });
    let result = eliminate(&before, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0]
            .boundary_settlements
            .iter()
            .map(|settlement| (settlement.block, settlement.instruction_index))
            .collect::<Vec<_>>(),
        vec![(SelectedBlockId(0), 1)]
    );
}

#[test]
fn cross_block_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = chained(target);
    let result = eliminate(&source, &environment).unwrap();
    // Any structural drift beyond the removal itself mismatches, including
    // in the successor block the removal never touched.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[1].instructions.remove(0);
    assert_eq!(
        validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), proposed)
            .unwrap_err(),
        DeadStoreEliminationError::ReplayMismatch
    );
    let mut proposed = result.transformed().clone();
    let jump = environment
        .constraint(environment.selected_keys().jump)
        .unwrap();
    proposed.functions[0].blocks.push(SelectedBlock {
        id: SelectedBlockId(2),
        origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
        instructions: Vec::new(),
        terminator: SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(7),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            successor: successor(1),
        },
    });
    assert_eq!(
        validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), proposed)
            .unwrap_err(),
        DeadStoreEliminationError::ReplayMismatch
    );
}

/// Two runs over the identical source produce the identical validated result,
/// and the published plan is a legal second input: the sealed transformed
/// program already sits at the rule's fixed point, so the removed store no
/// longer exists and the surviving store has no later covering write.
#[test]
fn elimination_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = eliminate(&fixture(target), &environment).unwrap();
    let second = eliminate(&fixture(target), &environment).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one. Re-running on
    // it is terminal: the dead store is already gone,
    assert_eq!(
        eliminate_selected_dead_store(&first, 0, STORE, &environment, budget()).unwrap_err(),
        DeadStoreEliminationError::SourceMismatch
    );
    // and the covering store's own bytes are never rewritten, so no second
    // elimination can fire on it.
    assert_eq!(
        eliminate_selected_dead_store(&first, 0, KILLER, &environment, budget()).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
}

/// Coverage killers apply across a crossed edge the same way they do inside
/// one block: the dead store may be any exact width and the covering store at
/// the successor's head may be a wider `Store` or a covering `StorePacked`.
#[test]
fn cross_block_covering_killers_beyond_the_exact_pair_eliminate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A four-byte dead store crossed into the successor's eight-byte covering
    // store.
    let subword = mutated_chained(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 4,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses[0].byte_count = 4;
    });
    let result = eliminate(&subword, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![KILLER]
    );
    // A packed covering store at the successor's head kills a shifted dead
    // range: six bytes at offset 2 contain the dead four at offset 4.
    let packed = mutated_chained(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let packed = environment
            .constraint(environment.selected_keys().store_packed.unwrap())
            .unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 4,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 4,
            ..access(STORE, 1, place(), 4, SelectedMemoryAccessRole::WritePlace)
        };
        function.blocks[1].instructions[0] = instruction(
            KILLER,
            SelectedInstructionKind::StorePacked {
                byte_offset: 2,
                width: PackedByteWidth::Six,
            },
            packed,
            &[POINTER, SCRATCH, SCRATCH],
        );
        function.memory_accesses[1] = SelectedMemoryAccess {
            byte_count: 6,
            ..access(KILLER, 2, place(), 2, SelectedMemoryAccessRole::WritePlace)
        };
    });
    eliminate(&packed, &environment).unwrap();
    // A packed store too narrow to cover still rejects across the edge.
    let short = mutated_chained(target, |function, environment| {
        let packed = environment
            .constraint(environment.selected_keys().store_packed.unwrap())
            .unwrap();
        function.blocks[1].instructions[0] = instruction(
            KILLER,
            SelectedInstructionKind::StorePacked {
                byte_offset: 0,
                width: PackedByteWidth::Seven,
            },
            packed,
            &[POINTER, SCRATCH, SCRATCH],
        );
        function.memory_accesses[1].byte_count = 7;
    });
    assert_eq!(
        eliminate(&short, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// A packed dead store eliminates across the crossed edge the same way the
/// plain store does — the covering store at the successor's head rewrites
/// its range before any observer — and the scratch-`Def` custody still
/// applies: a successor transport or case payload naming the scratch
/// register would observe the definition the removal drops.
#[test]
fn cross_block_packed_dead_store_eliminates_and_keeps_scratch_custody() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = packed_dead_chained(target);
        let result = eliminate(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![KILLER]
        );
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WritePlace)]
        );
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A register transport on the crossed edge naming the scratch register
    // still reads the definition the removal would drop.
    let carried = mutated_chained(target, |function, environment| {
        make_packed_dead(function, environment);
        crossed_edge(function).bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(5).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            transport: SelectedValueTransport::Registers {
                argument: PACKED_SCRATCH,
                parameter: VALUE,
            },
        });
    });
    assert_eq!(
        eliminate(&carried, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // A case payload's register parameter naming the scratch register is an
    // occurrence too.
    let payload = mutated_chained(target, |function, environment| {
        make_packed_dead(function, environment);
        crossed_edge(function).structural_case = Some(SelectedStructuralCaseEdge {
            slot: LocalStorageSlotId::Structural {
                operation: OperationId::new(9).unwrap(),
                place: PlaceId::new(2).unwrap(),
            },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: vec![SelectedCasePayloadBinding {
                semantic: legalized_operations::LegalizedStructuralCasePayload {
                    field: StructuralFieldId::new(1).unwrap(),
                    field_byte_offset: 0,
                    parameter: legalized_operations::LegalizedValueDefinition {
                        value: ValueId::new(5).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        ),
                        definition_site: ValueDefinitionSite::BlockParameter {
                            block: BlockId::new(2).unwrap(),
                            position: 0,
                        },
                    },
                },
                transport: SelectedCasePayloadTransport::Registers {
                    argument: PACKED_SCRATCH,
                    parameter: VALUE,
                },
            }],
            trivial_affine_discards: Vec::new(),
        });
    });
    assert_eq!(
        eliminate(&payload, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
}

/// Two runs over the identical chained source produce the identical
/// validated result, and the published cross-block plan is a legal second
/// input: the removed store no longer exists and the covering store's block
/// ends in a return, so no later write can cover it.
#[test]
fn cross_block_elimination_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = eliminate(&chained(target), &environment).unwrap();
    let second = eliminate(&chained(target), &environment).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one. Re-running on
    // it is terminal: the eliminated store is already gone,
    assert_eq!(
        eliminate_selected_dead_store(&first, 0, STORE, &environment, budget()).unwrap_err(),
        DeadStoreEliminationError::SourceMismatch
    );
    // and the covering store's own bytes are never rewritten again — the
    // successor block's terminator has no edge a later write could cross.
    assert_eq!(
        eliminate_selected_dead_store(&first, 0, KILLER, &environment, budget()).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
}

/// A place-storage local write crosses the edge as the dead store too: the
/// `Store64` into the place's own parameter slot sits in block 0 while the
/// covering `Store` opens block 1 — every path forward still reaches the
/// cover before any observer.
#[test]
fn cross_block_local_dead_write_eliminates_across_the_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        let source = mutated_chained(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            function.blocks[0].instructions[1] = instruction(
                STORE,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                store64,
                &[VALUE],
            );
            function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        let result =
            eliminate_selected_dead_store(&source, 0, STORE, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WritePlace)]
        );
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The `CopyBytes` dead store walks across edges like every other route:
/// the copy sits in block 0 and the covering write opens block 1. A runtime
/// `length` is covered by a second `CopyBytes` spelling the same extent
/// there; a materialized `length` collapses the dead extent to an exact
/// range the successor's plain store contains.
#[test]
fn byte_span_dead_store_dies_across_an_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Runtime count: the covering copy in block 1 writes the same
        // extent — the same `byte_offset` and the same `length` value.
        let same_extent = mutated_chained(target, |function, environment| {
            dead_span_copy(
                function,
                environment,
                0,
                DEAD_SPAN_COUNT,
                dead_span_length(),
            );
            runtime_count(function, environment, DEAD_SPAN_COUNT, dead_span_length());
            span_copy(
                function,
                environment,
                KILLER,
                1,
                0,
                SPAN_COUNT,
                dead_span_length(),
            );
            runtime_count(function, environment, SPAN_COUNT, dead_span_length());
        });
        let result = eliminate(&same_extent, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![KILLER]
        );
        // Both of the dead copy's rows drop; the covering copy's span and
        // source read survive.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![
                (
                    KILLER,
                    SelectedMemoryAccessRole::WriteByteSpan {
                        length: dead_span_length(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    }
                ),
                (
                    KILLER,
                    SelectedMemoryAccessRole::ReadByteSpan {
                        length: dead_span_length(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    }
                ),
            ]
        );
        validate_dead_store_elimination(
            &same_extent,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // Resolved count: the dead extent is eight bytes at offset 0, and
        // the successor block's exact store contains it.
        let resolved = mutated_chained(target, |function, environment| {
            dead_span_copy(
                function,
                environment,
                0,
                DEAD_SPAN_COUNT,
                dead_span_length(),
            );
            define_count_as(
                function,
                environment,
                0,
                1,
                MATERIALIZE_INDEX,
                DEAD_SPAN_COUNT,
                dead_span_length(),
                8,
            );
        });
        let result = eliminate(&resolved, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WritePlace)]
        );
        validate_dead_store_elimination(
            &resolved,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}
