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
    return_instruction.kind = SelectedInstructionKind::ReturnI64;
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
        source_block: BlockId::new(2).unwrap(),
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
            definition_site: if index < 2 {
                ValueDefinitionSite::FunctionParameter(index)
            } else {
                ValueDefinitionSite::BlockParameter {
                    block: BlockId::new(2).unwrap(),
                    position: 0,
                }
            },
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
