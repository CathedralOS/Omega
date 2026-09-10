use super::*;
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
                let operand = function.blocks[1].instructions[1].operands[0];
                let SelectedTerminator::Return { instruction, .. } =
                    &mut function.blocks[2].terminator
                else {
                    unreachable!()
                };
                instruction.operands.push(operand);
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
fn every_successor_transport_and_terminator_operand_excludes_the_victim() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for terminator_kind in 0..6 {
        for reference_kind in 0..4 {
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
                3 => edge.structural_bindings.push(SelectedStructuralBinding {
                    semantic: abstract_operations::AbstractStructuralBinding {
                        parameter: PlaceId::new(1).unwrap(),
                        argument: terminal_psi::StructuralArgument {
                            place: PlaceId::new(2).unwrap(),
                            path: Vec::new(),
                            access: terminal_psi::StructuralAccess::SharedBorrow,
                        },
                    },
                    transport: SelectedStructuralTransport::Descriptor {
                        argument: VirtualRegisterId(1),
                        destination: LocalStorageSlotId::Boundary {
                            operation: OperationId::new(1).unwrap(),
                        },
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
                assert_eq!(
                    admission::admit(&source, 0, VirtualRegisterId(1), &environment, budget())
                        .err(),
                    Some(RuntimeSpillError::UnsupportedUse)
                );
            }
        }
    }
}

#[test]
fn cyclic_functions_remain_frozen_even_when_the_victim_is_outside_the_cycle() {
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
        assert_eq!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap_err(),
            RuntimeSpillError::UnsupportedControlFlow
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
