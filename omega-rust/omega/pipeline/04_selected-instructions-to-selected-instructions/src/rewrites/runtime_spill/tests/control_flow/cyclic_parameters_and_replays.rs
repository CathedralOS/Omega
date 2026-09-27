use super::cfg_fixture;
use crate::rewrites::runtime_spill::admission;
use crate::rewrites::runtime_spill::tests::{
    Arc, BlockId, NativeTarget, SelectedBlockId, SelectedInstructionId, SelectedInstructionKind,
    SelectedTerminator, ValueDefinitionSite, ValueId, VirtualRegisterId,
    baseline_target_register_environment, budget, fixture,
};
use crate::{RuntimeSpillError, spill_selected_runtime_value, validate_runtime_spill};
use selected_instructions::{
    LocalStorageSlotId, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
};
use semantic_vocabulary::{BoundaryMachineId, OperationId};

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
            source: selected_instructions::SelectedCaseDispatchSource::Local {
                slot: LocalStorageSlotId::Boundary {
                    operation: OperationId::new(1).unwrap(),
                },
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
