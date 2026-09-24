use super::control_flow::successor;
use super::{
    Arc, BlockId, EdgeId, NativeTarget, SelectedBlock, SelectedBlockId, SelectedFunction,
    SelectedInstructionId, SelectedInstructionKind, SelectedTerminator, ValueDefinitionSite,
    ValueId, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    baseline_target_register_environment, fixture, selected_instruction_plan_identity,
};
use crate::RuntimeSpillError;
use crate::ValidatedRuntimeSpill;
use crate::rewrites::runtime_spill::admission;
use crate::rewrites::runtime_spill::tests::budget;
use crate::spill_selected_runtime_value;
use crate::validate_runtime_spill;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockOrigin, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedCasePayloadBinding, SelectedCasePayloadTransport,
    SelectedLocalStorageSlot, SelectedStructuralCaseEdge, SelectedSuccessor, SelectedSuccessorRole,
    SelectedValueBinding, SelectedValueTransport,
};
use semantic_vocabulary::{
    BoundaryMachineId, OperationId, PlaceId, StructuralCaseId, StructuralFieldId,
};

pub(super) fn parameter_fixture(target: NativeTarget) -> ValidatedRuntimeSpill {
    let mut source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let jump = environment.constraint(keys.jump).unwrap();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let scalar_type = function.virtual_registers[0].scalar_type;
    let mut terminal = function.blocks[0].terminator.clone();
    let SelectedTerminator::Return { instruction, .. } = &mut terminal else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(1000);
    function.virtual_registers.truncate(1);
    function.virtual_registers.push(VirtualRegister {
        id: VirtualRegisterId(1),
        scalar_type,
        class: copy.operands[0].class,
        origin: VirtualRegisterOrigin::BlockParameter {
            source_value: ValueId::new(2).unwrap(),
            block: SelectedBlockId(2),
            parameter_index: 0,
        },
        definition_site: Some(ValueDefinitionSite::BlockParameter {
            block: BlockId::new(3).unwrap(),
            position: 0,
        }),
        entry_fixed_view: None,
    });
    for (register, instruction, source_value) in [
        (2, 201, 1),
        (3, 301, 1),
        (4, 202, 1),
        (5, 401, 2),
        (6, 402, 2),
    ] {
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(register),
            scalar_type,
            class: copy.operands[0].class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(instruction),
                source_value: ValueId::new(source_value).unwrap(),
            },
            definition_site: Some(if source_value == 1 {
                ValueDefinitionSite::FunctionParameter(0)
            } else {
                ValueDefinitionSite::BlockParameter {
                    block: BlockId::new(3).unwrap(),
                    position: 0,
                }
            }),
            entry_fixed_view: None,
        });
    }
    let incoming = |argument| {
        let mut edge = successor(2);
        edge.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(2).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type,
            },
            transport: SelectedValueTransport::Registers {
                argument: VirtualRegisterId(argument),
                parameter: VirtualRegisterId(1),
            },
        });
        edge
    };
    let instruction = |identity, input, output| {
        admission::instruction(
            SelectedInstructionId(identity),
            SelectedInstructionKind::CopyI64,
            copy,
            &[VirtualRegisterId(input), VirtualRegisterId(output)],
        )
    };
    let edge_block = |block, argument, copy_id, jump_id| SelectedBlock {
        id: SelectedBlockId(block),
        origin: SelectedBlockOrigin::EdgeTransfer {
            edge: EdgeId::new(2).unwrap(),
            target: BlockId::new(3).unwrap(),
        },
        instructions: vec![instruction(copy_id, 0, argument)],
        terminator: SelectedTerminator::Jump {
            instruction: admission::instruction(
                SelectedInstructionId(jump_id),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            successor: incoming(argument),
        },
    };
    function.blocks = vec![
        SelectedBlock {
            id: SelectedBlockId(0),
            origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: admission::instruction(
                    SelectedInstructionId(100),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                when_nonzero: successor(1),
                when_zero: successor(3),
            },
        },
        edge_block(1, 2, 201, 200),
        SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: vec![instruction(401, 1, 5), instruction(402, 1, 6)],
            terminator: terminal,
        },
        edge_block(3, 3, 301, 300),
    ];
    function.blocks[1].instructions.push(instruction(202, 0, 4));
    for (block, instruction_index) in [(0, 0), (1, 1), (1, 2), (2, 0), (2, 2), (3, 1)] {
        function
            .boundary_settlements
            .push(SelectedBoundarySettlement {
                block: SelectedBlockId(block),
                instruction_index,
                settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                    operation: OperationId::new(1).unwrap(),
                    boundary: BoundaryMachineId::new(1).unwrap(),
                    source: ValueId::new(1).unwrap(),
                },
            });
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// A case-payload parameter: each predecessor is a case-dispatch bridge whose
/// continuation binds the destination's parameter through a `Registers`
/// payload transport. The bridge's own `FrameAddress`/`Load64` pair observes
/// the field, so the edge-exact store lands right after that load — the same
/// position an edge copy's output would occupy.
pub(super) fn case_parameter_fixture(target: NativeTarget) -> ValidatedRuntimeSpill {
    let mut source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let address = environment.constraint(keys.frame_address.unwrap()).unwrap();
    let load = environment.constraint(keys.load64.unwrap()).unwrap();
    let jump = environment.constraint(keys.jump).unwrap();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let scalar_type = function.virtual_registers[0].scalar_type;
    let class = copy.operands[0].class;
    let place = PlaceId::new(1).unwrap();
    let slot = LocalStorageSlotId::Structural {
        operation: OperationId::new(1).unwrap(),
        place,
    };
    function.local_storage_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: 8,
        alignment: 8,
    });
    let mut terminal = function.blocks[0].terminator.clone();
    let SelectedTerminator::Return { instruction, .. } = &mut terminal else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(1000);
    let site = ValueDefinitionSite::BlockParameter {
        block: BlockId::new(3).unwrap(),
        position: 0,
    };
    function.virtual_registers.truncate(1);
    function.virtual_registers.push(VirtualRegister {
        id: VirtualRegisterId(1),
        scalar_type,
        class,
        origin: VirtualRegisterOrigin::BlockParameter {
            source_value: ValueId::new(2).unwrap(),
            block: SelectedBlockId(2),
            parameter_index: 0,
        },
        definition_site: Some(site),
        entry_fixed_view: None,
    });
    // Each bridge observes the field through an `AbiTransport` frame pointer
    // and a `StructuralObservation` load result, exactly as edge construction
    // emits them.
    for (register, instruction, observation) in [
        (2u32, 201u32, false),
        (3, 202, true),
        (4, 301, false),
        (5, 302, true),
    ] {
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(register),
            scalar_type,
            class,
            origin: if observation {
                VirtualRegisterOrigin::StructuralObservation {
                    instruction: SelectedInstructionId(instruction),
                    place,
                    byte_offset: 0,
                }
            } else {
                VirtualRegisterOrigin::AbiTransport {
                    instruction: SelectedInstructionId(instruction),
                    place,
                    byte_offset: 0,
                }
            },
            definition_site: None,
            entry_fixed_view: None,
        });
    }
    for (register, instruction) in [(6u32, 401u32), (7, 402)] {
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(register),
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(instruction),
                source_value: ValueId::new(2).unwrap(),
            },
            definition_site: Some(site),
            entry_fixed_view: None,
        });
    }
    let declaration = || legalized_operations::LegalizedStructuralCasePayload {
        field: StructuralFieldId::new(1).unwrap(),
        field_byte_offset: 0,
        parameter: legalized_operations::LegalizedValueDefinition {
            value: ValueId::new(2).unwrap(),
            scalar_type,
            definition_site: site,
        },
    };
    let case_edge = |payloads| SelectedStructuralCaseEdge {
        slot,
        case: StructuralCaseId::new(1).unwrap(),
        case_tag: 0,
        trivial_affine_discards: Vec::new(),
        payloads,
    };
    // The semantic edge keeps the case declaration with its payloads all
    // `Unused` — edge construction moves materialization into the bridge.
    let semantic_edge = |bridge: u32, psi_edge: u64| {
        let mut edge = successor(bridge);
        edge.role = SelectedSuccessorRole::Semantic;
        edge.psi_edge = EdgeId::new(psi_edge).unwrap();
        edge.structural_case = Some(case_edge(vec![SelectedCasePayloadBinding {
            semantic: declaration(),
            transport: SelectedCasePayloadTransport::Unused,
        }]));
        edge
    };
    let bridge = |block: u32, psi_edge: u64, address_id: u32, load_id: u32, loaded: u32| {
        let mut continuation = successor(2);
        continuation.psi_edge = EdgeId::new(psi_edge).unwrap();
        continuation.structural_case = Some(case_edge(vec![SelectedCasePayloadBinding {
            semantic: declaration(),
            transport: SelectedCasePayloadTransport::Registers {
                argument: VirtualRegisterId(loaded),
                parameter: VirtualRegisterId(1),
            },
        }]));
        SelectedBlock {
            id: SelectedBlockId(block),
            origin: SelectedBlockOrigin::EdgeTransfer {
                edge: EdgeId::new(psi_edge).unwrap(),
                target: BlockId::new(3).unwrap(),
            },
            instructions: vec![
                admission::instruction(
                    SelectedInstructionId(address_id),
                    SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset: 0,
                    },
                    address,
                    &[VirtualRegisterId(if address_id == 201 { 2 } else { 4 })],
                ),
                admission::instruction(
                    SelectedInstructionId(load_id),
                    SelectedInstructionKind::Load64 { byte_offset: 0 },
                    load,
                    &[
                        VirtualRegisterId(if address_id == 201 { 2 } else { 4 }),
                        VirtualRegisterId(loaded),
                    ],
                ),
            ],
            terminator: SelectedTerminator::Jump {
                instruction: admission::instruction(
                    SelectedInstructionId(address_id - 1),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: continuation,
            },
        }
    };
    let consumer = |instruction, input, output| {
        admission::instruction(
            SelectedInstructionId(instruction),
            SelectedInstructionKind::CopyI64,
            copy,
            &[VirtualRegisterId(input), VirtualRegisterId(output)],
        )
    };
    function.blocks = vec![
        SelectedBlock {
            id: SelectedBlockId(0),
            origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: admission::instruction(
                    SelectedInstructionId(100),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                when_nonzero: semantic_edge(1, 2),
                when_zero: semantic_edge(3, 3),
            },
        },
        bridge(1, 2, 201, 202, 3),
        SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: vec![consumer(401, 1, 6), consumer(402, 1, 7)],
            terminator: terminal,
        },
        bridge(3, 3, 301, 302, 5),
    ];
    for (block, instruction_index) in [(0, 0), (1, 1), (2, 0), (3, 1)] {
        function
            .boundary_settlements
            .push(SelectedBoundarySettlement {
                block: SelectedBlockId(block),
                instruction_index,
                settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                    operation: OperationId::new(1).unwrap(),
                    boundary: BoundaryMachineId::new(1).unwrap(),
                    source: ValueId::new(1).unwrap(),
                },
            });
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

fn incoming(function: &mut SelectedFunction, block_index: usize) -> &mut SelectedSuccessor {
    let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[block_index].terminator
    else {
        unreachable!()
    };
    successor
}

#[test]
fn every_incoming_copy_stores_before_later_copies_and_preserves_parameter_bindings() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        for reordered in [false, true] {
            let mut source = parameter_fixture(target);
            if reordered {
                Arc::make_mut(&mut source.transformed).functions[0]
                    .blocks
                    .swap(0, 3);
            }
            let environment = baseline_target_register_environment(target).unwrap();
            let result = spill_selected_runtime_value(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
            )
            .unwrap();
            let original = &source.transformed().functions[0];
            let transformed = &result.transformed().functions[0];
            assert_eq!(transformed.local_storage_slots.len(), 1);
            for (before, after) in original.blocks.iter().zip(&transformed.blocks) {
                assert_eq!(before.terminator, after.terminator);
                match before.id.0 {
                    1 | 3 => {
                        assert_eq!(after.instructions[0], before.instructions[0]);
                        assert!(matches!(
                            after.instructions[1].kind,
                            SelectedInstructionKind::Store64 { .. }
                        ));
                        assert_eq!(
                            after.instructions[1].operands[0].virtual_register,
                            VirtualRegisterId(if before.id.0 == 1 { 2 } else { 3 })
                        );
                        assert_eq!(&after.instructions[2..], &before.instructions[1..]);
                    }
                    2 => {
                        // One shared pair serves both body uses.
                        assert_eq!(after.instructions.len(), 4);
                        assert_eq!(
                            after.instructions[2].operands[0].virtual_register,
                            after.instructions[3].operands[0].virtual_register
                        );
                    }
                    _ => assert_eq!(before, after),
                }
            }
            assert_eq!(
                transformed
                    .boundary_settlements
                    .iter()
                    .map(|settlement| settlement.instruction_index)
                    .collect::<Vec<_>>(),
                [0, 2, 3, 2, 4, 2]
            );
            for reload in transformed
                .virtual_registers
                .iter()
                .skip(original.virtual_registers.len())
                .filter(|value| {
                    matches!(
                        value.origin,
                        VirtualRegisterOrigin::InstructionResult { .. }
                    )
                })
            {
                assert!(
                    matches!(reload.origin, VirtualRegisterOrigin::InstructionResult { source_value, .. } if source_value == ValueId::new(2).unwrap())
                );
                assert_eq!(
                    reload.definition_site,
                    original.virtual_registers[1].definition_site
                );
            }
        }
    }
}

#[test]
fn parameter_terminator_uses_reload_from_edge_initialized_storage() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let mut source = parameter_fixture(target);
        {
            // The destination's return operand consumes the edge-initialized
            // parameter through the ABI's pinned result register.
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            function.blocks[2].terminator = SelectedTerminator::Return {
                instruction: admission::instruction(
                    SelectedInstructionId(2000),
                    SelectedInstructionKind::ReturnScalar,
                    environment
                        .constraint(environment.selected_keys().return_i64)
                        .unwrap(),
                    &[VirtualRegisterId(1)],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            };
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let original = &source.transformed().functions[0];
        let transformed = &result.transformed().functions[0];
        let block = &transformed.blocks[2];
        // The two body uses share one pair; the pinned terminator use keeps a
        // private pair that lands after every body instruction.
        assert_eq!(
            block.instructions.len(),
            original.blocks[2].instructions.len() + 4
        );
        assert!(matches!(
            block.instructions[4].kind,
            SelectedInstructionKind::FrameAddress { .. }
        ));
        assert!(matches!(
            block.instructions[5].kind,
            SelectedInstructionKind::Load64 { .. }
        ));
        let terminator = super::super::control(&block.terminator).0;
        assert_eq!(
            terminator.operands[0].virtual_register,
            block.instructions[5].operands[1].virtual_register
        );
        assert_ne!(
            terminator.operands[0].virtual_register,
            block.instructions[1].operands[1].virtual_register
        );
        assert_eq!(
            transformed
                .boundary_settlements
                .iter()
                .map(|settlement| settlement.instruction_index)
                .collect::<Vec<_>>(),
            [0, 2, 3, 2, 6, 2]
        );
        assert!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone()
            )
            .is_ok()
        );
    }
}

#[test]
fn edge_initialized_parameters_transport_through_fresh_reload_registers() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let mut source = parameter_fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            let scalar_type = function.virtual_registers[1].scalar_type;
            let class = function.virtual_registers[1].class;
            // The destination parameter is transported onward to a new exit
            // block's own parameter through an ordinary edge binding.
            function.virtual_registers.push(VirtualRegister {
                id: VirtualRegisterId(7),
                scalar_type,
                class,
                origin: VirtualRegisterOrigin::BlockParameter {
                    source_value: ValueId::new(3).unwrap(),
                    block: SelectedBlockId(4),
                    parameter_index: 0,
                },
                definition_site: Some(ValueDefinitionSite::BlockParameter {
                    block: BlockId::new(4).unwrap(),
                    position: 0,
                }),
                entry_fixed_view: None,
            });
            let terminal = function.blocks[2].terminator.clone();
            let jump = environment
                .constraint(environment.selected_keys().jump)
                .unwrap();
            let mut onward = successor(4);
            onward.role = SelectedSuccessorRole::Semantic;
            onward.source_target = BlockId::new(4).unwrap();
            onward.bindings.push(SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: ValueId::new(3).unwrap(),
                    argument: ValueId::new(2).unwrap(),
                    scalar_type,
                },
                transport: SelectedValueTransport::Registers {
                    argument: VirtualRegisterId(1),
                    parameter: VirtualRegisterId(7),
                },
            });
            function.blocks[2].terminator = SelectedTerminator::Jump {
                instruction: admission::instruction(
                    SelectedInstructionId(2000),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: onward,
            };
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(4),
                origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
                instructions: Vec::new(),
                terminator: terminal,
            });
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let original = &source.transformed().functions[0];
        let transformed = &result.transformed().functions[0];
        // Both edge stores stay; the destination shares one pair across both
        // body uses plus a private one for the outgoing transport.
        for id in [1, 3] {
            assert!(matches!(
                transformed.blocks[id].instructions[1].kind,
                SelectedInstructionKind::Store64 { .. }
            ));
        }
        let block = &transformed.blocks[2];
        assert_eq!(
            block.instructions.len(),
            original.blocks[2].instructions.len() + 4
        );
        let tail = block.instructions.len() - 1;
        assert!(matches!(
            block.instructions[tail].kind,
            SelectedInstructionKind::Load64 { .. }
        ));
        let SelectedTerminator::Jump { successor, .. } = &block.terminator else {
            unreachable!()
        };
        assert_eq!(
            successor.bindings[0].transport,
            SelectedValueTransport::Registers {
                argument: block.instructions[tail].operands[1].virtual_register,
                parameter: VirtualRegisterId(7),
            }
        );
        assert!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone()
            )
            .is_ok()
        );
    }
}

#[test]
fn incomplete_or_nonlocal_parameter_initialization_is_rejected() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for mutation in 0..12 {
        let mut source = parameter_fixture(NativeTarget::linux_x64());
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        match mutation {
            0 => incoming(function, 3).bindings.clear(),
            1 => {
                let binding = incoming(function, 3).bindings[0].clone();
                incoming(function, 3).bindings.push(binding);
            }
            2 => incoming(function, 3).role = SelectedSuccessorRole::Semantic,
            3 => function.blocks[3].origin = SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            4 => function.entry_block = SelectedBlockId(2),
            5 => {
                function.virtual_registers[1].definition_site =
                    Some(ValueDefinitionSite::BlockParameter {
                        block: BlockId::new(3).unwrap(),
                        position: 1,
                    })
            }
            6 => {
                let definition = function.blocks[3].instructions.remove(0);
                function.blocks[0].instructions.push(definition);
            }
            7 => function.blocks[3].instructions[0].kind = SelectedInstructionKind::CompareI64,
            8 => {
                let mut duplicate = function.blocks[3].instructions[0].clone();
                duplicate.id = SelectedInstructionId(302);
                function.blocks[3].instructions.push(duplicate);
            }
            9 => {
                let use_instruction = function.blocks[2].instructions.pop().unwrap();
                function.blocks[3].instructions.push(use_instruction);
            }
            10 => incoming(function, 3).bindings[0].semantic.argument = ValueId::new(99).unwrap(),
            11 => {
                let edge = incoming(function, 3).clone();
                let instruction = super::super::control(&function.blocks[3].terminator)
                    .0
                    .clone();
                function.blocks[3].terminator = SelectedTerminator::ConditionalBranch {
                    instruction,
                    when_nonzero: edge.clone(),
                    when_zero: edge,
                };
            }
            _ => unreachable!(),
        }
        assert!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn parameter_replay_rejects_missing_late_wrong_and_rebound_incoming_stores() {
    let source = parameter_fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    for mutation in 0..7 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            0 => {
                function.blocks[3].instructions.remove(1);
            }
            1 => function.blocks[1].instructions.swap(1, 2),
            2 => {
                function.blocks[3].instructions[1].operands[0].virtual_register =
                    VirtualRegisterId(2)
            }
            3 => incoming(function, 3).bindings[0].transport = SelectedValueTransport::Unused,
            4 => {
                function.blocks[2].instructions[2].operands[0].virtual_register =
                    VirtualRegisterId(1)
            }
            5 => function.boundary_settlements[1].instruction_index = 1,
            6 => {
                function.virtual_registers[8].definition_site =
                    Some(ValueDefinitionSite::FunctionParameter(0))
            }
            _ => unreachable!(),
        }
        assert_eq!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                proposed
            )
            .unwrap_err(),
            RuntimeSpillError::ReplayMismatch,
            "mutation {mutation}"
        );
    }
}

#[test]
fn case_payload_initialized_parameters_store_after_each_field_load() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = case_parameter_fixture(target);
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let transformed = &result.transformed().functions[0];
        // The declared structural slot stays; the victim's private spill slot
        // is appended once.
        assert_eq!(transformed.local_storage_slots.len(), 2);
        assert_eq!(
            transformed.local_storage_slots[1].id,
            LocalStorageSlotId::Spill {
                register: VirtualRegisterId(1)
            }
        );
        // Each bridge stores its own field observation right after the load
        // that produced it — the same edge-exact position an edge copy's
        // output would occupy.
        for (block_index, loaded) in [(1usize, 3u32), (3, 5)] {
            let block = &transformed.blocks[block_index];
            assert_eq!(block.instructions.len(), 3);
            assert!(matches!(
                block.instructions[2].kind,
                SelectedInstructionKind::Store64 { .. }
            ));
            assert_eq!(
                block.instructions[2].operands[0].virtual_register,
                VirtualRegisterId(loaded)
            );
        }
        // The destination's two body uses share one block-local reload pair.
        let destination = &transformed.blocks[2];
        assert_eq!(destination.instructions.len(), 4);
        assert!(matches!(
            destination.instructions[0].kind,
            SelectedInstructionKind::FrameAddress { .. }
        ));
        assert!(matches!(
            destination.instructions[1].kind,
            SelectedInstructionKind::Load64 { .. }
        ));
        assert_eq!(
            destination.instructions[2].operands[0].virtual_register,
            destination.instructions[1].operands[1].virtual_register
        );
        assert_eq!(
            destination.instructions[3].operands[0].virtual_register,
            destination.instructions[2].operands[0].virtual_register
        );
        // The payload transports stay exact: the parameter keeps its binding
        // and the bridge's observation stays the argument.
        for (block_index, loaded) in [(1usize, 3u32), (3, 5)] {
            let SelectedTerminator::Jump { successor, .. } =
                &transformed.blocks[block_index].terminator
            else {
                unreachable!()
            };
            let case = successor.structural_case.as_ref().unwrap();
            assert_eq!(
                case.payloads[0].transport,
                SelectedCasePayloadTransport::Registers {
                    argument: VirtualRegisterId(loaded),
                    parameter: VirtualRegisterId(1),
                }
            );
        }
        // The settlements naming each bridge's field load stay put; the
        // destination's first-use settlement moves past its reload pair.
        assert_eq!(
            transformed
                .boundary_settlements
                .iter()
                .map(|settlement| settlement.instruction_index)
                .collect::<Vec<_>>(),
            [0, 1, 2, 1]
        );
        assert!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone()
            )
            .is_ok()
        );
        for mutation in 0..5 {
            let mut proposed = result.transformed().clone();
            let function = &mut proposed.functions[0];
            match mutation {
                // Dropping a bridge's store leaves the slot stale on that
                // arrival: replay requires every edge-exact store.
                0 => {
                    function.blocks[1].instructions.remove(2);
                }
                // The store must follow the load that produced the
                // observation.
                1 => function.blocks[3].instructions.swap(1, 2),
                // The stored register must be the bridge's own field
                // observation, not its frame pointer.
                2 => {
                    function.blocks[1].instructions[2].operands[0].virtual_register =
                        VirtualRegisterId(2)
                }
                // The payload's declared argument cannot silently change.
                3 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[1].terminator
                    else {
                        unreachable!()
                    };
                    successor.structural_case.as_mut().unwrap().payloads[0].transport =
                        SelectedCasePayloadTransport::Registers {
                            argument: VirtualRegisterId(4),
                            parameter: VirtualRegisterId(1),
                        };
                }
                // The private slot must be appended exactly once.
                4 => {
                    function.local_storage_slots.pop();
                }
                _ => unreachable!(),
            }
            assert_eq!(
                validate_runtime_spill(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    proposed
                )
                .unwrap_err(),
                RuntimeSpillError::ReplayMismatch,
                "{target:?} mutation {mutation}"
            );
        }
    }
}

#[test]
fn case_payload_parameter_arrivals_still_require_exact_edge_definitions() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for mutation in 0..14 {
        let mut source = case_parameter_fixture(NativeTarget::linux_x64());
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let expected = match mutation {
            // The payload declares the parameter but materializes nothing:
            // without the bridge's observation no edge-exact store exists.
            0 => {
                incoming(function, 3)
                    .structural_case
                    .as_mut()
                    .unwrap()
                    .payloads[0]
                    .transport = SelectedCasePayloadTransport::Unmaterialized {
                    parameter: VirtualRegisterId(1),
                };
                RuntimeSpillError::UnsupportedUse
            }
            // An unused payload leaves the parameter uninitialized on that
            // arrival.
            1 => {
                incoming(function, 3)
                    .structural_case
                    .as_mut()
                    .unwrap()
                    .payloads[0]
                    .transport = SelectedCasePayloadTransport::Unused;
                RuntimeSpillError::UnsupportedUse
            }
            // A payload naming a different source value does not initialize
            // this parameter — and no value binding covers the arrival.
            2 => {
                incoming(function, 3)
                    .structural_case
                    .as_mut()
                    .unwrap()
                    .payloads[0]
                    .semantic
                    .parameter
                    .value = ValueId::new(99).unwrap();
                RuntimeSpillError::UnsupportedUse
            }
            // The declared site must equal the parameter's own site.
            3 => {
                incoming(function, 3)
                    .structural_case
                    .as_mut()
                    .unwrap()
                    .payloads[0]
                    .semantic
                    .parameter
                    .definition_site = ValueDefinitionSite::BlockParameter {
                    block: BlockId::new(3).unwrap(),
                    position: 1,
                };
                RuntimeSpillError::UnsupportedUse
            }
            // The transported register must be the bridge's own field
            // observation, not an arbitrary instruction result.
            4 => {
                function.virtual_registers[5].origin = VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(302),
                    source_value: ValueId::new(2).unwrap(),
                };
                RuntimeSpillError::UnsupportedValue
            }
            // An observation at another byte offset does not carry this field.
            5 => {
                function.virtual_registers[5].origin =
                    VirtualRegisterOrigin::StructuralObservation {
                        instruction: SelectedInstructionId(302),
                        place: PlaceId::new(1).unwrap(),
                        byte_offset: 8,
                    };
                RuntimeSpillError::UnsupportedValue
            }
            // The case slot must be the structural place the load observes.
            6 => {
                incoming(function, 3).structural_case.as_mut().unwrap().slot =
                    LocalStorageSlotId::Boundary {
                        operation: OperationId::new(1).unwrap(),
                    };
                RuntimeSpillError::UnsupportedValue
            }
            // The observation's definer must be the edge's own field load
            // idiom, not a copy.
            7 => {
                function.blocks[3].instructions[1].kind = SelectedInstructionKind::CopyI64;
                RuntimeSpillError::UnsupportedUse
            }
            // A second payload for the same source value is not one exact
            // edge definition.
            8 => {
                let payload = incoming(function, 3)
                    .structural_case
                    .as_ref()
                    .unwrap()
                    .payloads[0]
                    .clone();
                incoming(function, 3)
                    .structural_case
                    .as_mut()
                    .unwrap()
                    .payloads
                    .push(payload);
                RuntimeSpillError::UnsupportedUse
            }
            // A semantic edge directly into the destination is not an
            // edge-exact arrival: only the dedicated continuation carries
            // the store.
            9 => {
                incoming(function, 3).role = SelectedSuccessorRole::Semantic;
                RuntimeSpillError::UnsupportedControlFlow
            }
            // A value binding duplicating the payload's source value is a
            // second definition for the same arrival.
            10 => {
                let scalar_type = function.virtual_registers[1].scalar_type;
                incoming(function, 3).bindings.push(SelectedValueBinding {
                    semantic: abstract_operations::ValueBinding {
                        parameter: ValueId::new(2).unwrap(),
                        argument: ValueId::new(1).unwrap(),
                        scalar_type,
                    },
                    transport: SelectedValueTransport::Registers {
                        argument: VirtualRegisterId(5),
                        parameter: VirtualRegisterId(1),
                    },
                });
                RuntimeSpillError::UnsupportedUse
            }
            // The payload must bind the victim register itself.
            11 => {
                incoming(function, 3)
                    .structural_case
                    .as_mut()
                    .unwrap()
                    .payloads[0]
                    .transport = SelectedCasePayloadTransport::Registers {
                    argument: VirtualRegisterId(5),
                    parameter: VirtualRegisterId(6),
                };
                RuntimeSpillError::UnsupportedUse
            }
            // The observed register must be defined exactly once.
            12 => {
                let mut duplicate = function.blocks[3].instructions[1].clone();
                duplicate.id = SelectedInstructionId(303);
                function.blocks[3].instructions.push(duplicate);
                RuntimeSpillError::UnsupportedUse
            }
            // A load in another block is not the edge's own definition.
            13 => {
                let load = function.blocks[3].instructions.remove(1);
                function.blocks[0].instructions.push(load);
                RuntimeSpillError::UnsupportedUse
            }
            _ => unreachable!(),
        };
        assert_eq!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap_err(),
            expected,
            "mutation {mutation}"
        );
    }
}

/// An entry parameter's definition is the function-entry boundary: the
/// register arrives live-in with its ABI view pinned, so the store opens the
/// entry block and the pin keeps covering only the entry-to-store window —
/// every use reads a reload. Replay independently requires that leading
/// store: one displaced into the block body mismatches.
#[test]
fn entry_parameter_stores_at_the_entry_boundary_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let mut source = fixture(target);
        let pinned = {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            // Selection pins the entry live-in to a real view of its class;
            // the pin survives the rewrite as the entry-to-store interval.
            let view = environment
                .physical()
                .model()
                .classes
                .iter()
                .find(|row| row.id == function.virtual_registers[0].class)
                .and_then(|row| row.views.first())
                .copied()
                .expect("the scalar class always declares a view");
            function.virtual_registers[0].entry_fixed_view = Some(view);
            view
        };
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(0), &environment, budget())
                .unwrap();
        let original = &source.transformed().functions[0];
        let transformed = &result.transformed().functions[0];
        let block = &transformed.blocks[0];
        // [store, address, load, copy×4]: the boundary store leads, then the
        // sole use's reload pair precedes its consumer.
        assert_eq!(
            block.instructions.len(),
            original.blocks[0].instructions.len() + 3
        );
        assert!(matches!(
            block.instructions[0].kind,
            SelectedInstructionKind::Store64 { .. }
        ));
        assert_eq!(
            block.instructions[0].operands[0].virtual_register,
            VirtualRegisterId(0)
        );
        assert!(matches!(
            block.instructions[1].kind,
            SelectedInstructionKind::FrameAddress { .. }
        ));
        assert!(matches!(
            block.instructions[2].kind,
            SelectedInstructionKind::Load64 { .. }
        ));
        let reload = block.instructions[2].operands[1].virtual_register;
        assert_eq!(block.instructions[3].operands[0].virtual_register, reload);
        // Nothing past the boundary store still names the victim register.
        assert!(block.instructions.iter().skip(1).all(|instruction| {
            instruction
                .operands
                .iter()
                .all(|operand| operand.virtual_register != VirtualRegisterId(0))
        }));
        assert_eq!(
            transformed.local_storage_slots.as_slice(),
            [SelectedLocalStorageSlot {
                id: LocalStorageSlotId::Spill {
                    register: VirtualRegisterId(0)
                },
                byte_size: 8,
                alignment: 8,
            }]
        );
        // The victim keeps its pinned entry view — its interval is exactly
        // entry-to-store now — while the generated registers carry none.
        assert_eq!(
            transformed.virtual_registers[0].entry_fixed_view,
            Some(pinned)
        );
        assert!(
            transformed.virtual_registers[original.virtual_registers.len()..]
                .iter()
                .all(|register| register.entry_fixed_view.is_none())
        );
        validate_runtime_spill(
            &source,
            0,
            VirtualRegisterId(0),
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // Replay demands the boundary store lead the block: pushing it into
        // the body leaves the first stream instruction a source copy.
        let mut moved = result.transformed().clone();
        moved.functions[0].blocks[0].instructions.swap(0, 3);
        assert_eq!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(0),
                &environment,
                budget(),
                moved
            )
            .unwrap_err(),
            RuntimeSpillError::ReplayMismatch
        );
    }
}

/// An entry parameter's single store is only valid once: any edge back to
/// the entry block would re-execute it reading a register whose interval
/// already ended at the first store, so re-entry stays rejected — whether
/// the edge is the entry block's own loop edge or a later block's back edge.
#[test]
fn entry_parameter_rejects_any_edge_back_into_the_entry_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let jump = environment
        .constraint(environment.selected_keys().jump)
        .unwrap();
    // The entry block's own conditional edge targets itself.
    let mut self_loop = fixture(target);
    {
        let function = &mut Arc::make_mut(&mut self_loop.transformed).functions[0];
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: admission::instruction(
                SelectedInstructionId(50),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            when_nonzero: successor(0),
            when_zero: successor(0),
        };
    }
    assert_eq!(
        spill_selected_runtime_value(&self_loop, 0, VirtualRegisterId(0), &environment, budget())
            .unwrap_err(),
        RuntimeSpillError::UnsupportedControlFlow
    );
    // A back edge from another block — reachable or not — still re-enters the
    // boundary, so the structural scan rejects it the same way.
    let mut back_edge = fixture(target);
    {
        let function = &mut Arc::make_mut(&mut back_edge.transformed).functions[0];
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: admission::instruction(
                    SelectedInstructionId(50),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(0),
            },
        });
    }
    assert_eq!(
        spill_selected_runtime_value(&back_edge, 0, VirtualRegisterId(0), &environment, budget())
            .unwrap_err(),
        RuntimeSpillError::UnsupportedControlFlow
    );
}

/// The boundary definition keeps its own site discipline: a parameter index
/// the site does not name, a block-parameter site, or no site at all leaves
/// the live-in outside admission, and a fixed view on a non-entry origin
/// stays rejected exactly as before.
#[test]
fn entry_parameter_requires_its_boundary_site_and_own_the_fixed_view() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..4 {
        let mut source = fixture(target);
        let victim = {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            match mutation {
                // A parameter index the site does not name is not this
                // boundary definition.
                0 => {
                    function.virtual_registers[0].definition_site =
                        Some(ValueDefinitionSite::FunctionParameter(1));
                    VirtualRegisterId(0)
                }
                // A block-parameter site is not the function-entry boundary.
                1 => {
                    function.virtual_registers[0].definition_site =
                        Some(ValueDefinitionSite::BlockParameter {
                            block: BlockId::new(3).unwrap(),
                            position: 0,
                        });
                    VirtualRegisterId(0)
                }
                // The boundary live-in must still declare its semantic site.
                2 => {
                    function.virtual_registers[0].definition_site = None;
                    VirtualRegisterId(0)
                }
                // A fixed view pins an interval the rewrite cannot shrink on
                // any origin but the boundary live-in itself.
                _ => {
                    let view = environment
                        .physical()
                        .model()
                        .classes
                        .iter()
                        .find(|row| row.id == function.virtual_registers[1].class)
                        .and_then(|row| row.views.first())
                        .copied()
                        .expect("the scalar class always declares a view");
                    function.virtual_registers[1].entry_fixed_view = Some(view);
                    VirtualRegisterId(1)
                }
            }
        };
        assert_eq!(
            spill_selected_runtime_value(&source, 0, victim, &environment, budget()).unwrap_err(),
            RuntimeSpillError::UnsupportedValue,
            "mutation {mutation}"
        );
    }
}

/// An entry parameter live across a loop: its store opens the entry block
/// and the back edge targets the loop header — never the boundary itself —
/// so the single-entry interval stays honest while every loop use reloads.
fn entry_cycle_fixture(target: NativeTarget) -> ValidatedRuntimeSpill {
    let mut source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let jump = environment.constraint(keys.jump).unwrap();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let scalar_type = function.virtual_registers[0].scalar_type;
    let class = function.virtual_registers[0].class;
    let mut terminal = function.blocks[0].terminator.clone();
    let SelectedTerminator::Return { instruction, .. } = &mut terminal else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(1000);
    function.virtual_registers.truncate(1);
    for (register, instruction) in [(5u32, 201u32), (6, 301)] {
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(register),
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(instruction),
                source_value: ValueId::new(2).unwrap(),
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        });
    }
    let copy_at = |instruction, input, output| {
        admission::instruction(
            SelectedInstructionId(instruction),
            SelectedInstructionKind::CopyI64,
            copy,
            &[VirtualRegisterId(input), VirtualRegisterId(output)],
        )
    };
    let jump_at = |instruction| {
        admission::instruction(
            SelectedInstructionId(instruction),
            SelectedInstructionKind::Jump,
            jump,
            &[],
        )
    };
    let semantic = |destination| {
        let mut edge = successor(destination);
        edge.role = SelectedSuccessorRole::Semantic;
        edge
    };
    function.blocks = vec![
        SelectedBlock {
            id: SelectedBlockId(0),
            origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: jump_at(100),
                successor: semantic(1),
            },
        },
        SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![copy_at(201, 0, 5)],
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: jump_at(200),
                when_nonzero: semantic(2),
                when_zero: semantic(3),
            },
        },
        SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: vec![copy_at(301, 0, 6)],
            terminator: SelectedTerminator::Jump {
                instruction: jump_at(300),
                successor: semantic(1),
            },
        },
        SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: Vec::new(),
            terminator: terminal,
        },
    ];
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// The loop keeps the boundary store single-entry: the back edge targets the
/// header, so the entry block's leading store executes once and both loop
/// uses reload from it. Removing the store or retargeting the back edge at
/// the boundary itself both reject.
#[test]
fn entry_parameter_loop_uses_reload_from_one_boundary_store() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = entry_cycle_fixture(target);
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(0), &environment, budget())
                .unwrap();
        let transformed = &result.transformed().functions[0];
        // The entry block holds only the boundary store before its jump.
        let entry = &transformed.blocks[0];
        assert_eq!(entry.instructions.len(), 1);
        assert!(matches!(
            entry.instructions[0].kind,
            SelectedInstructionKind::Store64 { .. }
        ));
        assert_eq!(
            entry.instructions[0].operands[0].virtual_register,
            VirtualRegisterId(0)
        );
        // Each loop block reads one reload pair ahead of its consumer, and no
        // survivor names the victim register.
        for block in &transformed.blocks[1..3] {
            assert_eq!(block.instructions.len(), 3);
            assert!(matches!(
                block.instructions[0].kind,
                SelectedInstructionKind::FrameAddress { .. }
            ));
            assert!(matches!(
                block.instructions[1].kind,
                SelectedInstructionKind::Load64 { .. }
            ));
            assert_eq!(
                block.instructions[2].operands[0].virtual_register,
                block.instructions[1].operands[1].virtual_register
            );
        }
        for (source_block, block) in source.transformed().functions[0]
            .blocks
            .iter()
            .zip(&transformed.blocks)
        {
            assert_eq!(block.terminator, source_block.terminator);
        }
        validate_runtime_spill(
            &source,
            0,
            VirtualRegisterId(0),
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // Dropping the boundary store leaves the entry stream a prefix of the
        // source block — replay requires the leading store it emitted.
        let mut dropped = result.transformed().clone();
        dropped.functions[0].blocks[0].instructions.clear();
        assert_eq!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(0),
                &environment,
                budget(),
                dropped
            )
            .unwrap_err(),
            RuntimeSpillError::ReplayMismatch
        );
    }
    // The same loop shape with its back edge retargeted at the boundary is
    // exactly the re-entry admission refuses.
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let mut reentry = entry_cycle_fixture(NativeTarget::linux_x64());
    {
        let function = &mut Arc::make_mut(&mut reentry.transformed).functions[0];
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        function.blocks[2].terminator = SelectedTerminator::Jump {
            instruction: admission::instruction(
                SelectedInstructionId(300),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            successor: successor(0),
        };
    }
    assert_eq!(
        spill_selected_runtime_value(&reentry, 0, VirtualRegisterId(0), &environment, budget())
            .unwrap_err(),
        RuntimeSpillError::UnsupportedControlFlow
    );
}

/// A boundary store can share a declared slot whose incumbent window sits
/// entirely after the victim's own loads: the entry-positioned store is the
/// block's first writer event, so an incumbent store/load pair later in the
/// block still reads its own last writer while the victim's reload reads the
/// boundary store. Reuse declares nothing — the slot list stays singular.
#[test]
fn entry_parameter_reuses_an_incumbent_slot_written_after_its_loads() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let store = environment.constraint(keys.store64.unwrap()).unwrap();
    let address = environment.constraint(keys.frame_address.unwrap()).unwrap();
    let load = environment.constraint(keys.load64.unwrap()).unwrap();
    let incumbent = LocalStorageSlotId::Spill {
        register: VirtualRegisterId(9),
    };
    let mut source = fixture(target);
    {
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let scalar_type = function.virtual_registers[0].scalar_type;
        let class = function.virtual_registers[0].class;
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: incumbent,
            byte_size: 8,
            alignment: 8,
        });
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(10),
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::SpillAddress {
                instruction: SelectedInstructionId(91),
                register: VirtualRegisterId(9),
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(11),
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(92),
                source_value: ValueId::new(9).unwrap(),
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        });
        let frame_slot = FrameStorageSlotId::Local(incumbent);
        let instructions = &mut function.blocks[0].instructions;
        instructions.insert(
            1,
            admission::instruction(
                SelectedInstructionId(90),
                SelectedInstructionKind::Store64 {
                    slot: frame_slot,
                    byte_offset: 0,
                },
                store,
                &[VirtualRegisterId(1)],
            ),
        );
        instructions.insert(
            2,
            admission::instruction(
                SelectedInstructionId(91),
                SelectedInstructionKind::FrameAddress {
                    slot: frame_slot,
                    byte_offset: 0,
                },
                address,
                &[VirtualRegisterId(10)],
            ),
        );
        instructions.insert(
            3,
            admission::instruction(
                SelectedInstructionId(92),
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                load,
                &[VirtualRegisterId(10), VirtualRegisterId(11)],
            ),
        );
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(0), &environment, budget())
            .unwrap();
    let function = &result.transformed().functions[0];
    // Reuse declares nothing: the incumbent's entry stays the only slot, so
    // the frame's byte demand is unchanged by the second victim.
    assert_eq!(
        function.local_storage_slots.as_slice(),
        [SelectedLocalStorageSlot {
            id: incumbent,
            byte_size: 8,
            alignment: 8,
        }]
    );
    // The boundary store leads the block and names the shared slot.
    let block = &function.blocks[0];
    assert_eq!(
        block.instructions[0].kind,
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(incumbent),
            byte_offset: 0,
        }
    );
    assert_eq!(
        block.instructions[0].operands[0].virtual_register,
        VirtualRegisterId(0)
    );
    validate_runtime_spill(
        &source,
        0,
        VirtualRegisterId(0),
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A structural parameter's pointer register arrives live-in pinned to its
/// ABI view — the same boundary shape a scalar entry parameter keeps. Its
/// provenance is the structural contract's own parameter row rather than a
/// source site, and it carries no `ValueId`.
fn structural_entry_fixture(target: NativeTarget) -> ValidatedRuntimeSpill {
    let mut source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let place = PlaceId::new(1).unwrap();
    let structural_type = semantic_vocabulary::StructuralTypeId::new(1).unwrap();
    {
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let view = environment
            .physical()
            .model()
            .classes
            .iter()
            .find(|row| row.id == function.virtual_registers[0].class)
            .and_then(|row| row.views.first())
            .copied()
            .expect("the scalar class always declares a view");
        let register = &mut function.virtual_registers[0];
        register.origin = VirtualRegisterOrigin::StructuralParameter {
            place,
            parameter_index: 0,
        };
        register.definition_site = None;
        register.entry_fixed_view = Some(view);
        function.structural = Some(legalized_operations::LegalizedStructuralContract {
            result: None,
            structural_types: Vec::new().into(),
            parameters: vec![legalized_operations::LegalizedCallUnitParameter {
                semantic: terminal_psi::StructuralParameterDeclaration {
                    place,
                    position: 0,
                    is_self: false,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                target: target_operations::TargetStructuralParameter {
                    place,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    projected_qualifications: Vec::new(),
                    shape: calling_conventions::ValueShape::borrowed_reference(8, 8),
                    placement: calling_conventions::ValuePlacement {
                        shape: calling_conventions::ValueShape::borrowed_reference(8, 8),
                        locations: Vec::new(),
                    },
                },
            }],
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// The hidden aggregate-result destination is the one `AbiTransport` that is
/// a boundary live-in: instruction zero, offset zero, pinned view, no site,
/// naming the declared result place.
fn hidden_result_fixture(target: NativeTarget) -> ValidatedRuntimeSpill {
    let mut source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let place = PlaceId::new(2).unwrap();
    let structural_type = semantic_vocabulary::StructuralTypeId::new(1).unwrap();
    {
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let view = environment
            .physical()
            .model()
            .classes
            .iter()
            .find(|row| row.id == function.virtual_registers[0].class)
            .and_then(|row| row.views.first())
            .copied()
            .expect("the scalar class always declares a view");
        let register = &mut function.virtual_registers[0];
        register.origin = VirtualRegisterOrigin::AbiTransport {
            instruction: SelectedInstructionId(0),
            place,
            byte_offset: 0,
        };
        register.definition_site = None;
        register.entry_fixed_view = Some(view);
        function.structural = Some(legalized_operations::LegalizedStructuralContract {
            result: Some(terminal_psi::StructuralResultDeclaration {
                place,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                reference_sources: Vec::new(),
            }),
            structural_types: Vec::new().into(),
            parameters: Vec::new(),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// A structural parameter's boundary definition stores at the head of the
/// entry block exactly like a scalar entry parameter's, and its reload
/// registers re-observe the declared place rather than restating a source
/// value. Replay independently demands the same leading store.
#[test]
fn structural_parameter_stores_at_the_entry_boundary_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = structural_entry_fixture(target);
        let place = PlaceId::new(1).unwrap();
        let pinned = source.transformed().functions[0].virtual_registers[0].entry_fixed_view;
        let original = source.transformed().functions[0].blocks[0]
            .instructions
            .len();
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(0), &environment, budget())
                .unwrap();
        let transformed = &result.transformed().functions[0];
        let block = &transformed.blocks[0];
        // [store, address, load, copy×4]: the boundary store leads, then the
        // sole use's reload pair precedes its consumer.
        assert_eq!(block.instructions.len(), original + 3);
        assert!(matches!(
            block.instructions[0].kind,
            SelectedInstructionKind::Store64 { .. }
        ));
        assert_eq!(
            block.instructions[0].operands[0].virtual_register,
            VirtualRegisterId(0)
        );
        assert!(matches!(
            block.instructions[1].kind,
            SelectedInstructionKind::FrameAddress { .. }
        ));
        assert!(matches!(
            block.instructions[2].kind,
            SelectedInstructionKind::Load64 { .. }
        ));
        let reload = block.instructions[2].operands[1].virtual_register;
        assert_eq!(block.instructions[3].operands[0].virtual_register, reload);
        // The reload re-observes the parameter's place; a structural live-in
        // restates no source value.
        let reload_register = transformed
            .virtual_registers
            .iter()
            .find(|register| register.id == reload)
            .unwrap();
        assert_eq!(
            reload_register.origin,
            VirtualRegisterOrigin::StructuralObservation {
                instruction: block.instructions[2].id,
                place,
                byte_offset: 0,
            }
        );
        // Nothing past the boundary store still names the victim register.
        assert!(block.instructions.iter().skip(1).all(|instruction| {
            instruction
                .operands
                .iter()
                .all(|operand| operand.virtual_register != VirtualRegisterId(0))
        }));
        assert_eq!(
            transformed.local_storage_slots.as_slice(),
            [SelectedLocalStorageSlot {
                id: LocalStorageSlotId::Spill {
                    register: VirtualRegisterId(0)
                },
                byte_size: 8,
                alignment: 8,
            }]
        );
        assert_eq!(transformed.virtual_registers[0].entry_fixed_view, pinned);
        validate_runtime_spill(
            &source,
            0,
            VirtualRegisterId(0),
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // Replay demands the boundary store lead the block: pushing it into
        // the body leaves the first stream instruction a source copy.
        let mut moved = result.transformed().clone();
        moved.functions[0].blocks[0].instructions.swap(0, 3);
        assert_eq!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(0),
                &environment,
                budget(),
                moved
            )
            .unwrap_err(),
            RuntimeSpillError::ReplayMismatch
        );
    }
}

/// The hidden aggregate-result destination spills through the same boundary
/// store: the ABI hands the result pointer in pinned, no instruction defines
/// it, and its reloads observe the declared result place.
#[test]
fn hidden_result_destination_stores_at_the_entry_boundary_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = hidden_result_fixture(target);
        let place = PlaceId::new(2).unwrap();
        let original = source.transformed().functions[0].blocks[0]
            .instructions
            .len();
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(0), &environment, budget())
                .unwrap();
        let transformed = &result.transformed().functions[0];
        let block = &transformed.blocks[0];
        assert_eq!(block.instructions.len(), original + 3);
        assert!(matches!(
            block.instructions[0].kind,
            SelectedInstructionKind::Store64 { .. }
        ));
        let reload = block.instructions[2].operands[1].virtual_register;
        assert_eq!(block.instructions[3].operands[0].virtual_register, reload);
        let reload_register = transformed
            .virtual_registers
            .iter()
            .find(|register| register.id == reload)
            .unwrap();
        assert_eq!(
            reload_register.origin,
            VirtualRegisterOrigin::StructuralObservation {
                instruction: block.instructions[2].id,
                place,
                byte_offset: 0,
            }
        );
        validate_runtime_spill(
            &source,
            0,
            VirtualRegisterId(0),
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// Structural live-ins keep their own provenance discipline: a place the
/// contract's parameter row does not name, a carried source site, a missing
/// contract, and a transport that is not the declared result destination all
/// stay rejected — the boundary definition admits only what the contract
/// itself proves.
#[test]
fn structural_live_in_requires_its_contract_provenance() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..7 {
        let mut source = structural_entry_fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            match mutation {
                // The contract's parameter row must name the origin's place.
                0 => {
                    function.structural.as_mut().unwrap().parameters[0]
                        .semantic
                        .place = PlaceId::new(7).unwrap();
                }
                // The index must select this place's row — an out-of-range
                // index names no live-in.
                1 => {
                    function.virtual_registers[0].origin =
                        VirtualRegisterOrigin::StructuralParameter {
                            place: PlaceId::new(1).unwrap(),
                            parameter_index: 1,
                        };
                }
                // A structural live-in carries no source definition site.
                2 => {
                    function.virtual_registers[0].definition_site =
                        Some(ValueDefinitionSite::FunctionParameter(0));
                }
                // Without the contract there is no provenance to check.
                3 => {
                    function.structural = None;
                }
                // An instruction-made transport address is not the boundary
                // live-in, whatever its place.
                4 => {
                    function.virtual_registers[0].origin = VirtualRegisterOrigin::AbiTransport {
                        instruction: SelectedInstructionId(7),
                        place: PlaceId::new(1).unwrap(),
                        byte_offset: 0,
                    };
                }
                // A nonzero offset observes a fragment, not the result
                // destination itself.
                5 => {
                    function.virtual_registers[0].origin = VirtualRegisterOrigin::AbiTransport {
                        instruction: SelectedInstructionId(0),
                        place: PlaceId::new(1).unwrap(),
                        byte_offset: 8,
                    };
                }
                // The boundary transport must keep its pinned ABI live-in
                // view; without it the origin is no entry register.
                _ => {
                    function.virtual_registers[0].origin = VirtualRegisterOrigin::AbiTransport {
                        instruction: SelectedInstructionId(0),
                        place: PlaceId::new(1).unwrap(),
                        byte_offset: 0,
                    };
                    function.virtual_registers[0].entry_fixed_view = None;
                }
            }
            let identity = selected_instruction_plan_identity(source.transformed());
            source.receipt.source_selected = identity;
            source.receipt.transformed_selected = identity;
        }
        assert_eq!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(0), &environment, budget())
                .unwrap_err(),
            RuntimeSpillError::UnsupportedValue,
            "mutation {mutation}"
        );
    }
    // The hidden destination must also name the contract's declared result
    // place — another place's pointer is an ordinary transport address.
    let mut other_place = hidden_result_fixture(target);
    {
        let function = &mut Arc::make_mut(&mut other_place.transformed).functions[0];
        function.virtual_registers[0].origin = VirtualRegisterOrigin::AbiTransport {
            instruction: SelectedInstructionId(0),
            place: PlaceId::new(9).unwrap(),
            byte_offset: 0,
        };
        let identity = selected_instruction_plan_identity(other_place.transformed());
        other_place.receipt.source_selected = identity;
        other_place.receipt.transformed_selected = identity;
    }
    assert_eq!(
        spill_selected_runtime_value(
            &other_place,
            0,
            VirtualRegisterId(0),
            &environment,
            budget()
        )
        .unwrap_err(),
        RuntimeSpillError::UnsupportedValue
    );
}

/// An instruction-defined field observation is a structural victim: its
/// definition is its own load, so the store lands right after it, and the
/// case edge's payload argument moves to a reload register carrying the same
/// place and field offset — the exact coordinate the transport demands.
/// Replay independently reconstructs the same observation origin.
#[test]
fn field_observation_payload_argument_spills_at_its_case_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = case_parameter_fixture(target);
        // Register 3 is the first bridge's `StructuralObservation`: the
        // `Load64` at instruction 202 defines it and the continuation's
        // payload argument is its only use.
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(3), &environment, budget())
                .unwrap();
        let transformed = &result.transformed().functions[0];
        // The case slot is the fixture's own declaration; the private spill
        // slot is appended after it.
        assert_eq!(
            transformed.local_storage_slots.as_slice()[1..],
            [SelectedLocalStorageSlot {
                id: LocalStorageSlotId::Spill {
                    register: VirtualRegisterId(3)
                },
                byte_size: 8,
                alignment: 8,
            }]
        );
        // [FrameAddress, Load64, Store64, FrameAddress, Load64]: the store
        // lands right after the observing load and the payload argument's
        // reload pair closes the block.
        let instructions = &transformed.blocks[1].instructions;
        assert_eq!(instructions.len(), 5);
        assert_eq!(instructions[0].id, SelectedInstructionId(201));
        assert_eq!(instructions[1].id, SelectedInstructionId(202));
        assert!(matches!(
            instructions[2].kind,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(LocalStorageSlotId::Spill { register }),
                byte_offset: 0,
            } if register == VirtualRegisterId(3)
        ));
        assert_eq!(
            instructions[2].operands[0].virtual_register,
            VirtualRegisterId(3)
        );
        assert!(matches!(
            instructions[3].kind,
            SelectedInstructionKind::FrameAddress { .. }
        ));
        assert!(matches!(
            instructions[4].kind,
            SelectedInstructionKind::Load64 { byte_offset: 0 }
        ));
        let reloaded = instructions[4].operands[1].virtual_register;
        // The payload argument now names the reload, whose observation origin
        // restates the victim's declared place at the payload's field offset.
        let reload_register = transformed
            .virtual_registers
            .iter()
            .find(|register| register.id == reloaded)
            .unwrap();
        assert_eq!(
            reload_register.origin,
            VirtualRegisterOrigin::StructuralObservation {
                instruction: instructions[4].id,
                place: PlaceId::new(1).unwrap(),
                byte_offset: 0,
            }
        );
        let SelectedTerminator::Jump { successor, .. } = &transformed.blocks[1].terminator else {
            unreachable!()
        };
        let case = successor.structural_case.as_ref().unwrap();
        assert_eq!(
            case.payloads[0].transport,
            SelectedCasePayloadTransport::Registers {
                argument: reloaded,
                parameter: VirtualRegisterId(1),
            }
        );
        // The other bridge's observation is untouched.
        let SelectedTerminator::Jump { successor, .. } = &transformed.blocks[3].terminator else {
            unreachable!()
        };
        assert_eq!(
            successor.structural_case.as_ref().unwrap().payloads[0].transport,
            SelectedCasePayloadTransport::Registers {
                argument: VirtualRegisterId(5),
                parameter: VirtualRegisterId(1),
            }
        );
        validate_runtime_spill(
            &source,
            0,
            VirtualRegisterId(3),
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A payload still naming the victim is not what replay builds.
        let mut proposed = result.transformed().clone();
        let SelectedTerminator::Jump { successor, .. } =
            &mut proposed.functions[0].blocks[1].terminator
        else {
            unreachable!()
        };
        successor.structural_case.as_mut().unwrap().payloads[0].transport =
            SelectedCasePayloadTransport::Registers {
                argument: VirtualRegisterId(3),
                parameter: VirtualRegisterId(1),
            };
        assert_eq!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(3),
                &environment,
                budget(),
                proposed
            )
            .unwrap_err(),
            RuntimeSpillError::ReplayMismatch
        );
    }
}

/// A structural victim serving a case-payload argument must restate the exact
/// coordinate the payload declares: another place or another field offset
/// would make the rewritten argument an observation the transport never
/// asked for, so the naming stays rejected. An address-producing definition
/// — the bridge's own `FrameAddress` pointer — stays rejected too: it is the
/// slot's coordinate, not a stored value.
#[test]
fn instruction_defined_structural_victims_keep_their_coordinates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..2 {
        let mut source = case_parameter_fixture(target);
        {
            let victim =
                &mut Arc::make_mut(&mut source.transformed).functions[0].virtual_registers[3];
            victim.origin = match mutation {
                // A different place's field is not this payload's argument.
                0 => VirtualRegisterOrigin::StructuralObservation {
                    instruction: SelectedInstructionId(202),
                    place: PlaceId::new(9).unwrap(),
                    byte_offset: 0,
                },
                // A nonzero offset observes a different field of the case's
                // place than the payload's declared `field_byte_offset`.
                _ => VirtualRegisterOrigin::StructuralObservation {
                    instruction: SelectedInstructionId(202),
                    place: PlaceId::new(1).unwrap(),
                    byte_offset: 4,
                },
            };
            let identity = selected_instruction_plan_identity(source.transformed());
            source.receipt.source_selected = identity;
            source.receipt.transformed_selected = identity;
        }
        assert_eq!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(3), &environment, budget())
                .unwrap_err(),
            RuntimeSpillError::UnsupportedUse,
            "mutation {mutation}"
        );
    }
    // The bridge pointer defined by `FrameAddress` names storage coordinates
    // — an `AbiTransport` provenance with no `ValueId` — and its resolved
    // address round-trips through private storage like any other result. Its
    // single use is the field `Load64`'s address operand, which no replay
    // joins to the forming instruction, so the operand follows the reload
    // while the load's own observation result keeps the payload's exact
    // coordinates.
    let source = case_parameter_fixture(target);
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(2), &environment, budget())
            .unwrap();
    let transformed = &result.transformed().functions[0];
    let bridge = transformed
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| instruction.id == SelectedInstructionId(201))
        })
        .unwrap();
    let load = bridge
        .instructions
        .iter()
        .find(|instruction| instruction.id == SelectedInstructionId(202))
        .unwrap();
    let reloaded = load.operands[0].virtual_register;
    assert_ne!(reloaded, VirtualRegisterId(2));
    assert!(matches!(
        transformed
            .virtual_registers
            .iter()
            .find(|register| register.id == reloaded)
            .unwrap()
            .origin,
        VirtualRegisterOrigin::StructuralObservation { place: restated, byte_offset: 0, .. }
            if restated == PlaceId::new(1).unwrap()
    ));
    assert!(
        validate_runtime_spill(
            &source,
            0,
            VirtualRegisterId(2),
            &environment,
            budget(),
            result.transformed().clone()
        )
        .is_ok()
    );
}

/// A structural live-in restates no source value, so a `Registers` binding
/// naming it can never satisfy the semantic link a value transport needs.
/// And like every boundary definition, re-entry into the entry block stays
/// rejected — the store would re-read a register whose interval ended.
#[test]
fn structural_live_in_rejects_value_binding_transport_and_reentry() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let jump = environment
        .constraint(environment.selected_keys().jump)
        .unwrap();
    let mut bound = structural_entry_fixture(target);
    {
        let scalar_type = bound.transformed().functions[0].virtual_registers[0].scalar_type;
        let function = &mut Arc::make_mut(&mut bound.transformed).functions[0];
        let mut edge = successor(1);
        edge.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(9).unwrap(),
                argument: ValueId::new(8).unwrap(),
                scalar_type,
            },
            transport: SelectedValueTransport::Registers {
                argument: VirtualRegisterId(0),
                parameter: VirtualRegisterId(9),
            },
        });
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: admission::instruction(
                SelectedInstructionId(50),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            when_nonzero: edge,
            when_zero: successor(1),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: admission::instruction(
                    SelectedInstructionId(51),
                    SelectedInstructionKind::ReturnUnit,
                    return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
        let identity = selected_instruction_plan_identity(bound.transformed());
        bound.receipt.source_selected = identity;
        bound.receipt.transformed_selected = identity;
    }
    assert_eq!(
        spill_selected_runtime_value(&bound, 0, VirtualRegisterId(0), &environment, budget())
            .unwrap_err(),
        RuntimeSpillError::UnsupportedUse
    );
    // Any edge back to the boundary re-executes the store against a register
    // whose interval already ended — the same rejection entry parameters keep.
    let mut reentry = structural_entry_fixture(target);
    {
        let function = &mut Arc::make_mut(&mut reentry.transformed).functions[0];
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: admission::instruction(
                SelectedInstructionId(50),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            when_nonzero: successor(0),
            when_zero: successor(0),
        };
        let identity = selected_instruction_plan_identity(reentry.transformed());
        reentry.receipt.source_selected = identity;
        reentry.receipt.transformed_selected = identity;
    }
    assert_eq!(
        spill_selected_runtime_value(&reentry, 0, VirtualRegisterId(0), &environment, budget())
            .unwrap_err(),
        RuntimeSpillError::UnsupportedControlFlow
    );
}
