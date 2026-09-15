use super::{
    Arc, BlockId, EdgeId, NativeTarget, ScalarType, SelectedBlock, SelectedBlockId,
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
    LocalStorageSlotId, SelectedBlockOrigin, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedStructuralBinding, SelectedStructuralTransport,
    SelectedSuccessor, SelectedSuccessorRole, SelectedValueBinding, SelectedValueTransport,
};
use semantic_vocabulary::{BoundaryMachineId, OperationId, PlaceId};

pub(super) fn successor(destination: u32) -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::EdgeTransferContinuation,
        psi_edge: EdgeId::new(2).unwrap(),
        block: SelectedBlockId(destination),
        source_target: BlockId::new(3).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        fuel: Vec::new(),
    }
}

pub(super) fn cfg_fixture(target: NativeTarget) -> ValidatedRuntimeSpill {
    let mut source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let jump_row = environment
        .constraint(environment.selected_keys().jump)
        .unwrap();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let mut body = function.blocks.remove(0);
    body.id = SelectedBlockId(1);
    body.origin = SelectedBlockOrigin::EdgeTransfer {
        edge: EdgeId::new(2).unwrap(),
        target: BlockId::new(3).unwrap(),
    };
    let mut exit = SelectedBlock {
        id: SelectedBlockId(2),
        origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
        instructions: Vec::new(),
        terminator: body.terminator.clone(),
    };
    let SelectedTerminator::Return { instruction, .. } = &mut exit.terminator else {
        unreachable!();
    };
    instruction.id = SelectedInstructionId(1000);
    body.terminator = SelectedTerminator::Jump {
        instruction: admission::instruction(
            SelectedInstructionId(101),
            SelectedInstructionKind::Jump,
            jump_row,
            &[],
        ),
        successor: successor(2),
    };
    function.blocks = vec![
        SelectedBlock {
            id: SelectedBlockId(0),
            origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: admission::instruction(
                    SelectedInstructionId(100),
                    SelectedInstructionKind::Jump,
                    jump_row,
                    &[],
                ),
                successor: successor(1),
            },
        },
        body,
        exit,
    ];
    function.virtual_registers[1].definition_site = Some(ValueDefinitionSite::BlockParameter {
        block: BlockId::new(3).unwrap(),
        position: 0,
    });
    for (block, instruction_index) in [(0, 0), (1, 0), (1, 1), (1, 4), (2, 0)] {
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

#[test]
fn instruction_defined_edge_snapshots_spill_in_any_block_order_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        for reordered in [false, true] {
            let mut source = cfg_fixture(target);
            if reordered {
                Arc::make_mut(&mut source.transformed).functions[0]
                    .blocks
                    .swap(0, 2);
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
            for (before, after) in original.blocks.iter().zip(&transformed.blocks) {
                assert_eq!(before.terminator, after.terminator);
                if before.id != SelectedBlockId(1) {
                    assert_eq!(before, after);
                } else {
                    assert_eq!(after.instructions.len(), 11);
                    assert_eq!(after.instructions[1].id, SelectedInstructionId(1001));
                }
            }
            assert_eq!(
                transformed
                    .boundary_settlements
                    .iter()
                    .map(|settlement| settlement.instruction_index)
                    .collect::<Vec<_>>(),
                [0, 0, 4, 11, 0]
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
                assert_eq!(
                    reload.definition_site,
                    original.virtual_registers[1].definition_site
                );
                assert_eq!(
                    reload.scalar_type,
                    original.virtual_registers[1].scalar_type
                );
            }
        }
    }
}

#[test]
fn undominated_lifetimes_and_uninitialized_parameters_do_not_gain_spill_authority() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for mutation in 0..7 {
        let mut source = cfg_fixture(NativeTarget::linux_x64());
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        match mutation {
            0 => {
                let instruction = function.blocks[1].instructions.pop().unwrap();
                function.blocks[0].instructions.push(instruction);
            }
            1 => function.blocks[1].instructions.swap(0, 1),
            2 => {
                let mut duplicate = function.blocks[1].instructions[0].clone();
                duplicate.id = SelectedInstructionId(500);
                function.blocks[1].instructions.push(duplicate);
            }
            3 => {
                function.virtual_registers[1].origin = VirtualRegisterOrigin::BlockParameter {
                    source_value: ValueId::new(1).unwrap(),
                    block: SelectedBlockId(1),
                    parameter_index: 0,
                };
            }
            4 => {
                // A terminator operand use is admissible only where the
                // definition block dominates it; the entry block is not
                // dominated by the body block, so this stays rejected.
                let operand = function.blocks[1].instructions[1].operands[0];
                super::super::control_mut(&mut function.blocks[0].terminator)
                    .operands
                    .push(operand);
            }
            5 => {
                let definition = function.blocks[1].instructions.remove(0);
                function.blocks[2].instructions.push(definition);
            }
            6 => {
                function.blocks[1].instructions[1].operands[1].tied_to = Some(0);
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
fn registers_binding_arguments_admit_while_other_transports_reject() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for terminator_kind in 0..6 {
        for reference_kind in 0..5 {
            let mut source = cfg_fixture(NativeTarget::linux_x64());
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            let mut instruction = super::super::control(&function.blocks[1].terminator)
                .0
                .clone();
            let mut edge = successor(2);
            match reference_kind {
                0 => instruction
                    .operands
                    .push(function.blocks[1].instructions[1].operands[0]),
                1 | 2 => edge.bindings.push(SelectedValueBinding {
                    semantic: abstract_operations::ValueBinding {
                        parameter: ValueId::new(2).unwrap(),
                        argument: ValueId::new(1).unwrap(),
                        scalar_type: function.virtual_registers[1].scalar_type,
                    },
                    transport: SelectedValueTransport::Registers {
                        argument: VirtualRegisterId(if reference_kind == 1 { 1 } else { 4 }),
                        parameter: VirtualRegisterId(if reference_kind == 2 { 1 } else { 4 }),
                    },
                }),
                3 | 4 => edge.structural_bindings.push(SelectedStructuralBinding {
                    semantic: abstract_operations::AbstractStructuralBinding {
                        parameter: PlaceId::new(1).unwrap(),
                        argument: terminal_psi::StructuralArgument {
                            place: PlaceId::new(2).unwrap(),
                            path: Vec::new(),
                            access: terminal_psi::StructuralAccess::SharedBorrow,
                        },
                    },
                    transport: if reference_kind == 3 {
                        SelectedStructuralTransport::Descriptor {
                            argument: VirtualRegisterId(1),
                            destination: LocalStorageSlotId::Boundary {
                                operation: OperationId::new(1).unwrap(),
                            },
                        }
                    } else {
                        SelectedStructuralTransport::WholeValue {
                            argument: VirtualRegisterId(1),
                            destination: LocalStorageSlotId::Boundary {
                                operation: OperationId::new(1).unwrap(),
                            },
                            byte_size: 8,
                            alignment: 8,
                        }
                    },
                }),
                _ => unreachable!(),
            }
            // Both branch polarities must inspect their own metadata.
            for referenced_first in [false, true] {
                let (first, second) = if referenced_first {
                    (edge.clone(), successor(2))
                } else {
                    (successor(2), edge.clone())
                };
                Arc::make_mut(&mut source.transformed).functions[0].blocks[1].terminator =
                    match terminator_kind {
                        0 => SelectedTerminator::Jump {
                            instruction: instruction.clone(),
                            successor: edge.clone(),
                        },
                        1 => SelectedTerminator::ConditionalBranch {
                            instruction: instruction.clone(),
                            when_nonzero: first,
                            when_zero: second,
                        },
                        2 => SelectedTerminator::ConditionalBranchU64LessThan {
                            instruction: instruction.clone(),
                            when_less: first,
                            when_not_less: second,
                        },
                        3 => SelectedTerminator::ConditionalBranchI64LessThan {
                            instruction: instruction.clone(),
                            when_less: first,
                            when_not_less: second,
                        },
                        4 => SelectedTerminator::Return {
                            instruction: instruction.clone(),
                            psi_return_edge: EdgeId::new(3).unwrap(),
                        },
                        5 => SelectedTerminator::HostedExitProcess {
                            instruction: instruction.clone(),
                            nominal_return_edge: EdgeId::new(3).unwrap(),
                        },
                        _ => unreachable!(),
                    };
                // Return/exit have no successor metadata to reference.
                if terminator_kind >= 4 && reference_kind != 0 {
                    continue;
                }
                let admitted =
                    admission::admit(&source, 0, VirtualRegisterId(1), &environment, budget());
                if reference_kind <= 1 {
                    // Terminator operands and outgoing binding arguments are
                    // both ordinary end-of-block uses.
                    assert!(
                        admitted.is_ok(),
                        "terminator {terminator_kind} reference {reference_kind}"
                    );
                } else {
                    // The parameter side of a transport on a foreign edge and
                    // every structural-transport argument stay outside
                    // admission.
                    assert_eq!(
                        admitted.err(),
                        Some(RuntimeSpillError::UnsupportedUse),
                        "terminator {terminator_kind} transport {reference_kind}"
                    );
                }
            }
        }
    }
}

#[test]
fn terminator_operand_uses_reload_at_block_end_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        for pinned in [false, true] {
            let (kind, key) = if pinned {
                (SelectedInstructionKind::ReturnScalar, keys.return_i64)
            } else {
                // The flexible exit-code use exists only on hosted targets.
                let Some(key) = keys.hosted_exit_process_i32 else {
                    continue;
                };
                (SelectedInstructionKind::HostedExitProcessI32, key)
            };
            let mut source = cfg_fixture(target);
            {
                let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
                let instruction = admission::instruction(
                    SelectedInstructionId(2000),
                    kind,
                    environment.constraint(key).unwrap(),
                    &[VirtualRegisterId(1)],
                );
                function.blocks[2].terminator = if pinned {
                    SelectedTerminator::Return {
                        instruction,
                        psi_return_edge: EdgeId::new(3).unwrap(),
                    }
                } else {
                    SelectedTerminator::HostedExitProcess {
                        instruction,
                        nominal_return_edge: EdgeId::new(3).unwrap(),
                    }
                };
            }
            let identity = selected_instruction_plan_identity(source.transformed());
            source.receipt.source_selected = identity;
            source.receipt.transformed_selected = identity;
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
            // The exit block gains exactly one address/load pair, appended
            // after its last instruction; the terminator operand keeps its
            // access and fixed view while moving to the fresh reload register.
            let block = &transformed.blocks[2];
            let tail = block.instructions.len() - 2;
            assert_eq!(
                block.instructions.len(),
                original.blocks[2].instructions.len() + 2
            );
            assert!(matches!(
                block.instructions[tail].kind,
                SelectedInstructionKind::FrameAddress { .. }
            ));
            assert!(matches!(
                block.instructions[tail + 1].kind,
                SelectedInstructionKind::Load64 { .. }
            ));
            let original_operand = super::super::control(&original.blocks[2].terminator)
                .0
                .operands[0];
            let rewritten_operand = super::super::control(&block.terminator).0.operands[0];
            let reload_register = block.instructions[tail + 1].operands[1].virtual_register;
            assert_eq!(original_operand.virtual_register, VirtualRegisterId(1));
            assert_eq!(rewritten_operand.virtual_register, reload_register);
            assert_eq!(rewritten_operand.access, original_operand.access);
            assert_eq!(rewritten_operand.fixed_view, original_operand.fixed_view);
            assert_eq!(pinned, original_operand.fixed_view.is_some());
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
                    0 => {
                        function.blocks[2].instructions.remove(tail + 1);
                    }
                    1 => {
                        function.blocks[2].instructions.swap(tail, tail + 1);
                    }
                    2 => {
                        super::super::control_mut(&mut function.blocks[2].terminator).operands[0]
                            .virtual_register = VirtualRegisterId(1);
                    }
                    3 => {
                        super::super::control_mut(&mut function.blocks[2].terminator).operands[0]
                            .access = register_model::RegisterOperandAccess::Def;
                    }
                    4 => {
                        function.virtual_registers.pop();
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
                    "{target:?} pinned={pinned} mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn edge_binding_arguments_reload_at_predecessor_end_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let mut source = cfg_fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            let scalar_type = function.virtual_registers[1].scalar_type;
            // The destination's own parameter register receives the transport.
            function.virtual_registers.push(VirtualRegister {
                id: VirtualRegisterId(5),
                scalar_type,
                class: function.virtual_registers[1].class,
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
            let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[1].terminator
            else {
                unreachable!()
            };
            successor.bindings.push(SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: ValueId::new(2).unwrap(),
                    argument: ValueId::new(1).unwrap(),
                    scalar_type,
                },
                transport: SelectedValueTransport::Registers {
                    argument: VirtualRegisterId(1),
                    parameter: VirtualRegisterId(5),
                },
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
        let block = &transformed.blocks[1];
        // Three body uses plus the edge-argument use each get their own pair;
        // the transport pair lands after the last body instruction.
        assert_eq!(
            block.instructions.len(),
            original.blocks[1].instructions.len() + 1 + 8
        );
        let tail = block.instructions.len() - 2;
        assert!(matches!(
            block.instructions[tail].kind,
            SelectedInstructionKind::FrameAddress { .. }
        ));
        assert!(matches!(
            block.instructions[tail + 1].kind,
            SelectedInstructionKind::Load64 { .. }
        ));
        let reload_register = block.instructions[tail + 1].operands[1].virtual_register;
        let SelectedTerminator::Jump { successor, .. } = &block.terminator else {
            unreachable!()
        };
        let binding = &successor.bindings[0];
        assert_eq!(
            binding.transport,
            SelectedValueTransport::Registers {
                argument: reload_register,
                parameter: VirtualRegisterId(5),
            }
        );
        assert_eq!(
            binding.semantic,
            super::super::control(&original.blocks[1].terminator).1[0]
                .unwrap()
                .bindings[0]
                .semantic
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
        for mutation in 0..6 {
            let mut proposed = result.transformed().clone();
            let function = &mut proposed.functions[0];
            match mutation {
                0 => {
                    function.blocks[1].instructions.remove(tail + 1);
                }
                1 => {
                    function.blocks[1].instructions.swap(tail, tail + 1);
                }
                2 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[1].terminator
                    else {
                        unreachable!()
                    };
                    let SelectedValueTransport::Registers { argument, .. } =
                        &mut successor.bindings[0].transport
                    else {
                        unreachable!()
                    };
                    *argument = VirtualRegisterId(1);
                }
                3 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[1].terminator
                    else {
                        unreachable!()
                    };
                    successor.bindings[0].semantic.argument = ValueId::new(2).unwrap();
                }
                4 => {
                    function.virtual_registers.pop();
                }
                5 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[1].terminator
                    else {
                        unreachable!()
                    };
                    successor.bindings.clear();
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
fn branch_bindings_reload_per_edge_after_terminator_operands() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = cfg_fixture(target);
    {
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let scalar_type = function.virtual_registers[1].scalar_type;
        let class = function.virtual_registers[1].class;
        // Both destination parameter registers receive the same victim.
        for (id, block) in [(5, SelectedBlockId(2)), (6, SelectedBlockId(0))] {
            function.virtual_registers.push(VirtualRegister {
                id: VirtualRegisterId(id),
                scalar_type,
                class,
                origin: VirtualRegisterOrigin::BlockParameter {
                    source_value: ValueId::new(2).unwrap(),
                    block,
                    parameter_index: 0,
                },
                definition_site: Some(ValueDefinitionSite::BlockParameter {
                    block: BlockId::new(3).unwrap(),
                    position: 0,
                }),
                entry_fixed_view: None,
            });
        }
        let instruction = super::super::control(&function.blocks[1].terminator)
            .0
            .clone();
        let mut bound = successor(2);
        bound.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(2).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type,
            },
            transport: SelectedValueTransport::Registers {
                argument: VirtualRegisterId(1),
                parameter: VirtualRegisterId(5),
            },
        });
        let mut second = successor(0);
        second.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(2).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type,
            },
            transport: SelectedValueTransport::Registers {
                argument: VirtualRegisterId(1),
                parameter: VirtualRegisterId(6),
            },
        });
        function.blocks[1].terminator = SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero: bound,
            when_zero: second,
        };
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    let block = &result.transformed().functions[0].blocks[1];
    // One edge use per successor: two pairs after the definition store and
    // the three body-use pairs, in successor order.
    assert_eq!(block.instructions.len(), 4 + 1 + 6 + 4);
    let SelectedTerminator::ConditionalBranch {
        when_nonzero,
        when_zero,
        ..
    } = &block.terminator
    else {
        unreachable!()
    };
    for (successor, tail) in [(when_nonzero, 12), (when_zero, 14)] {
        let SelectedValueTransport::Registers { argument, .. } = successor.bindings[0].transport
        else {
            unreachable!()
        };
        assert!(matches!(
            block.instructions[tail - 1].kind,
            SelectedInstructionKind::FrameAddress { .. }
        ));
        assert!(matches!(
            block.instructions[tail].kind,
            SelectedInstructionKind::Load64 { .. }
        ));
        assert_eq!(
            argument,
            block.instructions[tail].operands[1].virtual_register
        );
        assert_ne!(argument, VirtualRegisterId(1));
    }
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

#[test]
fn undominated_and_misdeclared_binding_arguments_do_not_gain_spill_authority() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for mutation in 0..3 {
        let mut source = cfg_fixture(NativeTarget::linux_x64());
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let scalar_type = function.virtual_registers[1].scalar_type;
        let binding = |argument, semantic_argument| SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(2).unwrap(),
                argument: semantic_argument,
                scalar_type,
            },
            transport: SelectedValueTransport::Registers {
                argument,
                parameter: VirtualRegisterId(5),
            },
        };
        match mutation {
            // The entry block's edge is not dominated by the victim's
            // definition block, so the transport cannot read initialized
            // storage.
            0 => {
                let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator
                else {
                    unreachable!()
                };
                successor
                    .bindings
                    .push(binding(VirtualRegisterId(1), ValueId::new(1).unwrap()));
            }
            // A declaration naming a different source value is an
            // inconsistent plan, not a use of the victim.
            1 => {
                let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[1].terminator
                else {
                    unreachable!()
                };
                successor
                    .bindings
                    .push(binding(VirtualRegisterId(1), ValueId::new(99).unwrap()));
            }
            // A declaration naming a different scalar type is inconsistent
            // the same way.
            2 => {
                let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[1].terminator
                else {
                    unreachable!()
                };
                let mut binding = binding(VirtualRegisterId(1), ValueId::new(1).unwrap());
                binding.semantic.scalar_type = ScalarType::Boolean;
                successor.bindings.push(binding);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap_err(),
            RuntimeSpillError::UnsupportedUse,
            "mutation {mutation}"
        );
    }
}

#[test]
fn cyclic_functions_admit_dominating_instruction_results() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for destination in [0, 1, 2] {
        let mut source = cfg_fixture(NativeTarget::linux_x64());
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let instruction = super::super::control(&function.blocks[0].terminator)
            .0
            .clone();
        function.blocks[2].terminator = SelectedTerminator::Jump {
            instruction,
            successor: successor(destination),
        };
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
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

/// A loop-carried parameter: the destination's terminator gains a back edge
/// through a dedicated edge-transfer block that rebinds the parameter from a
/// loop-body value (or, when `passthrough`, from the parameter itself). The
/// destination still dominates every use and every arrival stores its bound
/// argument before the destination executes.
fn cyclic_parameter_fixture(target: NativeTarget, passthrough: bool) -> ValidatedRuntimeSpill {
    let mut source = super::parameters::parameter_fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let jump = environment.constraint(keys.jump).unwrap();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let scalar_type = function.virtual_registers[1].scalar_type;
    let class = function.virtual_registers[1].class;
    let terminal = function.blocks[2].terminator.clone();
    // Register 7 is the back-edge arrival's bound argument: one CopyI64 in the
    // edge-transfer block produces it, exactly like the entry-side arrivals.
    function.virtual_registers.push(VirtualRegister {
        id: VirtualRegisterId(7),
        scalar_type,
        class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(501),
            source_value: ValueId::new(2).unwrap(),
        },
        definition_site: function.virtual_registers[5].definition_site,
        entry_fixed_view: None,
    });
    let mut back = successor(2);
    back.bindings.push(SelectedValueBinding {
        semantic: abstract_operations::ValueBinding {
            parameter: ValueId::new(2).unwrap(),
            argument: ValueId::new(2).unwrap(),
            scalar_type,
        },
        transport: SelectedValueTransport::Registers {
            argument: VirtualRegisterId(7),
            parameter: VirtualRegisterId(1),
        },
    });
    function.blocks[2].terminator = SelectedTerminator::ConditionalBranch {
        instruction: admission::instruction(
            SelectedInstructionId(2000),
            SelectedInstructionKind::Jump,
            jump,
            &[],
        ),
        when_nonzero: successor(5),
        when_zero: successor(4),
    };
    function.blocks.push(SelectedBlock {
        id: SelectedBlockId(4),
        origin: SelectedBlockOrigin::EdgeTransfer {
            edge: EdgeId::new(2).unwrap(),
            target: BlockId::new(3).unwrap(),
        },
        instructions: vec![admission::instruction(
            SelectedInstructionId(501),
            SelectedInstructionKind::CopyI64,
            copy,
            &[
                VirtualRegisterId(if passthrough { 1 } else { 5 }),
                VirtualRegisterId(7),
            ],
        )],
        terminator: SelectedTerminator::Jump {
            instruction: admission::instruction(
                SelectedInstructionId(500),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            successor: back,
        },
    });
    function.blocks.push(SelectedBlock {
        id: SelectedBlockId(5),
        origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
        instructions: Vec::new(),
        terminator: terminal,
    });
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

#[test]
fn cycles_disconnected_from_parameter_arrivals_do_not_freeze_spills() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let mut source = super::parameters::parameter_fixture(NativeTarget::linux_x64());
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let instruction = super::super::control(&function.blocks[0].terminator)
        .0
        .clone();
    // An unreachable self-loop makes the function cyclic without touching the
    // parameter's arrivals or uses; edge-initialized storage stays admitted.
    function.blocks.push(SelectedBlock {
        id: SelectedBlockId(4),
        origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
        instructions: Vec::new(),
        terminator: SelectedTerminator::Jump {
            instruction,
            successor: successor(4),
        },
    });
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
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

#[test]
fn loop_carried_parameters_store_on_every_back_edge_arrival() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = cyclic_parameter_fixture(target, false);
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let transformed = &result.transformed().functions[0];
        // Every arrival at the destination — both entry edges and the back
        // edge — stores its bound argument right after its edge copy.
        for (block_index, argument) in [(1usize, 2u32), (3, 3), (4, 7)] {
            let block = &transformed.blocks[block_index];
            assert!(matches!(
                block.instructions[1].kind,
                SelectedInstructionKind::Store64 { .. }
            ));
            assert_eq!(
                block.instructions[1].operands[0].virtual_register,
                VirtualRegisterId(argument)
            );
        }
        let destination = &transformed.blocks[2];
        assert_eq!(
            destination.instructions.len(),
            source.transformed().functions[0].blocks[2]
                .instructions
                .len()
                + 4
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
        for mutation in 0..4 {
            let mut proposed = result.transformed().clone();
            let function = &mut proposed.functions[0];
            match mutation {
                // Dropping the back-edge store leaves the slot stale on
                // re-entry: replay requires every arrival's store.
                0 => {
                    function.blocks[4].instructions.remove(1);
                }
                1 => function.blocks[4].instructions.swap(0, 1),
                2 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[4].terminator
                    else {
                        unreachable!()
                    };
                    let SelectedValueTransport::Registers { argument, .. } =
                        &mut successor.bindings[0].transport
                    else {
                        unreachable!()
                    };
                    *argument = VirtualRegisterId(1);
                }
                3 => {
                    function.blocks[4].terminator = SelectedTerminator::Jump {
                        instruction: admission::instruction(
                            SelectedInstructionId(500),
                            SelectedInstructionKind::Jump,
                            environment
                                .constraint(environment.selected_keys().jump)
                                .unwrap(),
                            &[],
                        ),
                        successor: successor(5),
                    };
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
fn passthrough_back_edges_reload_before_restoring_the_parameter() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = cyclic_parameter_fixture(target, true);
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    let transformed = &result.transformed().functions[0];
    // The back-edge copy reads the parameter: its reload precedes the copy and
    // the store follows it, so the slot first serves the old binding then
    // records the rebound one.
    let back_edge = &transformed.blocks[4];
    assert_eq!(back_edge.instructions.len(), 4);
    assert!(matches!(
        back_edge.instructions[0].kind,
        SelectedInstructionKind::FrameAddress { .. }
    ));
    assert!(matches!(
        back_edge.instructions[1].kind,
        SelectedInstructionKind::Load64 { .. }
    ));
    assert!(matches!(
        back_edge.instructions[2].kind,
        SelectedInstructionKind::CopyI64
    ));
    assert!(matches!(
        back_edge.instructions[3].kind,
        SelectedInstructionKind::Store64 { .. }
    ));
    assert_eq!(
        back_edge.instructions[2].operands[0].virtual_register,
        back_edge.instructions[1].operands[1].virtual_register
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

#[test]
fn cyclic_parameter_arrivals_still_require_dedicated_edge_stores() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for mutation in 0..3 {
        let mut source = cyclic_parameter_fixture(NativeTarget::linux_x64(), false);
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        match mutation {
            // The rebound parameter may not ride the destination's own
            // conditional edge: without the dedicated transfer block no
            // instruction slot exists that executes only on that arrival.
            0 => {
                let binding = SelectedValueBinding {
                    semantic: abstract_operations::ValueBinding {
                        parameter: ValueId::new(2).unwrap(),
                        argument: ValueId::new(2).unwrap(),
                        scalar_type: function.virtual_registers[1].scalar_type,
                    },
                    transport: SelectedValueTransport::Registers {
                        argument: VirtualRegisterId(7),
                        parameter: VirtualRegisterId(1),
                    },
                };
                let mut self_edge = successor(2);
                self_edge.bindings.push(binding);
                let instruction = super::super::control(&function.blocks[2].terminator)
                    .0
                    .clone();
                function.blocks[2].terminator = SelectedTerminator::ConditionalBranch {
                    instruction,
                    when_nonzero: successor(5),
                    when_zero: self_edge,
                };
            }
            // An argument not materialized by the edge's own copy stays
            // rejected: the store could not name an edge-exact value.
            1 => {
                let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[4].terminator
                else {
                    unreachable!()
                };
                successor.bindings[0].transport = SelectedValueTransport::Registers {
                    argument: VirtualRegisterId(5),
                    parameter: VirtualRegisterId(1),
                };
            }
            // Dropping the back-edge binding leaves the parameter
            // uninitialized on that arrival.
            2 => {
                let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[4].terminator
                else {
                    unreachable!()
                };
                successor.bindings.clear();
            }
            _ => unreachable!(),
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        assert!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn replay_rejects_changed_edges_untouched_blocks_and_foreign_settlements() {
    let source = cfg_fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    for mutation in 0..9 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            0 => {
                let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[1].terminator
                else {
                    unreachable!()
                };
                successor.block = SelectedBlockId(0);
            }
            1 => {
                let instruction = function.blocks[1].instructions[0].clone();
                function.blocks[0].instructions.push(instruction);
            }
            2 => function.blocks[2].origin = SelectedBlockOrigin::Source(BlockId::new(99).unwrap()),
            3 => function.boundary_settlements[0].instruction_index = 1,
            4 => function.boundary_settlements[2].instruction_index = 1,
            5 => function.boundary_settlements[4].block = SelectedBlockId(1),
            6 => {
                function.boundary_settlements[2].settlement =
                    SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                        operation: OperationId::new(2).unwrap(),
                        boundary: BoundaryMachineId::new(1).unwrap(),
                        source: ValueId::new(1).unwrap(),
                    }
            }
            7 => {
                let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator
                else {
                    unreachable!()
                };
                successor.role = SelectedSuccessorRole::Semantic;
            }
            8 => function.blocks.swap(0, 2),
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
fn case_payload_argument_and_parameter_references_remain_outside_spill_admission() {
    use selected_instructions::{
        SelectedCasePayloadBinding, SelectedCasePayloadTransport, SelectedStructuralCaseEdge,
    };
    use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for transport in [
        SelectedCasePayloadTransport::Registers {
            argument: VirtualRegisterId(1),
            parameter: VirtualRegisterId(4),
        },
        SelectedCasePayloadTransport::Registers {
            argument: VirtualRegisterId(4),
            parameter: VirtualRegisterId(1),
        },
        SelectedCasePayloadTransport::Unmaterialized {
            parameter: VirtualRegisterId(1),
        },
    ] {
        let mut source = cfg_fixture(NativeTarget::linux_x64());
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let scalar_type = function.virtual_registers[1].scalar_type;
        let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[1].terminator else {
            unreachable!()
        };
        successor.structural_case = Some(SelectedStructuralCaseEdge {
            slot: LocalStorageSlotId::Boundary {
                operation: OperationId::new(1).unwrap(),
            },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            trivial_affine_discards: Vec::new(),
            payloads: vec![SelectedCasePayloadBinding {
                semantic: legalized_operations::LegalizedStructuralCasePayload {
                    field: StructuralFieldId::new(1).unwrap(),
                    field_byte_offset: 0,
                    parameter: legalized_operations::LegalizedValueDefinition {
                        value: ValueId::new(1).unwrap(),
                        scalar_type,
                        definition_site: ValueDefinitionSite::BlockParameter {
                            block: BlockId::new(3).unwrap(),
                            position: 0,
                        },
                    },
                },
                transport,
            }],
        });
        assert_eq!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap_err(),
            RuntimeSpillError::UnsupportedUse
        );
    }
}

#[test]
fn hosted_boundary_locations_skip_reload_prefixes_but_eliminated_completions_do_not() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(target);
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let operation = OperationId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let slot = LocalStorageSlotId::Boundary { operation };
    let output = environment
        .constraint(environment.selected_keys().hosted_write_byte_i32.unwrap())
        .unwrap();
    function.blocks[0].instructions[1] = admission::instruction(
        SelectedInstructionId(2),
        SelectedInstructionKind::HostedWriteByteI32 { slot },
        output,
        &[VirtualRegisterId(1)],
    );
    function.boundary_settlements = vec![
        SelectedBoundarySettlement {
            block: SelectedBlockId(0),
            instruction_index: 1,
            settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                operation,
                boundary,
                source: ValueId::new(1).unwrap(),
            },
        },
        SelectedBoundarySettlement {
            block: SelectedBlockId(0),
            instruction_index: 1,
            settlement: SelectedBoundarySettlementPayload::ClaimCompletion(
                legalized_operations::LegalizedBoundarySettlement {
                    operation,
                    boundary,
                    provider_execution:
                        target_operations::ProviderExecutionBinding::from_execution_record(
                            target_operations::ProviderPlanReportIdentity::new(1).unwrap(),
                            1,
                            1,
                            1,
                            1,
                        )
                        .unwrap(),
                    realization: target_operations::ClaimCompletionOnlyRealization,
                    arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                    fuel: Vec::new(),
                    effect: optimization_unit::EffectLink {
                        input: 0,
                        output: 1,
                    },
                    ownership: Vec::new(),
                },
            ),
        },
    ];
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(function.boundary_settlements[0].instruction_index, 4);
    assert_eq!(function.boundary_settlements[1].instruction_index, 2);
    assert_eq!(
        function.blocks[0].instructions[4].kind,
        SelectedInstructionKind::HostedWriteByteI32 { slot }
    );
    for settlement_index in 0..2 {
        let mut proposed = result.transformed().clone();
        proposed.functions[0].boundary_settlements[settlement_index].instruction_index =
            if settlement_index == 0 { 2 } else { 4 };
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
            RuntimeSpillError::ReplayMismatch
        );
    }
}
