use super::{
    BETWEEN, LOAD, MATERIALIZE_READ_INDEX, OUTPUT, POINTER, READ_INDEX, SCRATCH, SEQUENCE_INDEX,
    STORE, VALUE, access, budget, chained, crossed_edge, define_index, define_index_as, fixture,
    forward, instruction, mutated_chained, place, sequence_pair, sequence_write, successor,
};
use crate::{
    StoredLoadForwardingError, forward_selected_stored_load, validate_stored_load_forwarding,
};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedCasePayloadBinding, SelectedCasePayloadTransport, SelectedFunction,
    SelectedInstructionId, SelectedInstructionKind, SelectedLocalStorageSlot, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedOperand, SelectedStructuralBinding,
    SelectedStructuralCaseEdge, SelectedStructuralTransport, SelectedTerminator,
    SelectedValueBinding, SelectedValueTransport, VirtualRegisterId,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralCaseId,
    StructuralFieldId, ValueId,
};
use target::NativeTarget;

#[test]
fn cross_block_store_forwards_through_a_unique_predecessor() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = chained(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let source_load = source.transformed().functions[0].blocks[1].instructions[0].clone();
        let result = forward(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        let rewritten = &function.blocks[1].instructions[0];
        assert_eq!(rewritten.id, LOAD);
        assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VALUE);
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.provenance, source_load.provenance);
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| access.role)
                .collect::<Vec<_>>(),
            vec![SelectedMemoryAccessRole::WritePlace]
        );
        // Replay restores the complete source by content.
        validate_stored_load_forwarding(
            &source,
            0,
            LOAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// A local-storage writer in the predecessor block sources the forward
/// across the edge the same way a referent-pointer store does: the
/// `Store64` into the place's own parameter slot carries `WriteLocal`, and
/// the crossed edge transports neither the stored register nor the load's
/// result.
#[test]
fn cross_block_local_slot_writer_forwards_across_the_edge() {
    let target = NativeTarget::linux_x64();
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
    let result = forward(&source, &environment).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[1].instructions[0];
    assert_eq!(rewritten.id, LOAD);
    assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
    assert_eq!(
        result.transformed().functions[0]
            .memory_accesses
            .iter()
            .map(|access| access.role)
            .collect::<Vec<_>>(),
        vec![SelectedMemoryAccessRole::WriteLocal { slot }]
    );
    validate_stored_load_forwarding(
        &source,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

#[test]
fn cross_block_walk_crosses_every_intermediate_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Store in block 0, the between-copy alone in a middle bridge block 2,
    // the load in block 1: the walk crosses two edges and one full body.
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
                origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
                instructions: vec![middle],
                terminator: mid_terminator,
            },
        );
    });
    forward(&three, &environment).unwrap();
    // A conditional predecessor still decides on every path to the load when
    // it is the only predecessor block.
    let branched = mutated_chained(target, |function, environment| {
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
    forward(&branched, &environment).unwrap();
}

/// A join forwards when every predecessor path's last writer stored the same
/// register: one store dominating the join, or each leg's own last writer of
/// that register.
#[test]
fn cross_block_joins_forward_when_every_path_resolves_one_register() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The store sits above a diamond: block 0 branches to blocks 2 and 3,
    // both empty legs jumping to the load's block, so the store decides on
    // both paths.
    let diamond = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
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
        for (instruction_id, block_id) in [(7, 2), (8, 3)] {
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(block_id),
                origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(block_id) + 2).unwrap()),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Jump {
                    instruction: instruction(
                        SelectedInstructionId(instruction_id),
                        SelectedInstructionKind::Jump,
                        jump,
                        &[],
                    ),
                    successor: successor(1),
                },
            });
        }
    });
    let result = forward(&diamond, &environment).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[1].instructions[0];
    assert_eq!(rewritten.id, LOAD);
    assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
    validate_stored_load_forwarding(
        &diamond,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // Each leg carrying its own last writer of the same register resolves the
    // join the same way: block 0 loses its store, and blocks 2 and 3 each
    // store VALUE to the same place range.
    let per_leg = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions.remove(1);
        function.memory_accesses.remove(0);
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
        for (instruction_id, block_id) in [(7, 2), (9, 3)] {
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(block_id),
                origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(block_id) + 2).unwrap()),
                instructions: vec![instruction(
                    SelectedInstructionId(instruction_id + 10),
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 8,
                    },
                    store,
                    &[POINTER, VALUE],
                )],
                terminator: SelectedTerminator::Jump {
                    instruction: instruction(
                        SelectedInstructionId(instruction_id),
                        SelectedInstructionKind::Jump,
                        jump,
                        &[],
                    ),
                    successor: successor(1),
                },
            });
            function.memory_accesses.push(access(
                SelectedInstructionId(instruction_id + 10),
                4,
                place(),
                0,
                SelectedMemoryAccessRole::WritePlace,
            ));
        }
    });
    let result = forward(&per_leg, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1].instructions[0].kind,
        SelectedInstructionKind::CopyI64
    );
    validate_stored_load_forwarding(
        &per_leg,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A non-interfering access on one leg does not end its resolution: block
    // 3's store of a different place walks past and the leg still resolves to
    // block 0's writer.
    let disjoint_leg = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
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
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
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
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: vec![instruction(
                SelectedInstructionId(9),
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                store,
                &[POINTER, SCRATCH],
            )],
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(1),
            },
        });
        function.memory_accesses.push(access(
            SelectedInstructionId(9),
            4,
            PlaceId::new(2).unwrap(),
            0,
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    let result = forward(&disjoint_leg, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1].instructions[0].kind,
        SelectedInstructionKind::CopyI64
    );
}

/// A join rejects when its legs disagree on the stored register, when a leg's
/// edge or terminator disturbs the pair, or when a deferred region never
/// reaches a writer.
#[test]
fn cross_block_joins_reject_when_paths_disagree_or_never_resolve() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Block 0 branches to blocks 2 and 3, each storing a different register
    // to the forwarded range: the load observes a path-dependent value.
    let divergent = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions.remove(1);
        function.memory_accesses.remove(0);
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
        for (instruction_id, block_id, value) in [(7, 2, VALUE), (9, 3, SCRATCH)] {
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(block_id),
                origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(block_id) + 2).unwrap()),
                instructions: vec![instruction(
                    SelectedInstructionId(instruction_id + 10),
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 8,
                    },
                    store,
                    &[POINTER, value],
                )],
                terminator: SelectedTerminator::Jump {
                    instruction: instruction(
                        SelectedInstructionId(instruction_id),
                        SelectedInstructionKind::Jump,
                        jump,
                        &[],
                    ),
                    successor: successor(1),
                },
            });
            function.memory_accesses.push(access(
                SelectedInstructionId(instruction_id + 10),
                4,
                place(),
                0,
                SelectedMemoryAccessRole::WritePlace,
            ));
        }
    });
    assert_eq!(
        forward(&divergent, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // A register parameter defined on one leg's crossed edge redefines the
    // carried value even though every leg's writer stored it.
    let redefined = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
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
        for (instruction_id, block_id) in [(7, 2), (8, 3)] {
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(block_id),
                origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(block_id) + 2).unwrap()),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Jump {
                    instruction: instruction(
                        SelectedInstructionId(instruction_id),
                        SelectedInstructionKind::Jump,
                        jump,
                        &[],
                    ),
                    successor: successor(1),
                },
            });
        }
        let SelectedTerminator::Jump {
            successor: edge, ..
        } = &mut function.blocks[2].terminator
        else {
            unreachable!()
        };
        edge.bindings.push(SelectedValueBinding {
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
    assert_eq!(
        forward(&redefined, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // A roster row on one leg's terminator still decides before its edge.
    let terminator_write = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
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
        for (instruction_id, block_id) in [(7, 2), (8, 3)] {
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(block_id),
                origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(block_id) + 2).unwrap()),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Jump {
                    instruction: instruction(
                        SelectedInstructionId(instruction_id),
                        SelectedInstructionKind::Jump,
                        jump,
                        &[],
                    ),
                    successor: successor(1),
                },
            });
        }
        function.memory_accesses.push(access(
            SelectedInstructionId(8),
            4,
            place(),
            0,
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        forward(&terminator_write, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A deferred cycle whose arriving legs disagree stays unproven: block 2
    // jumps to the load's block or to block 3, which jumps back to block 2,
    // and a fourth block stores a different register into the same range on
    // its own leg into block 3 — the cycle's legs carry both registers, so
    // no single register decides every arriving path.
    let cycled = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let SelectedTerminator::Jump {
            successor: edge, ..
        } = &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        *edge = successor(2);
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    branch,
                    &[],
                ),
                when_nonzero: successor(1),
                when_zero: successor(3),
            },
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(2),
            },
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(4),
            origin: SelectedBlockOrigin::Source(BlockId::new(6).unwrap()),
            instructions: vec![instruction(
                SelectedInstructionId(10),
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                store,
                &[POINTER, SCRATCH],
            )],
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(9),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(3),
            },
        });
        function.memory_accesses.push(access(
            SelectedInstructionId(10),
            5,
            place(),
            0,
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        forward(&cycled, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // A writer inside the cycle that stores a different register decides its
    // own leg to that register, which disagrees with block 0's leg the same
    // way an external writer does.
    let cycle_writer = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let SelectedTerminator::Jump {
            successor: edge, ..
        } = &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        *edge = successor(2);
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    branch,
                    &[],
                ),
                when_nonzero: successor(1),
                when_zero: successor(3),
            },
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: vec![instruction(
                SelectedInstructionId(10),
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                store,
                &[POINTER, SCRATCH],
            )],
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(2),
            },
        });
        function.memory_accesses.push(access(
            SelectedInstructionId(10),
            5,
            place(),
            0,
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        forward(&cycle_writer, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
}

/// A deferred cycle carries no writer of its own, so it forwards the one
/// register every arriving leg settled on: a two-block cycle under the
/// store, a self-loop on the load's own block, and a multi-block loop back
/// through the load's block all resolve to block 0's stored register.
#[test]
fn cross_block_writerless_cycles_resolve_when_arriving_legs_agree() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Block 2 jumps to the load's block or to block 3, which jumps back to
    // block 2 — every arriving path carries block 0's store.
    let cycled = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let SelectedTerminator::Jump {
            successor: edge, ..
        } = &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        *edge = successor(2);
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    branch,
                    &[],
                ),
                when_nonzero: successor(1),
                when_zero: successor(3),
            },
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(2),
            },
        });
    });
    let result = forward(&cycled, &environment).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[1].instructions[0];
    assert_eq!(rewritten.id, LOAD);
    assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
    validate_stored_load_forwarding(
        &cycled,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A self-loop on the load's block makes the block its own predecessor;
    // the empty tail behind the load keeps the cycle clear.
    let looped = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        function.blocks[1].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(9),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch,
                &[],
            ),
            when_nonzero: successor(1),
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
                psi_return_edge: EdgeId::new(4).unwrap(),
            },
        });
    });
    let result = forward(&looped, &environment).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[1].instructions[0];
    assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    validate_stored_load_forwarding(
        &looped,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A multi-block loop returns to the load's block: 1 branches to block 2
    // or exits at block 4, 2 jumps to 3, and 3 jumps back to 1. The span
    // behind the load is empty, so the cyclic legs still carry block 0's
    // store.
    let looped_through = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        function.blocks[1].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(9),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch,
                &[],
            ),
            when_nonzero: successor(2),
            when_zero: successor(4),
        };
        for (instruction_id, block_id, destination) in [(7, 2, 3), (8, 3, 1)] {
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(block_id),
                origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(block_id) + 3).unwrap()),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Jump {
                    instruction: instruction(
                        SelectedInstructionId(instruction_id),
                        SelectedInstructionKind::Jump,
                        jump,
                        &[],
                    ),
                    successor: successor(destination),
                },
            });
        }
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(4),
            origin: SelectedBlockOrigin::Source(BlockId::new(6).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::ReturnUnit,
                    return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(5).unwrap(),
            },
        });
    });
    let result = forward(&looped_through, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1].instructions[0].kind,
        SelectedInstructionKind::CopyI64
    );
    validate_stored_load_forwarding(
        &looped_through,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A writer inside the cycle that stores the same register decides its
    // own leg to that register: block 3 stores VALUE before jumping back to
    // block 2, so every arriving leg agrees.
    let cycle_writer = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let SelectedTerminator::Jump {
            successor: edge, ..
        } = &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        *edge = successor(2);
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    branch,
                    &[],
                ),
                when_nonzero: successor(1),
                when_zero: successor(3),
            },
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: vec![instruction(
                SelectedInstructionId(10),
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                store,
                &[POINTER, VALUE],
            )],
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(2),
            },
        });
        function.memory_accesses.push(access(
            SelectedInstructionId(10),
            5,
            place(),
            0,
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    let result = forward(&cycle_writer, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1].instructions[0].kind,
        SelectedInstructionKind::CopyI64
    );
    validate_stored_load_forwarding(
        &cycle_writer,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The looped head's tail stays subject to the walk's rules: a store to
    // a different place behind the load cannot touch the forwarded bytes,
    // so the self-loop still resolves.
    let disjoint_tail = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        function.blocks[1].instructions.push(instruction(
            SelectedInstructionId(10),
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        ));
        function.memory_accesses.push(access(
            SelectedInstructionId(10),
            5,
            PlaceId::new(2).unwrap(),
            0,
            SelectedMemoryAccessRole::WritePlace,
        ));
        function.blocks[1].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(9),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch,
                &[],
            ),
            when_nonzero: successor(1),
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
                psi_return_edge: EdgeId::new(4).unwrap(),
            },
        });
    });
    let result = forward(&disjoint_tail, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1].instructions[0].kind,
        SelectedInstructionKind::CopyI64
    );
    validate_stored_load_forwarding(
        &disjoint_tail,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A self-loop on the load's block makes the span behind the load part of
/// the walked interval: a write into the forwarded place there decides the
/// next iteration's read, and redefining either carried register breaks the
/// copy's identity.
#[test]
fn cross_block_looped_head_needs_a_clear_tail() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let looped_tail = |edit: &dyn Fn(
        &mut SelectedFunction,
        &register_environment::ValidatedTargetRegisterEnvironment,
    )| {
        mutated_chained(target, |function, environment| {
            let branch = environment
                .constraint(environment.selected_keys().conditional_branch)
                .unwrap();
            let return_row = environment
                .constraint(environment.selected_keys().return_unit)
                .unwrap();
            edit(function, environment);
            function.blocks[1].terminator = SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    SelectedInstructionId(9),
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    branch,
                    &[],
                ),
                when_nonzero: successor(1),
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
                    psi_return_edge: EdgeId::new(4).unwrap(),
                },
            });
        })
    };
    // A store into the forwarded place behind the load is the last writer on
    // the looping path — the read does not observe the carried register.
    let tail_write = looped_tail(&|function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[1].instructions.push(instruction(
            SelectedInstructionId(10),
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        ));
        function.memory_accesses.push(access(
            SelectedInstructionId(10),
            5,
            place(),
            0,
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        forward(&tail_write, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // The same store without its roster row is an unaccounted write.
    let unaccounted_tail = looped_tail(&|function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[1].instructions.push(instruction(
            SelectedInstructionId(10),
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        ));
    });
    assert_eq!(
        forward(&unaccounted_tail, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A tail redefinition of the carried register leaves the copy reading a
    // different value than the place holds.
    let redefined_tail = looped_tail(&|function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[1].instructions.push(instruction(
            SelectedInstructionId(10),
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, VALUE],
        ));
    });
    assert_eq!(
        forward(&redefined_tail, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // A tail definition of the load's own result register is equally fatal.
    let predefined_tail = looped_tail(&|function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[1].instructions.push(instruction(
            SelectedInstructionId(10),
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, OUTPUT],
        ));
    });
    assert_eq!(
        forward(&predefined_tail, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // The looped head's terminator joins the walked interval too: a branch
    // instruction defining the carried register cannot keep the copy honest.
    let terminator_defined = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        let class = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .operands[0]
            .class;
        let mut terminator = instruction(
            SelectedInstructionId(9),
            SelectedInstructionKind::ConditionalBranchNonZero,
            branch,
            &[POINTER],
        );
        terminator.operands.push(SelectedOperand {
            operand: 1,
            virtual_register: VALUE,
            access: RegisterOperandAccess::Def,
            class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
        function.blocks[1].terminator = SelectedTerminator::ConditionalBranch {
            instruction: terminator,
            when_nonzero: successor(1),
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
                psi_return_edge: EdgeId::new(4).unwrap(),
            },
        });
    });
    assert_eq!(
        forward(&terminator_defined, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
}

/// The mirror of the tail-write rejection: a tail store that sources the
/// carried register itself is the last writer on the looping path, and the
/// next iteration's read still observes that register.
#[test]
fn cross_block_looped_head_tail_store_of_the_carried_register_forwards() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let carried_tail = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        function.blocks[1].instructions.push(instruction(
            SelectedInstructionId(10),
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, VALUE],
        ));
        function.memory_accesses.push(access(
            SelectedInstructionId(10),
            5,
            place(),
            0,
            SelectedMemoryAccessRole::WritePlace,
        ));
        function.blocks[1].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(9),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch,
                &[],
            ),
            when_nonzero: successor(1),
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
                psi_return_edge: EdgeId::new(4).unwrap(),
            },
        });
    });
    let result = forward(&carried_tail, &environment).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[1].instructions[0];
    assert_eq!(rewritten.id, LOAD);
    assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
    validate_stored_load_forwarding(
        &carried_tail,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

#[test]
fn cross_block_joins_unreachable_and_entry_blocks_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second predecessor block admits a path that never passed the store.
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
    assert_eq!(
        forward(&join, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // With no predecessor the load's block is unreachable and no pair forms.
    let detached = mutated_chained(target, |function, environment| {
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        let exit = SelectedBlock {
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
                psi_return_edge: EdgeId::new(4).unwrap(),
            },
        };
        let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
            unreachable!()
        };
        *successor = self::successor(2);
        function.blocks.push(exit);
    });
    assert_eq!(
        forward(&detached, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // A deferred cycle closed on itself never carries a register: blocks 2
    // and 3 jump between each other and to the load's block, but no edge
    // enters the region from a resolved block — every leg stays open and the
    // load's block never settles.
    let closed = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        function.blocks[0].terminator = SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(11),
                SelectedInstructionKind::ReturnUnit,
                return_row,
                &[],
            ),
            psi_return_edge: EdgeId::new(5).unwrap(),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    branch,
                    &[],
                ),
                when_nonzero: successor(1),
                when_zero: successor(3),
            },
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(2),
            },
        });
    });
    assert_eq!(
        forward(&closed, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // Removing the store walks to the entry block's top, where the implicit
    // entry path keeps the pair unproven.
    let storeless = mutated_chained(target, |function, _| {
        function.blocks[0].instructions.remove(1);
        function.memory_accesses.remove(0);
    });
    assert_eq!(
        forward(&storeless, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
}

#[test]
fn cross_block_edge_transports_and_terminator_rows_decide() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // An edge register parameter redefining the carried value cannot forward.
    let value_binding = |parameter: VirtualRegisterId| SelectedValueBinding {
        semantic: abstract_operations::ValueBinding {
            parameter: ValueId::new(5).unwrap(),
            argument: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        },
        transport: SelectedValueTransport::Registers {
            argument: SCRATCH,
            parameter,
        },
    };
    let carried = mutated_chained(target, |function, _| {
        crossed_edge(function).bindings.push(value_binding(VALUE));
    });
    assert_eq!(
        forward(&carried, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // The same transport writing the load's output register is equally fatal.
    let result_register = mutated_chained(target, |function, _| {
        crossed_edge(function).bindings.push(value_binding(OUTPUT));
    });
    assert_eq!(
        forward(&result_register, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // An unrelated edge parameter does not touch the carried registers.
    let unrelated = mutated_chained(target, |function, _| {
        crossed_edge(function).bindings.push(value_binding(POINTER));
    });
    forward(&unrelated, &environment).unwrap();
    // A case-payload register parameter redefining the carried value rejects.
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
    assert_eq!(
        forward(&payload, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // A structural write into the forwarded place's storage aliases.
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
        forward(&aliased, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
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
    forward(&disjoint, &environment).unwrap();
    // A trivially discarded case binding on the forwarded place is a write.
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
        forward(&discarded, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
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
        forward(&terminator_write, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A dynamic-extent write on the crossed terminator reaches only upward
    // from its fixed offset, so starting at the read's end it is provably
    // disjoint and the walk crosses; one byte earlier the read's last byte
    // stays reachable and it still decides against the pair.
    let terminator_above = mutated_chained(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
                ..access(
                    SelectedInstructionId(6),
                    3,
                    place(),
                    8,
                    SelectedMemoryAccessRole::WriteByteSpan {
                        length: ValueId::new(7).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    forward(&terminator_above, &environment).unwrap();
    let terminator_inside = mutated_chained(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
                ..access(
                    SelectedInstructionId(6),
                    3,
                    place(),
                    7,
                    SelectedMemoryAccessRole::WriteByteSpan {
                        length: ValueId::new(7).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    assert_eq!(
        forward(&terminator_inside, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

#[test]
fn cross_block_predecessor_body_killers_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Redefining the carried value in the predecessor body must not forward.
    let redefined = mutated_chained(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, VALUE],
        );
    });
    assert_eq!(
        forward(&redefined, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // An unaccounted referent write in the predecessor body rejects.
    let unaccounted = mutated_chained(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        );
    });
    assert_eq!(
        forward(&unaccounted, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A partial overwrite in a crossed middle block is a killer, not a
    // forwarding source.
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
                origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
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
        forward(&middle, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

#[test]
fn cross_block_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = chained(target);
    let result = forward(&source, &environment).unwrap();
    // Forwarded from the wrong register.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[1].instructions[0].operands[0].virtual_register = POINTER;
    assert_eq!(
        validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), proposed)
            .unwrap_err(),
        StoredLoadForwardingError::ReplayMismatch
    );
    // Any structural drift beyond the forwarding itself mismatches.
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
        validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), proposed)
            .unwrap_err(),
        StoredLoadForwardingError::ReplayMismatch
    );
}

/// Two runs over the identical source produce the identical validated result,
/// and the published plan is a legal second input: the sealed transformed
/// program already sits at the rule's fixed point — the load keeps its
/// instruction identity as the forwarding copy, so a second forwarding at the
/// same site finds no load shape to admit.
#[test]
fn forwarding_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = forward(&fixture(target), &environment).unwrap();
    let second = forward(&fixture(target), &environment).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one. Re-running on
    // it is terminal: instruction LOAD is the emitted CopyI64 now, not a
    // place load.
    assert_eq!(
        forward_selected_stored_load(&first, 0, LOAD, &environment, budget()).unwrap_err(),
        StoredLoadForwardingError::UnsupportedInstruction
    );
}

/// Two runs over the identical chained source produce the identical
/// validated result, and the published cross-block plan is a legal second
/// input: the load's slot in the successor block already holds the emitted
/// copy, so a second forwarding at that site finds no load shape to admit,
/// and the surviving store is not a load either.
#[test]
fn cross_block_forwarding_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = forward(&chained(target), &environment).unwrap();
    let second = forward(&chained(target), &environment).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one. Re-running on
    // it is terminal.
    assert_eq!(
        forward_selected_stored_load(&first, 0, LOAD, &environment, budget()).unwrap_err(),
        StoredLoadForwardingError::UnsupportedInstruction
    );
    assert_eq!(
        forward_selected_stored_load(&first, 0, STORE, &environment, budget()).unwrap_err(),
        StoredLoadForwardingError::UnsupportedInstruction
    );
}

/// The indexed byte load forwards across the edge the same way an exact
/// load does: the byte-sequence store in the predecessor is its byte-exact
/// writer, the crossed terminator carries no row on the place, and the edge
/// moves neither the stored register nor the load's result.
#[test]
fn cross_block_byte_sequence_load_forwards_across_the_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = mutated_chained(target, |function, environment| {
            sequence_pair(function, environment, 0, 5);
        });
        let environment = baseline_target_register_environment(target).unwrap();
        let result = forward(&source, &environment).unwrap();
        let rewritten = &result.transformed().functions[0].blocks[1].instructions[0];
        assert_eq!(rewritten.id, LOAD);
        assert_eq!(rewritten.kind, SelectedInstructionKind::ZeroExtendU8);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VALUE);
        assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
        // The roster drops only the load's read row; the covering sequence
        // store's row survives in the predecessor block.
        assert_eq!(
            result.transformed().functions[0]
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(
                STORE,
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
        validate_stored_load_forwarding(
            &source,
            0,
            LOAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A sequence row on the crossed terminator always meets the dynamic
    // read, whatever its index.
    let terminator_sequence = mutated_chained(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        function.memory_accesses.insert(
            2,
            SelectedMemoryAccess {
                byte_count: 1,
                ..access(
                    SelectedInstructionId(6),
                    3,
                    place(),
                    0,
                    SelectedMemoryAccessRole::WriteByteSequence {
                        index: ValueId::new(5).unwrap(),
                        value: ValueId::new(6).unwrap(),
                        length: ValueId::new(7).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    assert_eq!(
        forward(&terminator_sequence, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // An edge transport redefining the carried register rejects the pair.
    let redefined = mutated_chained(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
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
    assert_eq!(
        forward(&redefined, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
    // A byte-sequence read the roster does not cover is not the pair: the
    // deferred predecessor region resolves to a register only when the
    // stored value is the read's source.
    let storeless = mutated_chained(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        function.blocks[0].instructions.remove(1);
        function.memory_accesses.remove(0);
    });
    assert_eq!(
        forward(&storeless, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
}

/// The constant-index treatment crosses an edge the same way it walks a
/// block: the resolved read byte is a fixed position, a resolved sequence
/// write in the predecessor lands on or off it, and an edge transport
/// redefining the index's carrier keeps the read dynamic — the resolved
/// `MaterializeI64` is no longer the value's only definition.
#[test]
fn cross_block_constant_index_rows_land_on_fixed_positions() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The read's index materializes to 3 against payload base 8 — the byte
    // is fixed at 11 — so the exact one-byte `Store` in the predecessor
    // sources the forward across the edge.
    let exact = mutated_chained(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            1,
            0,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        for block in &mut function.blocks {
            if let Some(position) = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == STORE)
            {
                block.instructions[position] = instruction(
                    STORE,
                    SelectedInstructionKind::Store {
                        byte_offset: 11,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, VALUE],
                );
            }
        }
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(STORE, 1, place(), 11, SelectedMemoryAccessRole::WritePlace)
        };
    });
    let result = forward(&exact, &environment).unwrap();
    let function = &result.transformed().functions[0];
    let rewritten = function.blocks[1]
        .instructions
        .iter()
        .find(|instruction| instruction.id == LOAD)
        .unwrap();
    assert_eq!(rewritten.kind, SelectedInstructionKind::ZeroExtendU8);
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    assert_eq!(
        function
            .memory_accesses
            .iter()
            .map(|access| (access.instruction, access.byte_offset, access.role))
            .collect::<Vec<_>>(),
        vec![(STORE, 11, SelectedMemoryAccessRole::WritePlace)]
    );
    validate_stored_load_forwarding(
        &exact,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A resolved sequence write in the predecessor landing off the read
    // byte — base 4 plus index 8 lands on 12 — walks past to the source.
    let walks_past = mutated_chained(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            1,
            0,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        for block in &mut function.blocks {
            if let Some(position) = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == STORE)
            {
                block.instructions[position] = instruction(
                    STORE,
                    SelectedInstructionKind::Store {
                        byte_offset: 11,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, VALUE],
                );
            }
        }
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(STORE, 1, place(), 11, SelectedMemoryAccessRole::WritePlace)
        };
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_write(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_index(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            8,
        );
    });
    let result = forward(&walks_past, &environment).unwrap();
    let rewritten = result.transformed().functions[0].blocks[1]
        .instructions
        .iter()
        .find(|instruction| instruction.id == LOAD)
        .unwrap();
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    validate_stored_load_forwarding(
        &walks_past,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // Landing on the read byte instead makes the intervening write the last
    // writer: it sources the forward with its own stored register across
    // the edge.
    let lands_on = mutated_chained(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            1,
            0,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_write(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_index(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            7,
        );
    });
    let result = forward(&lands_on, &environment).unwrap();
    let rewritten = result.transformed().functions[0].blocks[1]
        .instructions
        .iter()
        .find(|instruction| instruction.id == LOAD)
        .unwrap();
    assert_eq!(rewritten.operands[0].virtual_register, SCRATCH);
    validate_stored_load_forwarding(
        &lands_on,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // An edge transport defining the index's carrier leaves the resolved
    // `MaterializeI64` no longer the value's only definition: the audit
    // keeps the read index runtime, so the exact one-byte writer cannot
    // source the runtime-placed byte.
    let redefined = mutated_chained(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            1,
            0,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        for block in &mut function.blocks {
            if let Some(position) = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == STORE)
            {
                block.instructions[position] = instruction(
                    STORE,
                    SelectedInstructionKind::Store {
                        byte_offset: 11,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, VALUE],
                );
            }
        }
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(STORE, 1, place(), 11, SelectedMemoryAccessRole::WritePlace)
        };
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
                parameter: READ_INDEX,
            },
        });
    });
    assert_eq!(
        forward(&redefined, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}
