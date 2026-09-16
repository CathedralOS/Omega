use super::{
    BETWEEN, LOAD, OUTPUT, POINTER, SCRATCH, STORE, VALUE, access, budget, chained, crossed_edge,
    fixture, forward, instruction, mutated_chained, place, successor,
};
use crate::{
    StoredLoadForwardingError, forward_selected_stored_load, validate_stored_load_forwarding,
};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedCasePayloadBinding, SelectedCasePayloadTransport, SelectedInstructionId,
    SelectedInstructionKind, SelectedMemoryAccess, SelectedMemoryAccessRole,
    SelectedStructuralBinding, SelectedStructuralCaseEdge, SelectedStructuralTransport,
    SelectedTerminator, SelectedValueBinding, SelectedValueTransport, VirtualRegisterId,
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
    // A self-loop on the load's block makes the block its own predecessor.
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
    assert_eq!(
        forward(&looped, &environment).unwrap_err(),
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
