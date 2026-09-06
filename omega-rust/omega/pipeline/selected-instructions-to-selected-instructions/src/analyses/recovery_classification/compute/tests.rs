use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
    RegisterViewId,
};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedOperand, SelectedTerminator,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ScalarType,
    ValueId,
};

use super::function_classification::classify;
use crate::{
    BlockPointDomain, FunctionAllocationLegality, FunctionLiveRanges, FunctionSpillChoices,
    LiveRangeFragment, LiveRangePoint, LivenessPosition, NoAdmittedRecoveryReason,
    PressureContender, RecoveryClassification, RecoveryClassificationError, RecoveryVictimRole,
    SpillChoice, VirtualLiveRange, VirtualOccurrence, VirtualPointLegality,
    VirtualRegisterAllocationLegality,
};

fn operand(register: u32, operand: u16, access: RegisterOperandAccess) -> SelectedOperand {
    SelectedOperand {
        operand,
        virtual_register: VirtualRegisterId(register),
        access,
        class: RegisterClassId(0),
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    }
}

fn fixture() -> (
    SelectedFunction,
    FunctionLiveRanges,
    FunctionAllocationLegality,
    FunctionSpillChoices,
) {
    let machine = MachineId::new(1).unwrap();
    let source_block = BlockId::new(1).unwrap();
    let key = RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: 1,
    };
    let definitions = (0..3_u32)
        .map(|register| {
            let operation = OperationId::new(u64::from(register) + 1).unwrap();
            let source_value = ValueId::new(u64::from(register) + 1).unwrap();
            SelectedInstruction {
                id: SelectedInstructionId(register),
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(u128::from(register) + 7),
                },
                constraint: key,
                operands: vec![operand(register, 0, RegisterOperandAccess::Def)],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance {
                    operations: vec![operation],
                    values: vec![source_value],
                    edges: Vec::new(),
                    obligations: Vec::new(),
                    fuel: vec![FuelSettlement {
                        site: PsiProvenance::Operation(operation),
                        units: 1,
                    }],
                },
            }
        })
        .collect::<Vec<_>>();
    let returned = SelectedInstruction {
        id: SelectedInstructionId(3),
        kind: SelectedInstructionKind::ReturnI64,
        constraint: key,
        operands: (0..3_u32)
            .map(|register| operand(register, register as u16, RegisterOperandAccess::Use))
            .collect(),
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance {
            values: (1..=3).map(|id| ValueId::new(id).unwrap()).collect(),
            ..Default::default()
        },
    };
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let selected = SelectedFunction {
        machine,
        attachment: None,
        provenance: Default::default(),
        entry_block: SelectedBlockId(0),
        virtual_registers: (0..3_u32)
            .map(|register| VirtualRegister {
                id: VirtualRegisterId(register),
                scalar_type,
                class: RegisterClassId(0),
                origin: VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(register),
                    source_value: ValueId::new(u64::from(register) + 1).unwrap(),
                },
                definition_site: ValueDefinitionSite::Node {
                    block: source_block,
                    node: register,
                },
                entry_fixed_view: None,
            })
            .collect(),
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            source_block,
            instructions: definitions,
            terminator: SelectedTerminator::Return {
                instruction: returned,
                psi_return_edge: EdgeId::new(1).unwrap(),
            },
        }],
    };
    let ranges = FunctionLiveRanges {
        machine,
        block_domains: vec![BlockPointDomain {
            block: SelectedBlockId(0),
            source_block,
            start: LiveRangePoint(0),
            end: LiveRangePoint(8),
        }],
        virtual_registers: (0..3_u32)
            .map(|register| VirtualLiveRange {
                virtual_register: VirtualRegisterId(register),
                class: RegisterClassId(0),
                occurrences: vec![
                    VirtualOccurrence {
                        position: LivenessPosition(register),
                        point: LiveRangePoint(register * 2 + 1),
                        instruction: SelectedInstructionId(register),
                        operand: 0,
                        access: RegisterOperandAccess::Def,
                    },
                    VirtualOccurrence {
                        position: LivenessPosition(3),
                        point: LiveRangePoint(6),
                        instruction: SelectedInstructionId(3),
                        operand: register as u16,
                        access: RegisterOperandAccess::Use,
                    },
                ],
                fixed_constraints: Vec::new(),
                fragments: vec![LiveRangeFragment {
                    block: SelectedBlockId(0),
                    start: LiveRangePoint(register * 2 + 1),
                    end: LiveRangePoint(7),
                }],
                edge_connectors: Vec::new(),
            })
            .collect(),
        edge_transfers: Vec::new(),
        tied_pairs: Vec::new(),
        early_clobbers: Vec::new(),
        architectural_units: Vec::new(),
        interference: [(0, 1), (0, 2), (1, 2)]
            .into_iter()
            .map(|(lower, higher)| crate::VirtualInterference {
                lower: VirtualRegisterId(lower),
                higher: VirtualRegisterId(higher),
            })
            .collect(),
    };
    let legality = FunctionAllocationLegality {
        machine,
        virtual_registers: (0..3_u32)
            .map(|register| VirtualRegisterAllocationLegality {
                virtual_register: VirtualRegisterId(register),
                class: RegisterClassId(0),
                points: (register * 2 + 1..=6)
                    .map(|point| VirtualPointLegality {
                        block: SelectedBlockId(0),
                        point: LiveRangePoint(point),
                        candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                    })
                    .collect(),
                early_clobber_points: Vec::new(),
                entry_transitions: Vec::new(),
            })
            .collect(),
    };
    let choices = FunctionSpillChoices {
        machine,
        choice: Some(SpillChoice {
            block: SelectedBlockId(0),
            point: LiveRangePoint(5),
            incoming: VirtualRegisterId(2),
            incoming_class: RegisterClassId(0),
            incoming_common_candidates: vec![RegisterViewId(0), RegisterViewId(1)],
            active_residents: vec![
                crate::PressureResident {
                    virtual_register: VirtualRegisterId(0),
                    class: RegisterClassId(0),
                    start: LiveRangePoint(1),
                    exclusive_end: LiveRangePoint(7),
                    view: RegisterViewId(0),
                },
                crate::PressureResident {
                    virtual_register: VirtualRegisterId(1),
                    class: RegisterClassId(0),
                    start: LiveRangePoint(3),
                    exclusive_end: LiveRangePoint(7),
                    view: RegisterViewId(1),
                },
            ],
            contenders: vec![
                PressureContender {
                    virtual_register: VirtualRegisterId(0),
                    exclusive_end: LiveRangePoint(7),
                    reclaimed_view: Some(RegisterViewId(0)),
                },
                PressureContender {
                    virtual_register: VirtualRegisterId(1),
                    exclusive_end: LiveRangePoint(7),
                    reclaimed_view: Some(RegisterViewId(1)),
                },
                PressureContender {
                    virtual_register: VirtualRegisterId(2),
                    exclusive_end: LiveRangePoint(7),
                    reclaimed_view: None,
                },
            ],
            selected_victim: VirtualRegisterId(2),
        }),
    };
    (selected, ranges, legality, choices)
}

#[test]
fn incoming_literal_is_classified_identically_by_compute_and_replay() {
    let (selected, ranges, legality, choices) = fixture();
    let computed = classify(0, &selected, &ranges, &legality, &choices).unwrap();
    let replayed = crate::analyses::recovery_classification::validate::replay_function_for_test(
        0, &selected, &ranges, &legality, &choices,
    )
    .unwrap();
    assert_eq!(computed, replayed);
    let row = computed.classification.unwrap();
    assert_eq!(row.role, RecoveryVictimRole::Incoming);
    assert!(matches!(
        row.classification,
        RecoveryClassification::ImmediateU64RematerializationCandidate {
            value: IntegerValue::Unsigned(9),
            ref future_uses,
            ..
        } if future_uses.len() == 1
    ));
}

#[test]
fn honest_unsupported_and_corrupt_provenance_are_distinct() {
    let (mut selected, ranges, legality, choices) = fixture();
    selected.virtual_registers[2].scalar_type = ScalarType::Boolean;
    let result = classify(0, &selected, &ranges, &legality, &choices).unwrap();
    assert!(matches!(
        result.classification.unwrap().classification,
        RecoveryClassification::NoAdmittedRecovery {
            reason: NoAdmittedRecoveryReason::UnsupportedScalarType
        }
    ));

    let (mut selected, ranges, legality, choices) = fixture();
    selected.blocks[0].instructions[2].provenance.values[0] = ValueId::new(99).unwrap();
    let expected = Err(RecoveryClassificationError::VictimMismatch {
        function: 0,
        register: 2,
    });
    assert_eq!(
        classify(0, &selected, &ranges, &legality, &choices),
        expected
    );
    assert_eq!(
        crate::analyses::recovery_classification::validate::replay_function_for_test(
            0, &selected, &ranges, &legality, &choices,
        ),
        expected
    );
}
