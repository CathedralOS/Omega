//! Liveness fixtures and focused transfer tests.

use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
    RegisterUnitId,
};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedOperand, SelectedSuccessor,
    SelectedTerminator, VirtualRegisterId,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};

use super::compute::{compute_function, reject_unsupported_constraints};
use crate::LivenessError;

mod successor_transfers;

pub(crate) use successor_transfers::successor_parameter_function;

#[test]
fn ordinary_call_and_return_retain_exact_unit_liveness() {
    let mut caller = function_with_operand(RegisterOperandAccess::Use);
    caller.blocks[0].instructions[0].operands.clear();
    let call = &mut caller.blocks[0].instructions[0];
    call.kind = SelectedInstructionKind::CallUnit {
        callee: MachineId::new(2).unwrap(),
    };
    call.implicit_uses = vec![RegisterUnitId(1), RegisterUnitId(2)];
    call.implicit_defs = vec![RegisterUnitId(3)];
    call.clobbers = vec![RegisterUnitId(4)];
    let SelectedTerminator::Return { instruction, .. } = &mut caller.blocks[0].terminator else {
        unreachable!()
    };
    instruction.kind = SelectedInstructionKind::ReturnUnit;
    instruction.implicit_uses = vec![RegisterUnitId(3)];
    let liveness = compute_function(0, &caller).unwrap();
    assert!(liveness.entry_definitions.is_empty());
    assert!(liveness.operand_positions.is_empty());
    assert_eq!(
        liveness.blocks[0].unit_live_in,
        vec![RegisterUnitId(1), RegisterUnitId(2)]
    );
    assert!(liveness.blocks[0].unit_live_out.is_empty());
    assert_eq!(liveness.blocks[0].instructions.len(), 2);
    assert_eq!(
        liveness.blocks[0].instructions[0].position,
        crate::LivenessPosition(0)
    );
    assert_eq!(
        liveness.blocks[0].instructions[0].unit_live_out,
        vec![RegisterUnitId(3)]
    );
    assert_eq!(
        liveness.blocks[0].instructions[0].unit_clobbers,
        vec![RegisterUnitId(4)]
    );
    assert_eq!(
        liveness.blocks[0].instructions[1].instruction,
        SelectedInstructionId(1)
    );

    let mut callee = caller;
    callee.machine = MachineId::new(2).unwrap();
    callee.blocks[0].instructions.clear();
    let SelectedTerminator::Return { instruction, .. } = &mut callee.blocks[0].terminator else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(0);
    instruction.implicit_uses = vec![RegisterUnitId(5)];
    instruction.implicit_defs = vec![RegisterUnitId(6)];
    let liveness = compute_function(1, &callee).unwrap();
    assert_eq!(liveness.machine, callee.machine);
    assert_eq!(liveness.blocks[0].instructions.len(), 1);
    assert_eq!(liveness.blocks[0].unit_live_in, vec![RegisterUnitId(5)]);
    assert_eq!(
        liveness.blocks[0].instructions[0].unit_defs,
        vec![RegisterUnitId(6)]
    );
}

#[test]
fn integer_less_than_successors_retain_semantic_polarity_order() {
    let key = RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: 99,
    };
    let instruction = |id, kind| SelectedInstruction {
        id: SelectedInstructionId(id),
        kind,
        constraint: key,
        operands: Vec::new(),
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    };
    let successor = |edge, block, source_target| SelectedSuccessor {
        psi_edge: EdgeId::new(edge).unwrap(),
        block: SelectedBlockId(block),
        source_target: BlockId::new(source_target).unwrap(),
        bindings: Vec::new(),
        fuel: Vec::new(),
    };
    for signed in [false, true] {
        let branch_instruction = instruction(
            0,
            if signed {
                SelectedInstructionKind::ConditionalBranchI64LessThan
            } else {
                SelectedInstructionKind::ConditionalBranchU64LessThan
            },
        );
        let terminator = if signed {
            SelectedTerminator::ConditionalBranchI64LessThan {
                instruction: branch_instruction,
                when_less: successor(1, 1, 2),
                when_not_less: successor(2, 2, 3),
            }
        } else {
            SelectedTerminator::ConditionalBranchU64LessThan {
                instruction: branch_instruction,
                when_less: successor(1, 1, 2),
                when_not_less: successor(2, 2, 3),
            }
        };
        let function = SelectedFunction {
            ranked: None,
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: Default::default(),
            structural: None,
            outgoing_arguments: Vec::new(),
            local_storage_slots: Vec::new(),
            calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: Vec::new(),
            blocks: vec![
                SelectedBlock {
                    id: SelectedBlockId(0),
                    source_block: BlockId::new(1).unwrap(),
                    instructions: Vec::new(),
                    terminator,
                },
                SelectedBlock {
                    id: SelectedBlockId(1),
                    source_block: BlockId::new(2).unwrap(),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Return {
                        instruction: instruction(1, SelectedInstructionKind::ReturnUnit),
                        psi_return_edge: EdgeId::new(3).unwrap(),
                    },
                },
                SelectedBlock {
                    id: SelectedBlockId(2),
                    source_block: BlockId::new(3).unwrap(),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Return {
                        instruction: instruction(2, SelectedInstructionKind::ReturnUnit),
                        psi_return_edge: EdgeId::new(4).unwrap(),
                    },
                },
            ],
        };

        let liveness = compute_function(0, &function).unwrap();
        assert_eq!(liveness.blocks[0].successors.len(), 2);
        assert_eq!(liveness.blocks[0].successors[0].polarity_ordinal, 0);
        assert_eq!(liveness.blocks[0].successors[0].target, SelectedBlockId(1));
        assert_eq!(liveness.blocks[0].successors[1].polarity_ordinal, 1);
        assert_eq!(liveness.blocks[0].successors[1].target, SelectedBlockId(2));
    }
}

pub(crate) fn function_with_operand(access: RegisterOperandAccess) -> SelectedFunction {
    let key = RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: 99,
    };
    let instruction = SelectedInstruction {
        id: SelectedInstructionId(0),
        kind: SelectedInstructionKind::CompareI64Zero,
        constraint: key,
        operands: vec![SelectedOperand {
            operand: 0,
            virtual_register: VirtualRegisterId(0),
            access,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    };
    SelectedFunction {
        ranked: None,
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        provenance: Default::default(),
        structural: None,
        outgoing_arguments: Vec::new(),
        local_storage_slots: Vec::new(),
        calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: SelectedBlockId(0),
        virtual_registers: Vec::new(),
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            source_block: BlockId::new(1).unwrap(),
            instructions: vec![instruction],
            terminator: SelectedTerminator::Return {
                instruction: SelectedInstruction {
                    id: SelectedInstructionId(1),
                    kind: SelectedInstructionKind::ReturnI64,
                    constraint: key,
                    operands: Vec::new(),
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: SelectedInstructionProvenance::default(),
                },
                psi_return_edge: EdgeId::new(1).unwrap(),
            },
        }],
    }
}

pub(crate) fn supported_tied_function() -> SelectedFunction {
    let mut function = function_with_operand(RegisterOperandAccess::Use);
    function.blocks[0].instructions[0]
        .operands
        .push(SelectedOperand {
            operand: 1,
            virtual_register: VirtualRegisterId(1),
            access: RegisterOperandAccess::Def,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: Some(0),
            early_clobber: false,
        });
    function
}

pub(crate) fn supported_tied_component_function() -> SelectedFunction {
    let mut function = supported_tied_function();
    let key = function.blocks[0].instructions[0].constraint;
    function.blocks[0].instructions.push(SelectedInstruction {
        id: SelectedInstructionId(1),
        kind: SelectedInstructionKind::CompareI64Zero,
        constraint: key,
        operands: vec![
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
                virtual_register: VirtualRegisterId(2),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: Some(0),
                early_clobber: false,
            },
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    });
    let SelectedTerminator::Return { instruction, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(2);
    function
}

pub(crate) fn supported_early_clobber_function() -> SelectedFunction {
    let mut function = function_with_operand(RegisterOperandAccess::Use);
    function.blocks[0].instructions[0]
        .operands
        .push(SelectedOperand {
            operand: 1,
            virtual_register: VirtualRegisterId(1),
            access: RegisterOperandAccess::Def,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: true,
        });
    function
}

pub(crate) fn supported_multiple_early_clobber_function() -> SelectedFunction {
    let mut function = supported_early_clobber_function();
    let key = function.blocks[0].instructions[0].constraint;
    function.blocks[0].instructions.push(SelectedInstruction {
        id: SelectedInstructionId(1),
        kind: SelectedInstructionKind::CompareI64Zero,
        constraint: key,
        operands: vec![
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
                virtual_register: VirtualRegisterId(2),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: true,
            },
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    });
    let SelectedTerminator::Return { instruction, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(2);
    function
}

pub(crate) fn supported_isolated_tied_early_clobber_function() -> SelectedFunction {
    let mut function = function_with_operand(RegisterOperandAccess::Use);
    function.blocks[0].instructions[0].operands.extend([
        SelectedOperand {
            operand: 1,
            virtual_register: VirtualRegisterId(1),
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        },
        SelectedOperand {
            operand: 2,
            virtual_register: VirtualRegisterId(2),
            access: RegisterOperandAccess::Def,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: Some(0),
            early_clobber: true,
        },
    ]);
    function
}

pub(crate) fn supported_multiple_isolated_tied_early_clobber_function() -> SelectedFunction {
    let mut function = supported_isolated_tied_early_clobber_function();
    let key = function.blocks[0].instructions[0].constraint;
    function.blocks[0].instructions.push(SelectedInstruction {
        id: SelectedInstructionId(1),
        kind: SelectedInstructionKind::CompareI64Zero,
        constraint: key,
        operands: vec![
            SelectedOperand {
                operand: 0,
                virtual_register: VirtualRegisterId(3),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            SelectedOperand {
                operand: 1,
                virtual_register: VirtualRegisterId(4),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            SelectedOperand {
                operand: 2,
                virtual_register: VirtualRegisterId(5),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: Some(0),
                early_clobber: true,
            },
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    });
    let SelectedTerminator::Return { instruction, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(2);
    function
}

pub(crate) fn supported_component_tied_early_clobber_function() -> SelectedFunction {
    let mut function = supported_isolated_tied_early_clobber_function();
    let key = function.blocks[0].instructions[0].constraint;
    let mut early = function.blocks[0].instructions.remove(0);
    early.id = SelectedInstructionId(1);
    early.operands[0].virtual_register = VirtualRegisterId(1);
    early.operands[1].virtual_register = VirtualRegisterId(2);
    early.operands[2].virtual_register = VirtualRegisterId(3);
    function.blocks[0].instructions.push(SelectedInstruction {
        id: SelectedInstructionId(0),
        kind: SelectedInstructionKind::CompareI64Zero,
        constraint: key,
        operands: vec![
            SelectedOperand {
                operand: 0,
                virtual_register: VirtualRegisterId(0),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            SelectedOperand {
                operand: 1,
                virtual_register: VirtualRegisterId(1),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: Some(0),
                early_clobber: false,
            },
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    });
    function.blocks[0].instructions.push(early);
    let SelectedTerminator::Return { instruction, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(2);
    function
}

pub(crate) fn supported_multiple_component_tied_early_clobber_function() -> SelectedFunction {
    let mut function = supported_component_tied_early_clobber_function();
    let mut ordinary = function.blocks[0].instructions[0].clone();
    ordinary.id = SelectedInstructionId(2);
    for operand in &mut ordinary.operands {
        operand.virtual_register.0 += 4;
    }
    let mut early = function.blocks[0].instructions[1].clone();
    early.id = SelectedInstructionId(3);
    for operand in &mut early.operands {
        operand.virtual_register.0 += 4;
    }
    function.blocks[0].instructions.extend([ordinary, early]);
    let SelectedTerminator::Return { instruction, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(4);
    function
}

#[test]
fn admits_only_distinct_use_to_def_ties_and_rejects_other_phase_frontiers() {
    let use_def = function_with_operand(RegisterOperandAccess::UseDef);
    assert!(matches!(
        reject_unsupported_constraints(0, &use_def),
        Err(LivenessError::UnsupportedUseDef { .. })
    ));

    let supported = supported_tied_function();
    assert_eq!(reject_unsupported_constraints(0, &supported), Ok(()));

    let component = supported_tied_component_function();
    assert_eq!(reject_unsupported_constraints(0, &component), Ok(()));

    let mut multiple_defs = supported_tied_function();
    multiple_defs.blocks[0].instructions[0]
        .operands
        .push(SelectedOperand {
            operand: 2,
            virtual_register: VirtualRegisterId(2),
            access: RegisterOperandAccess::Def,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: Some(0),
            early_clobber: false,
        });
    assert_eq!(reject_unsupported_constraints(0, &multiple_defs), Ok(()));

    let early_supported = supported_early_clobber_function();
    assert_eq!(reject_unsupported_constraints(0, &early_supported), Ok(()));

    let multiple_early = supported_multiple_early_clobber_function();
    assert_eq!(reject_unsupported_constraints(0, &multiple_early), Ok(()));

    let composed = supported_isolated_tied_early_clobber_function();
    assert_eq!(reject_unsupported_constraints(0, &composed), Ok(()));

    let multiple_composed = supported_multiple_isolated_tied_early_clobber_function();
    assert_eq!(
        reject_unsupported_constraints(0, &multiple_composed),
        Ok(())
    );

    let component_composed = supported_component_tied_early_clobber_function();
    assert_eq!(
        reject_unsupported_constraints(0, &component_composed),
        Ok(())
    );

    let multiple_components = supported_multiple_component_tied_early_clobber_function();
    assert_eq!(
        reject_unsupported_constraints(0, &multiple_components),
        Ok(())
    );

    let mut ordinary_reuse = supported_isolated_tied_early_clobber_function();
    let key = ordinary_reuse.blocks[0].instructions[0].constraint;
    ordinary_reuse.blocks[0]
        .instructions
        .push(SelectedInstruction {
            id: SelectedInstructionId(1),
            kind: SelectedInstructionKind::CompareI64Zero,
            constraint: key,
            operands: vec![SelectedOperand {
                operand: 0,
                virtual_register: VirtualRegisterId(2),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            }],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance::default(),
        });
    assert_eq!(reject_unsupported_constraints(0, &ordinary_reuse), Ok(()));

    let mut no_unrelated = supported_isolated_tied_early_clobber_function();
    no_unrelated.blocks[0].instructions[0].operands.remove(1);
    assert!(matches!(
        reject_unsupported_constraints(0, &no_unrelated),
        Err(LivenessError::UnsupportedEarlyClobber { .. })
    ));

    let mut tied_unrelated = supported_isolated_tied_early_clobber_function();
    tied_unrelated.blocks[0].instructions[0].operands[1].tied_to = Some(0);
    assert!(matches!(
        reject_unsupported_constraints(0, &tied_unrelated),
        Err(LivenessError::UnsupportedEarlyClobber { .. })
    ));

    let mut extra_definition = supported_isolated_tied_early_clobber_function();
    extra_definition.blocks[0].instructions[0].operands[1].access = RegisterOperandAccess::Def;
    assert!(matches!(
        reject_unsupported_constraints(0, &extra_definition),
        Err(LivenessError::UnsupportedEarlyClobber { .. })
    ));

    let mut duplicate_composed = supported_isolated_tied_early_clobber_function();
    duplicate_composed.blocks[0].instructions[0].operands[1].virtual_register =
        VirtualRegisterId(0);
    assert!(matches!(
        reject_unsupported_constraints(0, &duplicate_composed),
        Err(LivenessError::UnsupportedEarlyClobber { .. })
    ));

    let mut nonisolated = supported_isolated_tied_early_clobber_function();
    let key = nonisolated.blocks[0].instructions[0].constraint;
    nonisolated.blocks[0]
        .instructions
        .push(SelectedInstruction {
            id: SelectedInstructionId(1),
            kind: SelectedInstructionKind::CompareI64Zero,
            constraint: key,
            operands: vec![
                SelectedOperand {
                    operand: 0,
                    virtual_register: VirtualRegisterId(2),
                    access: RegisterOperandAccess::Use,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                },
                SelectedOperand {
                    operand: 1,
                    virtual_register: VirtualRegisterId(3),
                    access: RegisterOperandAccess::Def,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: Some(0),
                    early_clobber: false,
                },
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance::default(),
        });
    assert!(matches!(
        reject_unsupported_constraints(0, &nonisolated),
        Ok(())
    ));

    let mut same_component_second_early = supported_component_tied_early_clobber_function();
    same_component_second_early.blocks[0].instructions[0].operands[1].early_clobber = true;
    same_component_second_early.blocks[0].instructions[0]
        .operands
        .push(SelectedOperand {
            operand: 2,
            virtual_register: VirtualRegisterId(4),
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
    assert!(matches!(
        reject_unsupported_constraints(0, &same_component_second_early),
        Err(LivenessError::UnsupportedEarlyClobber { .. })
    ));

    let mut tied = function_with_operand(RegisterOperandAccess::Use);
    tied.blocks[0].instructions[0].operands[0].tied_to = Some(0);
    assert!(matches!(
        reject_unsupported_constraints(0, &tied),
        Err(LivenessError::UnsupportedTiedOperand { .. })
    ));

    let mut early = function_with_operand(RegisterOperandAccess::Def);
    early.blocks[0].instructions[0].operands[0].early_clobber = true;
    assert!(matches!(
        reject_unsupported_constraints(0, &early),
        Err(LivenessError::UnsupportedEarlyClobber { .. })
    ));

    let mut duplicate = supported_early_clobber_function();
    duplicate.blocks[0].instructions[0].operands[1].virtual_register = VirtualRegisterId(0);
    assert!(matches!(
        reject_unsupported_constraints(0, &duplicate),
        Err(LivenessError::UnsupportedEarlyClobber { .. })
    ));

    let mut second_definition = supported_early_clobber_function();
    second_definition.blocks[0].instructions[0]
        .operands
        .push(SelectedOperand {
            operand: 2,
            virtual_register: VirtualRegisterId(2),
            access: RegisterOperandAccess::Def,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
    assert!(matches!(
        reject_unsupported_constraints(0, &second_definition),
        Err(LivenessError::UnsupportedEarlyClobber { .. })
    ));

    let mut tied_overlap = supported_early_clobber_function();
    let key = tied_overlap.blocks[0].instructions[0].constraint;
    tied_overlap.blocks[0]
        .instructions
        .push(SelectedInstruction {
            id: SelectedInstructionId(1),
            kind: SelectedInstructionKind::CompareI64Zero,
            constraint: key,
            operands: vec![
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
                    virtual_register: VirtualRegisterId(2),
                    access: RegisterOperandAccess::Def,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: Some(0),
                    early_clobber: false,
                },
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance::default(),
        });
    assert!(matches!(
        reject_unsupported_constraints(0, &tied_overlap),
        Err(LivenessError::UnsupportedEarlyClobber { .. })
    ));
}
