//! Focused fixed-view-copy computation fixtures.

use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterOperandConstraint, RegisterViewId,
};
use omega_selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedSuccessor,
};
use psi_core::{BlockId, EdgeId, IntegerType, MachineId, ValueId};

use super::*;
use crate::{
    EntryFixedViewTransition, FixedPrecoloredHomeDomainId, FixedPrecoloredSourceSegmentId,
    FunctionAllocationLegality, LiveRangeEdgeConnector, LiveRangePoint, LivenessPosition,
    VirtualRegisterAllocationLegality,
};

fn key(variant: u32) -> RegisterConstraintKey {
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant,
    }
}

fn instruction(
    id: u32,
    kind: SelectedInstructionKind,
    operands: Vec<SelectedOperand>,
) -> SelectedInstruction {
    SelectedInstruction {
        id: SelectedInstructionId(id),
        kind,
        constraint: key(id),
        operands,
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    }
}

fn use_operand(
    register: u32,
    class: RegisterClassId,
    view: Option<RegisterViewId>,
) -> SelectedOperand {
    SelectedOperand {
        operand: 0,
        virtual_register: VirtualRegisterId(register),
        access: RegisterOperandAccess::Use,
        class,
        fixed_view: view,
        tied_to: None,
        early_clobber: false,
    }
}

pub(crate) fn fixture() -> (
    SelectedFunction,
    FunctionAllocationLegality,
    RegisterInstructionConstraint,
) {
    let machine = MachineId::new(1).unwrap();
    let class = RegisterClassId(0);
    let from = RegisterViewId(1);
    let to = RegisterViewId(2);
    let source_value = ValueId::new(2).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let compare = instruction(
        0,
        SelectedInstructionKind::CompareI64Zero,
        vec![use_operand(0, class, None)],
    );
    let branch = instruction(
        1,
        SelectedInstructionKind::ConditionalBranchNonZero,
        Vec::new(),
    );
    let return_a = instruction(
        2,
        SelectedInstructionKind::ReturnI64,
        vec![use_operand(1, class, Some(to))],
    );
    let return_b = instruction(
        3,
        SelectedInstructionKind::ReturnI64,
        vec![use_operand(1, class, Some(to))],
    );
    let function = SelectedFunction {
        machine,
        attachment: None,
        provenance: Default::default(),
        entry_block: SelectedBlockId(0),
        virtual_registers: vec![
            VirtualRegister {
                id: VirtualRegisterId(0),
                scalar_type: scalar,
                class,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                },
                definition_site: ValueDefinitionSite::FunctionParameter(0),
                entry_fixed_view: None,
            },
            VirtualRegister {
                id: VirtualRegisterId(1),
                scalar_type: scalar,
                class,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value,
                    parameter_index: 1,
                },
                definition_site: ValueDefinitionSite::FunctionParameter(1),
                entry_fixed_view: Some(from),
            },
        ],
        blocks: vec![
            SelectedBlock {
                id: SelectedBlockId(0),
                source_block: BlockId::new(1).unwrap(),
                instructions: vec![compare],
                terminator: SelectedTerminator::ConditionalBranch {
                    instruction: branch,
                    when_nonzero: SelectedSuccessor {
                        psi_edge: EdgeId::new(1).unwrap(),
                        block: SelectedBlockId(1),
                        source_target: BlockId::new(2).unwrap(),
                        bindings: Vec::new(),
                        fuel: Vec::new(),
                    },
                    when_zero: SelectedSuccessor {
                        psi_edge: EdgeId::new(2).unwrap(),
                        block: SelectedBlockId(2),
                        source_target: BlockId::new(3).unwrap(),
                        bindings: Vec::new(),
                        fuel: Vec::new(),
                    },
                },
            },
            SelectedBlock {
                id: SelectedBlockId(1),
                source_block: BlockId::new(2).unwrap(),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Return {
                    instruction: return_a,
                    psi_return_edge: EdgeId::new(3).unwrap(),
                },
            },
            SelectedBlock {
                id: SelectedBlockId(2),
                source_block: BlockId::new(3).unwrap(),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Return {
                    instruction: return_b,
                    psi_return_edge: EdgeId::new(4).unwrap(),
                },
            },
        ],
    };
    let site = |instruction| VirtualFixedConstraintSite::Operand {
        position: LivenessPosition(instruction),
        point: LiveRangePoint(instruction),
        instruction: SelectedInstructionId(instruction),
        operand: 0,
        access: RegisterOperandAccess::Use,
    };
    let legality = FunctionAllocationLegality {
        machine,
        virtual_registers: vec![
            VirtualRegisterAllocationLegality {
                virtual_register: VirtualRegisterId(0),
                class,
                points: Vec::new(),
                early_clobber_points: Vec::new(),
                entry_transitions: Vec::new(),
            },
            VirtualRegisterAllocationLegality {
                virtual_register: VirtualRegisterId(1),
                class,
                points: Vec::new(),
                early_clobber_points: Vec::new(),
                entry_transitions: vec![
                    EntryFixedViewTransition {
                        from_view: from,
                        to_site: site(2),
                        to_view: to,
                    },
                    EntryFixedViewTransition {
                        from_view: from,
                        to_site: site(3),
                        to_view: to,
                    },
                ],
            },
        ],
    };
    let row = RegisterInstructionConstraint {
        id: RegisterConstraintId(9),
        key: key(9),
        operands: vec![
            RegisterOperandConstraint {
                operand: 0,
                access: RegisterOperandAccess::Use,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            RegisterOperandConstraint {
                operand: 1,
                access: RegisterOperandAccess::Def,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    };
    (function, legality, row)
}

pub(crate) fn computed_shared_fixture() -> (
    SelectedFunction,
    FunctionAllocationLegality,
    RegisterInstructionConstraint,
    FixedViewCopy,
    SelectedFunction,
) {
    let (function, legality, row) = fixture();
    let boundaries = boundaries(&legality);
    let references = boundaries.iter().collect::<Vec<_>>();
    let copy = build_shared_entry_copy(0, &function, &references, &row, row.key, 4, 2)
        .unwrap()
        .unwrap();
    let mut transformed = function.clone();
    apply_copy(0, &mut transformed, &copy, &row).unwrap();
    (function, legality, row, copy, transformed)
}

fn boundaries(
    legality: &FunctionAllocationLegality,
) -> Vec<super::super::evidence::AuthenticatedFixedViewBoundary> {
    legality.virtual_registers[1]
        .entry_transitions
        .iter()
        .enumerate()
        .map(|(index, transition)| {
            let block = SelectedBlockId(u32::try_from(index + 1).unwrap());
            super::super::evidence::AuthenticatedFixedViewBoundary {
                function: 0,
                machine: legality.machine,
                virtual_register: VirtualRegisterId(1),
                class: legality.virtual_registers[1].class,
                source_segment: FixedPrecoloredSourceSegmentId(0),
                source_domain: FixedPrecoloredHomeDomainId(0),
                from_view: transition.from_view,
                destination_segment: FixedPrecoloredSourceSegmentId(
                    u32::try_from(index + 1).unwrap(),
                ),
                destination_domain: FixedPrecoloredHomeDomainId(u32::try_from(index + 1).unwrap()),
                site: transition.to_site,
                block,
                to_view: transition.to_view,
                incoming: Some(LiveRangeEdgeConnector {
                    source: SelectedBlockId(0),
                    terminator: SelectedInstructionId(1),
                    polarity_ordinal: u8::try_from(index).unwrap(),
                    psi_edge: EdgeId::new(u64::try_from(index + 1).unwrap()).unwrap(),
                    target: block,
                }),
            }
        })
        .collect()
}

#[test]
fn shared_entry_policy_inserts_one_copy_after_compare_and_rewrites_both_returns() {
    let (_, _, _, copy, transformed) = computed_shared_fixture();
    assert_eq!(copy.insertion_block, SelectedBlockId(0));
    assert_eq!(copy.before_instruction, SelectedInstructionId(1));
    assert_eq!(copy.destinations.len(), 2);
    assert_eq!(transformed.blocks[0].instructions.len(), 2);
    assert_eq!(
        transformed.blocks[0].instructions[1].kind,
        SelectedInstructionKind::CopyI64
    );
    for leaf in &transformed.blocks[1..] {
        let SelectedTerminator::Return { instruction, .. } = &leaf.terminator else {
            panic!()
        };
        assert_eq!(
            instruction.operands[0].virtual_register,
            VirtualRegisterId(2)
        );
        assert!(leaf.instructions.is_empty());
    }
}

#[test]
fn shared_entry_policy_rejects_noncanonical_compare_copy_branch_shape() {
    let (mut function, legality, row) = fixture();
    function.blocks[0].instructions.push(instruction(
        4,
        SelectedInstructionKind::CopyI64,
        Vec::new(),
    ));
    let boundaries = boundaries(&legality);
    let references = boundaries.iter().collect::<Vec<_>>();
    assert!(matches!(
        build_shared_entry_copy(0, &function, &references, &row, row.key, 5, 2),
        Err(FixedViewCopyError::UnsupportedSharedTransitionSet { function: 0 })
    ));
}
