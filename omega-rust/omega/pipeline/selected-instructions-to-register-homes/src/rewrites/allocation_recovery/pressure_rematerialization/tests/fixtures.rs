use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
    RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, SelectedOperand, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::super::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, BlockPointDomain,
    FunctionLiveRanges, FunctionRecoveryClassification, LiveRangeFragment, LiveRangeIdentity,
    LiveRangePlan, LiveRangePoint, LivenessIdentity, LivenessPosition,
    PressureRecoveryClassification, RecoveryClassification, RecoveryClassificationPlan,
    RecoveryClassificationPolicy, RecoveryFutureUse, RecoveryVictimRole, SpillChoiceIdentity,
    VirtualLiveRange, VirtualOccurrence,
};

fn operand(register: u32, access: RegisterOperandAccess) -> SelectedOperand {
    SelectedOperand {
        operand: 0,
        virtual_register: VirtualRegisterId(register),
        access,
        class: RegisterClassId(0),
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    }
}

pub(crate) fn fixture() -> (
    SelectedInstructionPlan,
    LiveRangePlan,
    RecoveryClassificationPlan,
    RegisterInstructionConstraint,
) {
    let machine = MachineId::new(1).unwrap();
    let source_block = BlockId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let key = RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: 1,
    };
    let mut definitions = Vec::new();
    let mut registers = Vec::new();
    for register in 0..3_u32 {
        let source_value = ValueId::new(u64::from(register) + 1).unwrap();
        let operation = OperationId::new(u64::from(register) + 1).unwrap();
        definitions.push(SelectedInstruction {
            id: SelectedInstructionId(register),
            kind: SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(register) + 40),
            },
            constraint: key,
            operands: vec![operand(register, RegisterOperandAccess::Def)],
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
                    units: 2,
                }],
            },
        });
        registers.push(VirtualRegister {
            id: VirtualRegisterId(register),
            scalar_type: scalar,
            class: RegisterClassId(0),
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(register),
                source_value,
            },
            definition_site: ValueDefinitionSite::Node {
                block: source_block,
                node: register,
            },
            entry_fixed_view: None,
        });
    }
    let returned = SelectedInstruction {
        id: SelectedInstructionId(3),
        kind: SelectedInstructionKind::ReturnI64,
        constraint: key,
        operands: vec![operand(0, RegisterOperandAccess::Use)],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance {
            values: vec![ValueId::new(1).unwrap()],
            ..Default::default()
        },
    };
    let selected = SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target: target::NativeTarget::linux_x64(),
        entry: machine,
        functions: vec![SelectedFunction {
            machine,
            attachment: None,
            provenance: Default::default(),
            entry_block: SelectedBlockId(0),
            virtual_registers: registers,
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                source_block,
                instructions: definitions,
                terminator: SelectedTerminator::Return {
                    instruction: returned,
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        }],
        structural_unit_functions: Vec::new(),
        projected_structural_call_returns: Vec::new(),
    };
    let ranges = LiveRangePlan {
        selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
        liveness: LivenessIdentity::from_bytes([3; 32]),
        optimization_unit: OptimizationUnitIdentity::from_bytes([4; 32]),
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target: selected.target,
        functions: vec![FunctionLiveRanges {
            machine,
            block_domains: vec![BlockPointDomain {
                block: SelectedBlockId(0),
                source_block,
                start: LiveRangePoint(0),
                end: LiveRangePoint(8),
            }],
            virtual_registers: vec![VirtualLiveRange {
                virtual_register: VirtualRegisterId(0),
                class: RegisterClassId(0),
                occurrences: vec![
                    VirtualOccurrence {
                        position: LivenessPosition(0),
                        point: LiveRangePoint(1),
                        instruction: SelectedInstructionId(0),
                        operand: 0,
                        access: RegisterOperandAccess::Def,
                    },
                    VirtualOccurrence {
                        position: LivenessPosition(3),
                        point: LiveRangePoint(6),
                        instruction: SelectedInstructionId(3),
                        operand: 0,
                        access: RegisterOperandAccess::Use,
                    },
                ],
                fixed_constraints: Vec::new(),
                fragments: vec![LiveRangeFragment {
                    block: SelectedBlockId(0),
                    start: LiveRangePoint(1),
                    end: LiveRangePoint(7),
                }],
                edge_connectors: Vec::new(),
            }],
            tied_pairs: Vec::new(),
            early_clobbers: Vec::new(),
            architectural_units: Vec::new(),
            interference: Vec::new(),
        }],
        structural_unit_functions: Vec::new(),
    };
    let original = &selected.functions[0].blocks[0].instructions[0];
    let recovery = RecoveryClassificationPlan {
        selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
        spill_choices: SpillChoiceIdentity::from_bytes([5; 32]),
        ranges: LiveRangeIdentity::from_bytes([6; 32]),
        legality: AllocationLegalityIdentity::from_bytes([7; 32]),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([8; 32]),
        allocator_availability: AllocatorAvailabilityIdentity::from_bytes([9; 32]),
        optimization_unit: OptimizationUnitIdentity::from_bytes([4; 32]),
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        policy: RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        budget: OptimizationWorkBudget::new(10, 10, 30, 10, 1).unwrap(),
        usage: OptimizationWorkUsage {
            rule_evaluations: 1,
            candidates: 1,
            validation_steps: 1,
            commits: 1,
            iterations: 1,
        },
        functions: vec![FunctionRecoveryClassification {
            machine,
            classification: Some(PressureRecoveryClassification {
                block: SelectedBlockId(0),
                point: LiveRangePoint(5),
                victim: VirtualRegisterId(0),
                role: RecoveryVictimRole::ActiveResident {
                    current_view: RegisterViewId(0),
                    reclaimed_view: RegisterViewId(0),
                },
                scalar_type: scalar,
                class: RegisterClassId(0),
                origin: selected.functions[0].virtual_registers[0].origin,
                definition_site: selected.functions[0].virtual_registers[0].definition_site,
                classification: RecoveryClassification::ImmediateU64RematerializationCandidate {
                    defining_instruction: original.id,
                    source_value: ValueId::new(1).unwrap(),
                    value: IntegerValue::Unsigned(40),
                    provenance: original.provenance.clone(),
                    future_uses: vec![RecoveryFutureUse {
                        block: SelectedBlockId(0),
                        point: LiveRangePoint(6),
                        instruction: SelectedInstructionId(3),
                        operand: 0,
                    }],
                },
            }),
        }],
    };
    let row = RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key,
        operands: vec![RegisterOperandConstraint {
            operand: 0,
            access: RegisterOperandAccess::Def,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    };
    (selected, ranges, recovery, row)
}

pub(crate) fn multiple_future_fixture() -> (
    SelectedInstructionPlan,
    LiveRangePlan,
    RecoveryClassificationPlan,
    RegisterInstructionConstraint,
) {
    let (mut selected, mut ranges, mut recovery, row) = fixture();
    let block = &mut selected.functions[0].blocks[0];
    let SelectedTerminator::Return { instruction, .. } = &mut block.terminator else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(4);
    block.instructions.push(SelectedInstruction {
        id: SelectedInstructionId(3),
        kind: SelectedInstructionKind::CompareI64Zero,
        constraint: row.key,
        operands: vec![
            operand(0, RegisterOperandAccess::Use),
            SelectedOperand {
                operand: 1,
                virtual_register: VirtualRegisterId(1),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance {
            values: vec![ValueId::new(1).unwrap()],
            ..Default::default()
        },
    });
    let function_ranges = &mut ranges.functions[0];
    function_ranges.block_domains[0].end = LiveRangePoint(10);
    let victim = &mut function_ranges.virtual_registers[0];
    victim.occurrences[1] = VirtualOccurrence {
        position: LivenessPosition(3),
        point: LiveRangePoint(6),
        instruction: SelectedInstructionId(3),
        operand: 0,
        access: RegisterOperandAccess::Use,
    };
    victim.occurrences.push(VirtualOccurrence {
        position: LivenessPosition(4),
        point: LiveRangePoint(8),
        instruction: SelectedInstructionId(4),
        operand: 0,
        access: RegisterOperandAccess::Use,
    });
    victim.fragments[0].end = LiveRangePoint(9);
    let Some(PressureRecoveryClassification {
        classification:
            RecoveryClassification::ImmediateU64RematerializationCandidate { future_uses, .. },
        ..
    }) = recovery.functions[0].classification.as_mut()
    else {
        unreachable!()
    };
    future_uses.push(RecoveryFutureUse {
        block: SelectedBlockId(0),
        point: LiveRangePoint(8),
        instruction: SelectedInstructionId(4),
        operand: 0,
    });
    (selected, ranges, recovery, row)
}

pub(super) fn same_instruction_multiple_future_fixture() -> (
    SelectedInstructionPlan,
    LiveRangePlan,
    RecoveryClassificationPlan,
    RegisterInstructionConstraint,
) {
    let (mut selected, mut ranges, mut recovery, row) = multiple_future_fixture();
    selected.functions[0].blocks[0].instructions[3].operands[1].virtual_register =
        VirtualRegisterId(0);
    let SelectedTerminator::Return { instruction, .. } =
        &mut selected.functions[0].blocks[0].terminator
    else {
        unreachable!()
    };
    instruction.operands[0].virtual_register = VirtualRegisterId(1);
    let victim = &mut ranges.functions[0].virtual_registers[0];
    victim.occurrences[2] = VirtualOccurrence {
        position: LivenessPosition(3),
        point: LiveRangePoint(6),
        instruction: SelectedInstructionId(3),
        operand: 1,
        access: RegisterOperandAccess::Use,
    };
    victim.fragments[0].end = LiveRangePoint(7);
    let Some(PressureRecoveryClassification {
        classification:
            RecoveryClassification::ImmediateU64RematerializationCandidate { future_uses, .. },
        ..
    }) = recovery.functions[0].classification.as_mut()
    else {
        unreachable!()
    };
    future_uses[1] = RecoveryFutureUse {
        block: SelectedBlockId(0),
        point: LiveRangePoint(6),
        instruction: SelectedInstructionId(3),
        operand: 1,
    };
    (selected, ranges, recovery, row)
}
