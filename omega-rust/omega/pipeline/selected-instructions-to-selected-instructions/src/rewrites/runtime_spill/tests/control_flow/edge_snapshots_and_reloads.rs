use super::{cfg_fixture, successor};
use crate::rewrites::runtime_spill::admission;
use crate::rewrites::runtime_spill::tests::{
    Arc, BlockId, EdgeId, NativeTarget, ScalarType, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionKind, SelectedTerminator, ValueDefinitionSite, ValueId, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin, baseline_target_register_environment, budget,
    selected_instruction_plan_identity,
};
use crate::{RuntimeSpillError, spill_selected_runtime_value, validate_runtime_spill};
use selected_instructions::{
    LocalStorageSlotId, SelectedStructuralBinding, SelectedStructuralTransport,
    SelectedValueBinding, SelectedValueTransport,
};
use semantic_vocabulary::{OperationId, PlaceId};

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
                    // One definition store plus the single shared reload pair
                    // serving all three body uses.
                    assert_eq!(after.instructions.len(), 7);
                    assert_eq!(after.instructions[1].id, SelectedInstructionId(1001));
                }
            }
            assert_eq!(
                transformed
                    .boundary_settlements
                    .iter()
                    .map(|settlement| settlement.instruction_index)
                    .collect::<Vec<_>>(),
                [0, 0, 4, 7, 0]
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
                // A `Def` on the victim after its origin is now an admitted
                // redefinition with its own store; ahead of the origin in
                // the origin's own block it stays a write before existence.
                let mut duplicate = function.blocks[1].instructions[0].clone();
                duplicate.id = SelectedInstructionId(500);
                function.blocks[1].instructions.insert(0, duplicate);
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
                super::super::super::control_mut(&mut function.blocks[0].terminator)
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
            let mut instruction = super::super::super::control(&function.blocks[1].terminator)
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
                let admitted = admission::admit(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    crate::RuntimeSpillSpanPolicy::UnitWriteBounded,
                    budget(),
                );
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
            let original_operand = super::super::super::control(&original.blocks[2].terminator)
                .0
                .operands[0];
            let rewritten_operand = super::super::super::control(&block.terminator).0.operands[0];
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
                        super::super::super::control_mut(&mut function.blocks[2].terminator)
                            .operands[0]
                            .virtual_register = VirtualRegisterId(1);
                    }
                    3 => {
                        super::super::super::control_mut(&mut function.blocks[2].terminator)
                            .operands[0]
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
        // The three body uses share one pair; the edge-argument use keeps a
        // private pair landing after the last body instruction.
        assert_eq!(
            block.instructions.len(),
            original.blocks[1].instructions.len() + 1 + 4
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
            super::super::super::control(&original.blocks[1].terminator).1[0]
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
        let instruction = super::super::super::control(&function.blocks[1].terminator)
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
    // One edge use per successor: two private pairs after the definition
    // store and the single shared body-use pair, in successor order.
    assert_eq!(block.instructions.len(), 4 + 1 + 2 + 4);
    let SelectedTerminator::ConditionalBranch {
        when_nonzero,
        when_zero,
        ..
    } = &block.terminator
    else {
        unreachable!()
    };
    for (successor, tail) in [(when_nonzero, 8), (when_zero, 10)] {
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
fn case_payload_arguments_reload_after_binding_pairs_on_every_target() {
    use selected_instructions::{
        SelectedCasePayloadBinding, SelectedCasePayloadTransport, SelectedStructuralCaseEdge,
    };
    use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};
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
            let class = function.virtual_registers[1].class;
            // The destination's parameter registers receive the edge's value
            // binding and both case payloads.
            for (id, position) in [(5u32, 0u32), (6, 1), (7, 2)] {
                function.virtual_registers.push(VirtualRegister {
                    id: VirtualRegisterId(id),
                    scalar_type,
                    class,
                    origin: VirtualRegisterOrigin::BlockParameter {
                        source_value: ValueId::new(2).unwrap(),
                        block: SelectedBlockId(2),
                        parameter_index: position as usize,
                    },
                    definition_site: Some(ValueDefinitionSite::BlockParameter {
                        block: BlockId::new(3).unwrap(),
                        position,
                    }),
                    entry_fixed_view: None,
                });
            }
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
            successor.structural_case = Some(SelectedStructuralCaseEdge {
                slot: LocalStorageSlotId::Boundary {
                    operation: OperationId::new(1).unwrap(),
                },
                case: StructuralCaseId::new(1).unwrap(),
                case_tag: 0,
                trivial_affine_discards: Vec::new(),
                payloads: [6u32, 7]
                    .iter()
                    .map(|id| SelectedCasePayloadBinding {
                        semantic: legalized_operations::LegalizedStructuralCasePayload {
                            field: StructuralFieldId::new(u64::from(*id)).unwrap(),
                            field_byte_offset: 0,
                            parameter: legalized_operations::LegalizedValueDefinition {
                                value: ValueId::new(u64::from(*id)).unwrap(),
                                scalar_type,
                                definition_site: ValueDefinitionSite::BlockParameter {
                                    block: BlockId::new(3).unwrap(),
                                    position: id - 5,
                                },
                            },
                        },
                        transport: SelectedCasePayloadTransport::Registers {
                            argument: VirtualRegisterId(1),
                            parameter: VirtualRegisterId(*id),
                        },
                    })
                    .collect(),
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
        // The three body uses share one pair; the definition store, then the
        // private binding pair and both payload pairs follow in declaration
        // order at the end of the block.
        assert_eq!(
            block.instructions.len(),
            original.blocks[1].instructions.len() + 1 + 2 + 6
        );
        let tail = block.instructions.len() - 6;
        for offset in [0usize, 2, 4] {
            assert!(matches!(
                block.instructions[tail + offset].kind,
                SelectedInstructionKind::FrameAddress { .. }
            ));
            assert!(matches!(
                block.instructions[tail + offset + 1].kind,
                SelectedInstructionKind::Load64 { .. }
            ));
        }
        let SelectedTerminator::Jump { successor, .. } = &block.terminator else {
            unreachable!()
        };
        let binding_register = block.instructions[tail + 1].operands[1].virtual_register;
        assert_eq!(
            successor.bindings[0].transport,
            SelectedValueTransport::Registers {
                argument: binding_register,
                parameter: VirtualRegisterId(5),
            }
        );
        let case = successor.structural_case.as_ref().unwrap();
        for (payload_index, offset) in [(0usize, 2usize), (1, 4)] {
            let reload_register =
                block.instructions[tail + offset + 1].operands[1].virtual_register;
            assert_eq!(
                case.payloads[payload_index].transport,
                SelectedCasePayloadTransport::Registers {
                    argument: reload_register,
                    parameter: VirtualRegisterId(6 + payload_index as u32),
                }
            );
            // The payload keeps its exact declared parameter and field.
            assert_eq!(
                case.payloads[payload_index].semantic,
                super::super::super::control(&original.blocks[1].terminator).1[0]
                    .unwrap()
                    .structural_case
                    .as_ref()
                    .unwrap()
                    .payloads[payload_index]
                    .semantic
            );
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
        for mutation in 0..6 {
            let mut proposed = result.transformed().clone();
            let function = &mut proposed.functions[0];
            match mutation {
                0 => {
                    function.blocks[1].instructions.remove(tail + 5);
                }
                1 => {
                    function.blocks[1].instructions.swap(tail + 4, tail + 5);
                }
                2 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[1].terminator
                    else {
                        unreachable!()
                    };
                    let SelectedCasePayloadTransport::Registers { argument, .. } =
                        &mut successor.structural_case.as_mut().unwrap().payloads[0].transport
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
                    successor.structural_case.as_mut().unwrap().payloads[1]
                        .semantic
                        .parameter
                        .value = ValueId::new(99).unwrap();
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
                    successor
                        .structural_case
                        .as_mut()
                        .unwrap()
                        .payloads
                        .remove(1);
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
        let instruction = super::super::super::control(&function.blocks[0].terminator)
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
