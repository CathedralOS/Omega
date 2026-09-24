use super::{cfg_fixture, cyclic_parameter_fixture, successor};
use crate::rewrites::runtime_spill::admission;
use crate::rewrites::runtime_spill::tests::{
    Arc, BlockId, NativeTarget, SelectedBlock, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionKind, SelectedTerminator, ValueDefinitionSite, ValueId, VirtualRegisterId,
    baseline_target_register_environment, budget, fixture, selected_instruction_plan_identity,
};
use crate::{RuntimeSpillError, spill_selected_runtime_value, validate_runtime_spill};
use selected_instructions::{
    LocalStorageSlotId, SelectedBlockOrigin, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedSuccessorRole, SelectedValueBinding,
    SelectedValueTransport,
};
use semantic_vocabulary::{BoundaryMachineId, OperationId};

#[test]
fn cycles_disconnected_from_parameter_arrivals_do_not_freeze_spills() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let mut source = super::super::parameters::parameter_fixture(NativeTarget::linux_x64());
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let instruction = super::super::super::control(&function.blocks[0].terminator)
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
        // The destination's two body uses share one block-local reload pair.
        let destination = &transformed.blocks[2];
        assert_eq!(
            destination.instructions.len(),
            source.transformed().functions[0].blocks[2]
                .instructions
                .len()
                + 2
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
                let instruction = super::super::super::control(&function.blocks[2].terminator)
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

// The victim here is an instruction result, so a payload's parameter side can
// never be its own incoming definition — the edge-definition allowance for
// block parameters does not apply. (That allowance is exercised by the
// case-payload parameter fixtures in `parameters.rs`.)
#[test]
fn case_payload_parameter_and_unmaterialized_references_remain_outside_spill_admission() {
    use selected_instructions::{
        SelectedCasePayloadBinding, SelectedCasePayloadTransport, SelectedStructuralCaseEdge,
    };
    use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for mutation in 0..3 {
        let mut source = cfg_fixture(NativeTarget::linux_x64());
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let mut scalar_type = function.virtual_registers[1].scalar_type;
        let transport = match mutation {
            // The parameter side is the destination's payload definition,
            // never a use the predecessor's slot can serve.
            0 => SelectedCasePayloadTransport::Registers {
                argument: VirtualRegisterId(4),
                parameter: VirtualRegisterId(1),
            },
            1 => SelectedCasePayloadTransport::Unmaterialized {
                parameter: VirtualRegisterId(1),
            },
            // A payload argument whose declared type differs from the
            // victim's is an inconsistent plan, not an admitted use.
            _ => {
                scalar_type = semantic_vocabulary::ScalarType::Boolean;
                SelectedCasePayloadTransport::Registers {
                    argument: VirtualRegisterId(1),
                    parameter: VirtualRegisterId(4),
                }
            }
        };
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
            RuntimeSpillError::UnsupportedUse,
            "mutation {mutation}"
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
