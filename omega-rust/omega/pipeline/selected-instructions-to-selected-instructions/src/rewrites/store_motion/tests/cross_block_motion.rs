use super::{
    BETWEEN, KILLER, MATERIALIZE_INDEX, MATERIALIZE_MOVED_INDEX, MOVED_SEQUENCE_INDEX,
    PACKED_SCRATCH, POINTER, SCRATCH, SEQUENCE_INDEX, STORE, VALUE, access, budget, chained,
    crossed_edge, define_index, define_index_as, instruction, mutated_chained, pack_store, place,
    sequence_store, settlement, settlement_at, sink, successor,
};
use crate::rewrites::store_motion::{
    StoreMutationMotionError, ValidatedStoreMutationMotion, sink_selected_store_mutation,
    validate_store_mutation_motion,
};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
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
fn cross_block_store_sinks_across_the_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = chained(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let result =
            sink_selected_store_mutation(&source, 0, STORE, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        // The store lands at the head of the successor, before the covering
        // store that bounds the window.
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![STORE, KILLER]
        );
        assert_eq!(
            function.memory_accesses,
            source.transformed().functions[0].memory_accesses
        );
        validate_store_mutation_motion(
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

#[test]
fn cross_block_walk_crosses_converging_legs_and_intermediate_blocks() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A conditional whose legs both reach the covering block still carries
    // the write on every path forward.
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
    let result = sink(&converged, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
    // The store slides through an empty bridge block into the covering one.
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
        function.blocks.insert(
            1,
            SelectedBlock {
                id: SelectedBlockId(2),
                origin: SelectedBlockOrigin::EdgeTransfer {
                    edge: EdgeId::new(2).unwrap(),
                    target: BlockId::new(2).unwrap(),
                },
                instructions: Vec::new(),
                terminator: mid_terminator,
            },
        );
    });
    let result = sink(&three, &environment).unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN]
    );
    assert!(function.blocks[1].instructions.is_empty());
    assert_eq!(
        function.blocks[2]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
}

/// The packed store crosses the edge like the plain store, but its
/// early-clobber scratch definition crosses with it: an edge transport
/// reading the scratch moves a use across the moved definition, and one
/// rewriting it is a second definition the moved store's would overtake —
/// each lands the store at the crossed block's end. On a flag-publishing
/// target a flag-reading terminator keeps the store on the publishing side
/// of the edge even when every leg reaches the covering block, while a
/// target whose packed row publishes no condition state crosses the same
/// terminator freely.
#[test]
fn cross_block_packed_stores_carry_their_scratch() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The packed store sinks across the edge to the covering block's head.
    let packed = mutated_chained(target, |function, environment| {
        pack_store(function, environment);
    });
    let result = sink(&packed, &environment).unwrap();
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
        vec![STORE, KILLER]
    );
    validate_store_mutation_motion(
        &packed,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // An edge transport reading the scratch moves a use across the moved
    // definition, so the store lands at the crossed block's end.
    let scratch_argument = mutated_chained(target, |function, environment| {
        pack_store(function, environment);
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
                parameter: SCRATCH,
            },
        });
    });
    let result = sink(&scratch_argument, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // An edge transport rewriting the scratch is a second definition the
    // moved store's would overtake.
    let scratch_parameter = mutated_chained(target, |function, environment| {
        pack_store(function, environment);
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
                parameter: PACKED_SCRATCH,
            },
        });
    });
    let result = sink(&scratch_parameter, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // A case-payload register transport reading the scratch bounds the same
    // crossing.
    let payload = mutated_chained(target, |function, environment| {
        pack_store(function, environment);
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
                    parameter: SCRATCH,
                },
            }],
            trivial_affine_discards: Vec::new(),
        });
    });
    let result = sink(&payload, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // A flag-reading terminator keeps the packed store on the publishing
    // side of the edge even when every leg reaches the covering block.
    let flagged = |target: NativeTarget| {
        mutated_chained(target, |function, environment| {
            pack_store(function, environment);
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
        })
    };
    let result = sink(&flagged(target), &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // On a target whose packed row publishes no condition state the same
    // flag-reading terminator does not couple, and the packed store crosses.
    let flag_free_target = NativeTarget::linux_arm64();
    let flag_free_environment = baseline_target_register_environment(flag_free_target).unwrap();
    let result = sink(&flagged(flag_free_target), &flag_free_environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
}

/// The direct slot store crosses the edge like the place store: a `Store64`
/// into the place's own parameter home carries just the value register, so
/// an edge transport redefining the unused pointer register cannot stop it,
/// while one redefining the carried value still lands it at the crossed
/// block's end.
#[test]
fn cross_block_local_slot_store_sinks_across_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let slot = LocalStorageSlotId::StructuralParameter { place: place() };
    let direct = mutated_chained(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    let result = sink(&direct, &environment).unwrap();
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
        vec![STORE, KILLER]
    );
    assert_eq!(
        function.memory_accesses,
        direct.transformed().functions[0].memory_accesses
    );
    validate_store_mutation_motion(
        &direct,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The pointer register is not carried by the slot store, so an edge
    // redefining it cannot reorder what the moved store reads.
    let pointer_edge = mutated_chained(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
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
                parameter: POINTER,
            },
        });
    });
    let result = sink(&pointer_edge, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
    // The carried value register keeps the same veto the place store had.
    let value_edge = mutated_chained(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
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
    let result = sink(&value_edge, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
}

#[test]
fn cross_block_byte_sequence_store_sinks_across_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The byte-sequence store's reach is unbounded upward from its payload
    // base: the covering store's exact write ends at that base, so the moved
    // byte never meets it and the store slides to the successor's end.
    let sunk = mutated_chained(target, |function, environment| {
        sequence_store(function, environment, 8);
    });
    let result = sink(&sunk, &environment).unwrap();
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
        vec![KILLER, STORE]
    );
    assert_eq!(
        function.memory_accesses,
        sunk.transformed().functions[0].memory_accesses
    );
    validate_store_mutation_motion(
        &sunk,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A covering write reaching the payload base can still meet the moved
    // byte, so the store lands at the successor's head before it.
    let reached = mutated_chained(target, |function, environment| {
        sequence_store(function, environment, 8);
        function.memory_accesses[1].byte_offset = 8;
    });
    let result = sink(&reached, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
    // A terminator row on the moved place reaching the payload base lands
    // the store at the crossed block's end.
    let terminator_row = mutated_chained(target, |function, environment| {
        sequence_store(function, environment, 8);
        let SelectedTerminator::Jump {
            instruction: terminator,
            ..
        } = &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        function.memory_accesses.push(access(
            terminator.id,
            3,
            place(),
            8,
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    let result = sink(&terminator_row, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // An edge transport writing the moved place's storage runs between the
    // old and new positions however far the moved byte reaches.
    let aliased = mutated_chained(target, |function, environment| {
        sequence_store(function, environment, 8);
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
    let result = sink(&aliased, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // An edge redefining the carried address register still stops the
    // crossing: the moved store's reads keep the same custody.
    let redefined = mutated_chained(target, |function, environment| {
        sequence_store(function, environment, 8);
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
                parameter: POINTER,
            },
        });
    });
    let result = sink(&redefined, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
}

/// A byte-sequence row's resolved `index` decides the crossed walk the same
/// way it decides the in-block one: the row touches exactly the byte
/// `byte_offset + index`, so a landing off the moved extent lets the store
/// slide past it to the target block's end, while a landing on it lands the
/// store at the successor's head. The moved store's own resolved index
/// collapses its extent to that one byte before the walk, and an edge
/// transport redefining the index's carrier keeps the extent dynamic.
#[test]
fn cross_block_constant_index_rows_decide_by_the_landing_byte() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let block_ids = |result: &ValidatedStoreMutationMotion, block: usize| {
        result.transformed().functions[0].blocks[block]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>()
    };
    // The covering block's sequence row resolves to `4 + 8` = 12 — off the
    // moved range [0, 8) — so the store slides past the covering
    // instruction to the successor's end.
    let off = mutated_chained(target, |function, environment| {
        function.memory_accesses[1] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(
                KILLER,
                2,
                place(),
                4,
                SelectedMemoryAccessRole::WriteByteSequence {
                    index: ValueId::new(9).unwrap(),
                    value: ValueId::new(6).unwrap(),
                    length: ValueId::new(7).unwrap(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32],
                    ),
                },
            )
        };
        define_index(
            function,
            environment,
            1,
            0,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            8,
        );
    });
    let result = sink(&off, &environment).unwrap();
    assert_eq!(
        block_ids(&result, 0),
        vec![SelectedInstructionId(1), BETWEEN]
    );
    assert_eq!(
        block_ids(&result, 1),
        vec![MATERIALIZE_INDEX, KILLER, STORE]
    );
    validate_store_mutation_motion(
        &off,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The same row resolving to `4 + 2` = 6 lands inside the moved range,
    // so the store lands at the successor's head before it.
    let on = mutated_chained(target, |function, environment| {
        function.memory_accesses[1] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(
                KILLER,
                2,
                place(),
                4,
                SelectedMemoryAccessRole::WriteByteSequence {
                    index: ValueId::new(9).unwrap(),
                    value: ValueId::new(6).unwrap(),
                    length: ValueId::new(7).unwrap(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32],
                    ),
                },
            )
        };
        define_index(
            function,
            environment,
            1,
            0,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            2,
        );
    });
    let result = sink(&on, &environment).unwrap();
    assert_eq!(
        block_ids(&result, 1),
        vec![MATERIALIZE_INDEX, STORE, KILLER]
    );
    // The moved store's own resolved index collapses its extent: payload
    // base 4 plus index 8 lands the moved byte at 12, disjoint from the
    // covering write's [0, 8), so the store sinks to the successor's end.
    let collapsed = mutated_chained(target, |function, environment| {
        sequence_store(function, environment, 4);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_MOVED_INDEX,
            MOVED_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            8,
        );
    });
    let result = sink(&collapsed, &environment).unwrap();
    assert_eq!(
        block_ids(&result, 0),
        vec![SelectedInstructionId(1), MATERIALIZE_MOVED_INDEX, BETWEEN]
    );
    assert_eq!(block_ids(&result, 1), vec![KILLER, STORE]);
    validate_store_mutation_motion(
        &collapsed,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The same resolved index landing the moved byte at `4 + 2` = 6 keeps
    // the write inside the covering range: the store lands at the head.
    let collapsed_on = mutated_chained(target, |function, environment| {
        sequence_store(function, environment, 4);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_MOVED_INDEX,
            MOVED_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            2,
        );
    });
    let result = sink(&collapsed_on, &environment).unwrap();
    assert_eq!(block_ids(&result, 1), vec![STORE, KILLER]);
    // An edge transport redefining the index's carrier falsifies the
    // resolved constant: the extent stays dynamic from payload base 4, so
    // the covering write's [0, 8) still reaches it and the store lands at
    // the successor's head.
    let redefined_index = mutated_chained(target, |function, environment| {
        sequence_store(function, environment, 4);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_MOVED_INDEX,
            MOVED_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            8,
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
                parameter: MOVED_SEQUENCE_INDEX,
            },
        });
    });
    let result = sink(&redefined_index, &environment).unwrap();
    assert_eq!(
        block_ids(&result, 0),
        vec![SelectedInstructionId(1), MATERIALIZE_MOVED_INDEX, BETWEEN]
    );
    assert_eq!(block_ids(&result, 1), vec![STORE, KILLER]);
}

#[test]
fn cross_block_joins_forks_and_cycles_land_at_the_block_end() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second predecessor into the covering block means paths that never
    // carried the write would gain it, so the store only reaches block 0's
    // end.
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
    let result = sink(&join, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // Edges fanning out to distinct blocks admit a path the moved write never
    // runs on; the store lands at the forked block's end.
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
    let result = sink(&forked, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // A successor that can reach back to the walked chain closes a cycle
    // whose unverified interval could reorder an access across the moved
    // write; the store lands at the last proven block's end instead.
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
    let result = sink(&cycled, &environment).unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    assert!(function.blocks[1].instructions.is_empty());
    // A successor naming no block cannot be walked.
    let dangling = mutated_chained(target, |function, _| {
        crossed_edge(function).block = SelectedBlockId(99);
    });
    assert_eq!(
        sink(&dangling, &environment).unwrap_err(),
        StoreMutationMotionError::SourceMismatch
    );
}

#[test]
fn cross_block_edge_transports_and_terminator_rows_decide() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Register transports cannot reach memory; carrying an unrelated register
    // across the edge still sinks.
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
                parameter: VirtualRegisterId(9),
            },
        });
    });
    let result = sink(&carried, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
    // An edge that redefines the carried value register changes what the
    // moved store would read; the store lands at the crossed block's end.
    let redefined = mutated_chained(target, |function, _| {
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
    let result = sink(&redefined, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // A structural write into the moved place's storage runs between the old
    // and new positions.
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
    let result = sink(&aliased, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // Case custody on the moved place writes its slot inside the interval.
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
    let result = sink(&custody, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // A case payload's register parameter redefining the pointer register
    // stops the crossing the same way an edge binding does.
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
                    parameter: POINTER,
                },
            }],
            trivial_affine_discards: Vec::new(),
        });
    });
    let result = sink(&payload, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // A roster row on the crossed terminator's instruction lands the store at
    // the block's end before the terminator runs.
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
    let result = sink(&terminator_write, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // A dynamic-extent row on the crossed terminator reaches only upward
    // from its fixed offset, so starting at the moved range's end it is
    // provably disjoint and the store still crosses into the covering
    // block; one byte earlier the moved range's last byte stays reachable
    // and the store lands at the crossed block's end.
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
    let result = sink(&terminator_above, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
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
    let result = sink(&terminator_inside, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
}

#[test]
fn cross_block_settlements_shift_over_the_inserted_ordinal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A settlement at the covering block's head sits inside the landing slot;
    // inserting the store there shifts it one ordinal later so it still names
    // the covering store.
    let head = mutated_chained(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement_at(SelectedBlockId(1), 0));
    });
    let result = sink(&head, &environment).unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![STORE, KILLER]
    );
    assert_eq!(
        function
            .boundary_settlements
            .iter()
            .map(|settlement| (settlement.block, settlement.instruction_index))
            .collect::<Vec<_>>(),
        vec![(SelectedBlockId(1), 1)]
    );
    // A settlement inside block 0's remaining interval cannot be passed: the
    // boundary event must stay ordered after the write, so the walk stops
    // before it — here that leaves the store nowhere to move.
    let inside = mutated_chained(target, |function, _| {
        function.boundary_settlements.push(settlement(2));
    });
    assert_eq!(
        sink(&inside, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A settlement in the after-body slot of the crossed block bounds the
    // motion at the block's end and keeps its own position.
    let tail = mutated_chained(target, |function, _| {
        function.boundary_settlements.push(settlement(3));
    });
    let result = sink(&tail, &environment).unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    assert_eq!(
        function
            .boundary_settlements
            .iter()
            .map(|settlement| (settlement.block, settlement.instruction_index))
            .collect::<Vec<_>>(),
        vec![(SelectedBlockId(0), 3)]
    );
}

/// Two runs over the identical chained source produce the identical
/// validated result, and the published cross-block plan is a legal second
/// input: the moved store already sits at the successor's head immediately
/// before the covering store, so its next provable position is its own
/// index, and the covering store's terminator has no successor to sink
/// toward.
#[test]
fn cross_block_motion_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = sink(&chained(target), &environment).unwrap();
    let second = sink(&chained(target), &environment).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one. Re-running on
    // it is terminal.
    assert_eq!(
        sink_selected_store_mutation(&first, 0, STORE, &environment, budget()).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    assert_eq!(
        sink_selected_store_mutation(&first, 0, KILLER, &environment, budget()).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
}

/// A proposed cross-block result must reproduce the exact motion: drift in
/// the crossed block, in the landing block's order, in the retained roster,
/// or in the shifted settlements mismatches, and moving the store back must
/// restore the complete source by content.
#[test]
fn cross_block_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = chained(target);
    let result = sink(&source, &environment).unwrap();
    for mutation in 0..6 {
        let mut proposed = result.transformed().clone();
        match mutation {
            // Drift in the crossed block the motion left behind mismatches.
            0 => {
                proposed.functions[0].blocks[0].instructions.pop();
            }
            // The store must land at the successor's head, not after the
            // covering store.
            1 => {
                proposed.functions[0].blocks[1].instructions.swap(0, 1);
            }
            // The covering store must still be there.
            2 => {
                proposed.functions[0].blocks[1].instructions.pop();
            }
            // The roster is retained unchanged; a dropped row mismatches.
            3 => {
                proposed.functions[0].memory_accesses.pop();
            }
            // A phantom settlement cannot appear in the proposal.
            4 => {
                proposed.functions[0]
                    .boundary_settlements
                    .push(settlement(0));
            }
            // A phantom block cannot appear in the proposal.
            _ => {
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
            }
        }
        assert_eq!(
            validate_store_mutation_motion(&source, 0, STORE, &environment, budget(), proposed,)
                .unwrap_err(),
            StoreMutationMotionError::ReplayMismatch
        );
    }
}
