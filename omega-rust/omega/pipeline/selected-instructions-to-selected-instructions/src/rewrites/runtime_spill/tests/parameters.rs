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
