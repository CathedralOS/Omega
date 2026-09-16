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
    SelectedBlockOrigin, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
    SelectedSuccessor, SelectedSuccessorRole, SelectedValueBinding, SelectedValueTransport,
};
use semantic_vocabulary::{BoundaryMachineId, OperationId};

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
