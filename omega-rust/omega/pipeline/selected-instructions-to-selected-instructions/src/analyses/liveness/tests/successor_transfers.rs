//! Successor binding substitution and block-parameter fixtures.

use register_model::{RegisterClassId, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstructionId,
    SelectedInstructionKind, SelectedOperand, SelectedSuccessor, SelectedTerminator,
    VirtualRegisterId,
};
use semantic_vocabulary::{BlockId, EdgeId};

use super::{compute_function, function_with_operand};

pub(crate) fn successor_parameter_function() -> SelectedFunction {
    use optimization_unit::ValueDefinitionSite;
    use selected_instructions::{VirtualRegister, VirtualRegisterOrigin};
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = |number| ValueId::new(number).unwrap();
    let mut function = function_with_operand(RegisterOperandAccess::Use);
    let mut jump = function.blocks[0].instructions.remove(0);
    jump.kind = SelectedInstructionKind::Jump;
    jump.operands.clear();
    let mut return_instruction = jump.clone();
    return_instruction.id = SelectedInstructionId(1);
    return_instruction.kind = SelectedInstructionKind::ReturnScalar;
    return_instruction.operands.push(SelectedOperand {
        operand: 0,
        virtual_register: VirtualRegisterId(2),
        access: RegisterOperandAccess::Use,
        class: RegisterClassId(0),
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    });
    function.blocks[0].terminator = SelectedTerminator::Jump {
        instruction: jump,
        successor: SelectedSuccessor {
            role: selected_instructions::SelectedSuccessorRole::Semantic,
            structural_case: None,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(1).unwrap(),
            block: SelectedBlockId(1),
            source_target: BlockId::new(2).unwrap(),
            bindings: vec![selected_instructions::SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: value(3),
                    argument: value(1),
                    scalar_type,
                },
                transport: selected_instructions::SelectedValueTransport::Registers {
                    argument: VirtualRegisterId(0),
                    parameter: VirtualRegisterId(2),
                },
            }],
            fuel: Vec::new(),
        },
    };
    function.blocks.push(SelectedBlock {
        id: SelectedBlockId(1),
        origin: selected_instructions::SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
        instructions: Vec::new(),
        terminator: SelectedTerminator::Return {
            instruction: return_instruction,
            psi_return_edge: EdgeId::new(2).unwrap(),
        },
    });
    function.virtual_registers = (0..3)
        .map(|index| VirtualRegister {
            id: VirtualRegisterId(index),
            scalar_type,
            class: RegisterClassId(0),
            entry_fixed_view: None,
            origin: if index < 2 {
                VirtualRegisterOrigin::EntryParameter {
                    source_value: value(u64::from(index) + 1),
                    parameter_index: index as usize,
                }
            } else {
                VirtualRegisterOrigin::BlockParameter {
                    source_value: value(3),
                    block: SelectedBlockId(1),
                    parameter_index: 0,
                }
            },
            definition_site: Some(if index < 2 {
                ValueDefinitionSite::FunctionParameter(index)
            } else {
                ValueDefinitionSite::BlockParameter {
                    block: BlockId::new(2).unwrap(),
                    position: 0,
                }
            }),
        })
        .collect();
    function
}

#[test]
fn successor_parameter_liveness_substitutes_the_actual_edge_argument() {
    let mut selected = successor_parameter_function();
    let live = compute_function(0, &selected).unwrap();
    assert_eq!(live.blocks[1].virtual_live_in, [VirtualRegisterId(2)]);
    assert_eq!(live.blocks[0].virtual_live_out, [VirtualRegisterId(0)]);
    assert_eq!(
        live.blocks[0].successors[0].virtual_live,
        [VirtualRegisterId(0)]
    );
    let SelectedTerminator::Jump { successor, .. } = &mut selected.blocks[0].terminator else {
        unreachable!()
    };
    successor.bindings[0].semantic.argument = semantic_vocabulary::ValueId::new(2).unwrap();
    successor.bindings[0].transport = selected_instructions::SelectedValueTransport::Registers {
        argument: VirtualRegisterId(1),
        parameter: VirtualRegisterId(2),
    };
    let changed = compute_function(0, &selected).unwrap();
    assert_eq!(changed.blocks[0].virtual_live_out, [VirtualRegisterId(1)]);
    assert_ne!(live, changed);
}

#[test]
fn source_blocks_cannot_claim_implementation_edge_roles() {
    let mut function = successor_parameter_function();
    let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    successor.role = selected_instructions::SelectedSuccessorRole::EdgeTransferContinuation;
    assert!(compute_function(0, &function).is_err());
}

#[test]
fn successor_transport_distinguishes_duplicate_semantic_copies() {
    let mut function = successor_parameter_function();
    let mut copied = function.virtual_registers[0].clone();
    copied.id = VirtualRegisterId(3);
    function.virtual_registers.push(copied);
    let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    successor.bindings[0].transport = selected_instructions::SelectedValueTransport::Registers {
        argument: VirtualRegisterId(3),
        parameter: VirtualRegisterId(2),
    };
    let live = compute_function(0, &function).unwrap();
    assert_eq!(live.blocks[0].virtual_live_out, [VirtualRegisterId(3)]);
}

#[test]
fn successor_transport_rejects_absent_wrong_and_unused_register_pairs() {
    use selected_instructions::SelectedValueTransport;
    for transport in [
        SelectedValueTransport::Unused,
        SelectedValueTransport::Registers {
            argument: VirtualRegisterId(99),
            parameter: VirtualRegisterId(2),
        },
        SelectedValueTransport::Registers {
            argument: VirtualRegisterId(1),
            parameter: VirtualRegisterId(2),
        },
        SelectedValueTransport::Registers {
            argument: VirtualRegisterId(0),
            parameter: VirtualRegisterId(1),
        },
    ] {
        let mut function = successor_parameter_function();
        let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
            unreachable!()
        };
        successor.bindings[0].transport = transport;
        assert!(compute_function(0, &function).is_err());
    }
}

fn case_payload_function() -> SelectedFunction {
    use selected_instructions::{
        LocalStorageSlotId, SelectedCasePayloadBinding, SelectedCasePayloadTransport,
        SelectedStructuralCaseEdge, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralCaseId,
        StructuralFieldId, ValueId,
    };
    let mut function = successor_parameter_function();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    function.virtual_registers[0].scalar_type = scalar_type;
    function.virtual_registers[2].scalar_type = scalar_type;
    function.virtual_registers[0].origin = VirtualRegisterOrigin::StructuralObservation {
        instruction: SelectedInstructionId(2),
        place: PlaceId::new(1).unwrap(),
        byte_offset: 4,
    };
    function.virtual_registers[0].definition_site = None;
    let SelectedTerminator::Jump {
        instruction,
        successor,
    } = &mut function.blocks[0].terminator
    else {
        unreachable!()
    };
    successor.bindings.clear();
    successor.structural_case = Some(SelectedStructuralCaseEdge {
        slot: LocalStorageSlotId::Structural {
            operation: OperationId::new(1).unwrap(),
            place: PlaceId::new(1).unwrap(),
        },
        case: StructuralCaseId::new(2).unwrap(),
        case_tag: 1,
        trivial_affine_discards: vec![PlaceId::new(1).unwrap()],
        payloads: vec![SelectedCasePayloadBinding {
            semantic: legalized_operations::LegalizedStructuralCasePayload {
                field: StructuralFieldId::new(1).unwrap(),
                field_byte_offset: 4,
                parameter: legalized_operations::LegalizedValueDefinition {
                    value: ValueId::new(3).unwrap(),
                    scalar_type,
                    definition_site: optimization_unit::ValueDefinitionSite::BlockParameter {
                        block: BlockId::new(2).unwrap(),
                        position: 0,
                    },
                },
            },
            transport: SelectedCasePayloadTransport::Registers {
                argument: VirtualRegisterId(0),
                parameter: VirtualRegisterId(2),
            },
        }],
    });
    let mut load = instruction.clone();
    load.id = SelectedInstructionId(2);
    load.kind = SelectedInstructionKind::Load32 { byte_offset: 0 };
    load.operands = vec![
        SelectedOperand {
            operand: 0,
            virtual_register: VirtualRegisterId(1),
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        },
        SelectedOperand {
            operand: 1,
            virtual_register: VirtualRegisterId(0),
            access: RegisterOperandAccess::Def,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        },
    ];
    function.blocks[0].instructions.push(load);
    function
}

#[test]
fn case_payload_liveness_uses_exact_observation_and_rejects_unprepared_or_substituted_transport() {
    use selected_instructions::{SelectedCasePayloadTransport, VirtualRegisterOrigin};
    let function = case_bridge_function();
    let live = compute_function(0, &function).unwrap();
    assert_eq!(live.blocks[2].virtual_live_out, [VirtualRegisterId(0)]);
    assert_eq!(live.blocks[1].virtual_live_in, [VirtualRegisterId(2)]);
    assert!(super::super::edge_values::has_edge_use(
        &function,
        VirtualRegisterId(0)
    ));
    for mutation in 0..6 {
        let mut changed = function.clone();
        let SelectedTerminator::Jump { successor, .. } = &mut changed.blocks[2].terminator else {
            unreachable!()
        };
        let case = successor.structural_case.as_mut().unwrap();
        match mutation {
            0 => {
                case.payloads[0].transport = SelectedCasePayloadTransport::Unmaterialized {
                    parameter: VirtualRegisterId(2),
                }
            }
            1 => case.payloads[0].transport = SelectedCasePayloadTransport::Unused,
            2 => case.payloads[0].semantic.field_byte_offset = 0,
            3 => {
                case.payloads[0].semantic.parameter.definition_site =
                    optimization_unit::ValueDefinitionSite::BlockParameter {
                        block: BlockId::new(2).unwrap(),
                        position: 1,
                    }
            }
            4 => {
                changed.virtual_registers[0].definition_site =
                    Some(optimization_unit::ValueDefinitionSite::FunctionParameter(0))
            }
            _ => {
                changed.virtual_registers[0].origin = VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(2),
                    source_value: semantic_vocabulary::ValueId::new(3).unwrap(),
                }
            }
        }
        assert!(
            compute_function(0, &changed).is_err(),
            "mutation {mutation}"
        );
    }
}

fn case_bridge_function() -> SelectedFunction {
    use selected_instructions::{
        SelectedBlockOrigin, SelectedCasePayloadTransport, SelectedSuccessorRole,
    };
    let mut function = case_payload_function();
    let loads = std::mem::take(&mut function.blocks[0].instructions);
    let SelectedTerminator::Jump {
        instruction,
        successor,
    } = &mut function.blocks[0].terminator
    else {
        unreachable!()
    };
    let mut continuation = successor.clone();
    continuation.role = SelectedSuccessorRole::EdgeTransferContinuation;
    continuation
        .structural_case
        .as_mut()
        .unwrap()
        .trivial_affine_discards
        .clear();
    let mut jump = instruction.clone();
    jump.id = SelectedInstructionId(3);
    successor.block = SelectedBlockId(2);
    successor.structural_case.as_mut().unwrap().payloads[0].transport =
        SelectedCasePayloadTransport::Unused;
    let bridge_origin = SelectedBlockOrigin::EdgeTransfer {
        edge: successor.psi_edge,
        target: successor.source_target,
    };
    function.blocks.push(SelectedBlock {
        id: SelectedBlockId(2),
        origin: bridge_origin,
        instructions: loads,
        terminator: SelectedTerminator::Jump {
            instruction: jump,
            successor: continuation,
        },
    });
    function
}

#[test]
fn case_payload_registers_reject_eager_semantic_edge_observations() {
    assert!(compute_function(0, &case_payload_function()).is_err());
}

#[test]
fn case_bridge_retains_metadata_but_runs_cleanup_only_on_semantic_leg() {
    let function = case_bridge_function();
    assert!(compute_function(0, &function).is_ok());
    for mutation in 0..3 {
        let mut changed = function.clone();
        let SelectedTerminator::Jump { successor, .. } = &mut changed.blocks[2].terminator else {
            unreachable!()
        };
        match mutation {
            0 => successor.structural_case.as_mut().unwrap().case_tag = 0,
            1 => successor
                .structural_case
                .as_mut()
                .unwrap()
                .trivial_affine_discards
                .push(semantic_vocabulary::PlaceId::new(1).unwrap()),
            _ => successor.structural_case = None,
        }
        assert!(
            compute_function(0, &changed).is_err(),
            "mutation {mutation}"
        );
    }
}
