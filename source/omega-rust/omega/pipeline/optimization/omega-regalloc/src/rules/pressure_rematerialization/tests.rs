//! Focused pressure-rematerialization production and budget tests.

use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use omega_register_model::{
    PhysicalRegisterModel, RegisterClass, RegisterClassId, RegisterConstraintFamily,
    RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterOperandConstraint, RegisterUnit, RegisterUnitId,
    RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
    TargetRegisterEnvironmentIdentity, validate_physical_register_model,
};
use omega_selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, SelectedOperand, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::compute::{build_functions, ensure_budget, required_usage};
use super::*;

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
        target: omega_target::NativeTarget::linux_x64(),
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

fn same_instruction_multiple_future_fixture() -> (
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

#[test]
fn active_resident_is_split_before_sole_future_use_and_reanalyzes() {
    let (selected, ranges, recovery, row) = fixture();
    let original = selected.functions[0].blocks[0].instructions[0].clone();
    let (functions, transformed) = build_functions(
        &selected,
        &ranges,
        &recovery,
        &row,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
    )
    .unwrap();
    let action = functions[0].action.as_ref().unwrap();
    assert_eq!(action.fresh_materialize, SelectedInstructionId(4));
    assert_eq!(action.result_virtual_register, VirtualRegisterId(3));
    let function = &transformed.functions[0];
    let transformed_machine = function.machine;
    assert_eq!(function.blocks[0].instructions[0], original);
    let inserted = function.blocks[0].instructions.last().unwrap();
    assert_eq!(inserted.id, SelectedInstructionId(4));
    assert_eq!(
        inserted.kind,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(40)
        }
    );
    assert!(inserted.provenance.operations.is_empty());
    assert_eq!(inserted.provenance.values, vec![ValueId::new(1).unwrap()]);
    assert!(inserted.provenance.edges.is_empty());
    assert!(inserted.provenance.obligations.is_empty());
    assert!(inserted.provenance.fuel.is_empty());
    assert_eq!(original.provenance.fuel.len(), 1);
    let returned = match &function.blocks[0].terminator {
        SelectedTerminator::Return { instruction, .. } => instruction,
        _ => unreachable!(),
    };
    assert_eq!(returned.operands[0].virtual_register, VirtualRegisterId(3));
    assert_eq!(
        function.virtual_registers[3].origin,
        VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(4),
            source_value: ValueId::new(1).unwrap()
        }
    );

    let transformed_identity = selected_instruction_plan_identity(&transformed);
    let optimization_unit = OptimizationUnitIdentity::from_bytes([4; 32]);
    let plan = PressureRematerializationPlan {
        source_selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
        spill_choices: SpillChoiceIdentity::from_bytes([5; 32]),
        recovery_classifications: RecoveryClassificationIdentity::from_bytes([10; 32]),
        ranges: LiveRangeIdentity::from_bytes([6; 32]),
        legality: AllocationLegalityIdentity::from_bytes([7; 32]),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([8; 32]),
        allocator_availability: AllocatorAvailabilityIdentity::from_bytes([9; 32]),
        optimization_unit, fuel_schedule: transformed.fuel_schedule,
        policy: PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
        budget: OptimizationWorkBudget::new(10, 10, 30, 10, 1).unwrap(),
        usage: OptimizationWorkUsage { rule_evaluations: 1, candidates: 1, validation_steps: 8, commits: 1, iterations: 1 },
        functions, transformed_selected: transformed_identity,
    };
    let receipt = PressureRematerializationValidationReceipt {
        identity: pressure_rematerialization_identity(&plan),
        source_selected: plan.source_selected,
        spill_choices: plan.spill_choices,
        recovery_classifications: plan.recovery_classifications,
        ranges: plan.ranges,
        legality: plan.legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        transformed_selected: transformed_identity,
        policy: plan.policy,
        usage: plan.usage,
        function_count: 1,
        applied_count: 1,
        rewritten_use_count: 1,
    };
    let validated = ValidatedPressureRematerialization {
        plan,
        transformed,
        receipt,
    };
    let liveness = analyze_liveness(&validated).unwrap();
    let post_ranges = analyze_live_ranges(&validated, &liveness).unwrap();
    assert_eq!(post_ranges.receipt().virtual_register_count(), 4);
    assert!(
        !post_ranges.plan().functions[0]
            .interference
            .iter()
            .any(|edge| edge.lower == VirtualRegisterId(0) && edge.higher == VirtualRegisterId(2))
    );

    let physical = validate_physical_register_model(PhysicalRegisterModel {
        architecture: omega_target::Architecture::X86_64,
        units: (0..2)
            .map(|id| RegisterUnit {
                id: RegisterUnitId(id),
                name: format!("r{id}.storage"),
                bits: 64,
                kind: RegisterUnitKind::IntegerLane,
            })
            .collect(),
        views: (0..2)
            .map(|id| RegisterView {
                id: RegisterViewId(id),
                name: format!("r{id}"),
                class: RegisterClassId(0),
                units: vec![RegisterUnitId(id)],
                write_units: vec![RegisterUnitId(id)],
                bits: 64,
                write_semantics: RegisterWriteSemantics::ExactView,
                allocatable: true,
            })
            .collect(),
        classes: vec![RegisterClass {
            id: RegisterClassId(0),
            name: "integer".into(),
            views: vec![RegisterViewId(0), RegisterViewId(1)],
        }],
        conventions: Vec::new(),
        reservations: Vec::new(),
    })
    .unwrap();
    let legality = FunctionAllocationLegality {
        machine: transformed_machine,
        virtual_registers: post_ranges.plan().functions[0]
            .virtual_registers
            .iter()
            .map(|range| VirtualRegisterAllocationLegality {
                virtual_register: range.virtual_register,
                class: range.class,
                points: range
                    .fragments
                    .iter()
                    .flat_map(|fragment| fragment.start.0..fragment.end.0)
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
    let homes = crate::allocation::home_assignment::compute::compute_function(
        0,
        &legality,
        &post_ranges.plan().functions[0],
        &physical,
    )
    .unwrap();
    let replayed_homes = crate::allocation::home_assignment::validate::replay_function(
        0,
        &legality,
        &post_ranges.plan().functions[0],
        &physical,
    )
    .unwrap();
    assert_eq!(homes, replayed_homes);
    assert_eq!(homes.assignments.len(), 4);
}

#[test]
fn active_resident_is_split_once_before_a_multiple_use_suffix_and_reanalyzes() {
    let (selected, ranges, recovery, row) = multiple_future_fixture();
    assert!(matches!(
        build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
        ),
        Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
    ));
    let (functions, transformed) = build_functions(
        &selected,
        &ranges,
        &recovery,
        &row,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
    )
    .unwrap();
    let action = functions[0].action.as_ref().unwrap();
    assert_eq!(action.rewrites.len(), 2);
    assert_eq!(action.rewrites[0].instruction, SelectedInstructionId(3));
    assert_eq!(action.rewrites[1].instruction, SelectedInstructionId(4));
    assert_eq!(action.fresh_materialize, SelectedInstructionId(5));
    assert_eq!(action.result_virtual_register, VirtualRegisterId(3));

    let source_original = &selected.functions[0].blocks[0].instructions[0];
    let transformed_function = &transformed.functions[0];
    let transformed_machine = transformed_function.machine;
    assert_eq!(
        transformed_function.blocks[0].instructions[0],
        *source_original
    );
    assert_eq!(source_original.provenance.fuel.len(), 1);
    let inserted = &transformed_function.blocks[0].instructions[3];
    assert_eq!(inserted.id, SelectedInstructionId(5));
    assert_eq!(inserted.provenance.values, vec![ValueId::new(1).unwrap()]);
    assert!(inserted.provenance.operations.is_empty());
    assert!(inserted.provenance.fuel.is_empty());
    assert_eq!(
        transformed_function.blocks[0].instructions[4].operands[0].virtual_register,
        VirtualRegisterId(3)
    );
    assert_eq!(
        transformed_function.blocks[0].instructions[4].operands[1].virtual_register,
        VirtualRegisterId(1)
    );
    let returned = match &transformed_function.blocks[0].terminator {
        SelectedTerminator::Return { instruction, .. } => instruction,
        _ => unreachable!(),
    };
    assert_eq!(returned.operands[0].virtual_register, VirtualRegisterId(3));

    let transformed_identity = selected_instruction_plan_identity(&transformed);
    let plan = PressureRematerializationPlan {
        source_selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
        spill_choices: SpillChoiceIdentity::from_bytes([5; 32]),
        recovery_classifications: RecoveryClassificationIdentity::from_bytes([10; 32]),
        ranges: LiveRangeIdentity::from_bytes([6; 32]),
        legality: AllocationLegalityIdentity::from_bytes([7; 32]),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([8; 32]),
        allocator_availability: AllocatorAvailabilityIdentity::from_bytes([9; 32]),
        optimization_unit: OptimizationUnitIdentity::from_bytes([4; 32]),
        fuel_schedule: transformed.fuel_schedule,
        policy: PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        budget: OptimizationWorkBudget::new(10, 10, 30, 10, 1).unwrap(),
        usage: OptimizationWorkUsage { rule_evaluations: 1, candidates: 1, validation_steps: 10, commits: 1, iterations: 1 },
        functions,
        transformed_selected: transformed_identity,
    };
    let receipt = PressureRematerializationValidationReceipt {
        identity: pressure_rematerialization_identity(&plan),
        source_selected: plan.source_selected,
        spill_choices: plan.spill_choices,
        recovery_classifications: plan.recovery_classifications,
        ranges: plan.ranges,
        legality: plan.legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        transformed_selected: transformed_identity,
        policy: plan.policy,
        usage: plan.usage,
        function_count: 1,
        applied_count: 1,
        rewritten_use_count: 2,
    };
    let validated = ValidatedPressureRematerialization {
        plan,
        transformed,
        receipt,
    };
    assert_eq!(validated.receipt().rewritten_use_count(), 2);
    let liveness = analyze_liveness(&validated).unwrap();
    let post_ranges = analyze_live_ranges(&validated, &liveness).unwrap();
    let victim = &post_ranges.plan().functions[0].virtual_registers[0];
    assert!(
        victim
            .fragments
            .iter()
            .all(|fragment| fragment.end <= LiveRangePoint(5))
    );
    let suffix = &post_ranges.plan().functions[0].virtual_registers[3];
    assert_eq!(suffix.occurrences.len(), 3);

    let physical = validate_physical_register_model(PhysicalRegisterModel {
        architecture: omega_target::Architecture::X86_64,
        units: (0..2)
            .map(|id| RegisterUnit {
                id: RegisterUnitId(id),
                name: format!("r{id}.storage"),
                bits: 64,
                kind: RegisterUnitKind::IntegerLane,
            })
            .collect(),
        views: (0..2)
            .map(|id| RegisterView {
                id: RegisterViewId(id),
                name: format!("r{id}"),
                class: RegisterClassId(0),
                units: vec![RegisterUnitId(id)],
                write_units: vec![RegisterUnitId(id)],
                bits: 64,
                write_semantics: RegisterWriteSemantics::ExactView,
                allocatable: true,
            })
            .collect(),
        classes: vec![RegisterClass {
            id: RegisterClassId(0),
            name: "integer".into(),
            views: vec![RegisterViewId(0), RegisterViewId(1)],
        }],
        conventions: Vec::new(),
        reservations: Vec::new(),
    })
    .unwrap();
    let legality = FunctionAllocationLegality {
        machine: transformed_machine,
        virtual_registers: post_ranges.plan().functions[0]
            .virtual_registers
            .iter()
            .map(|range| VirtualRegisterAllocationLegality {
                virtual_register: range.virtual_register,
                class: range.class,
                points: range
                    .fragments
                    .iter()
                    .flat_map(|fragment| fragment.start.0..fragment.end.0)
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
    let homes = crate::allocation::home_assignment::compute::compute_function(
        0,
        &legality,
        &post_ranges.plan().functions[0],
        &physical,
    )
    .unwrap();
    assert_eq!(
        homes,
        crate::allocation::home_assignment::validate::replay_function(
            0,
            &legality,
            &post_ranges.plan().functions[0],
            &physical,
        )
        .unwrap()
    );
    assert_eq!(homes.assignments.len(), 4);
}

#[test]
fn multiple_use_policy_rejects_noncanonical_or_single_rewrite_evidence() {
    let (single_selected, single_ranges, single_recovery, row) = fixture();
    assert!(matches!(
        build_functions(
            &single_selected,
            &single_ranges,
            &single_recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        ),
        Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
    ));

    let (selected, ranges, mut recovery, row) = multiple_future_fixture();
    {
        let Some(PressureRecoveryClassification {
            classification:
                RecoveryClassification::ImmediateU64RematerializationCandidate { future_uses, .. },
            ..
        }) = recovery.functions[0].classification.as_mut()
        else {
            unreachable!()
        };
        future_uses.swap(0, 1);
    }
    assert!(matches!(
        build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        ),
        Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
    ));
    {
        let Some(PressureRecoveryClassification {
            classification:
                RecoveryClassification::ImmediateU64RematerializationCandidate { future_uses, .. },
            ..
        }) = recovery.functions[0].classification.as_mut()
        else {
            unreachable!()
        };
        future_uses.swap(0, 1);
        future_uses[1] = future_uses[0];
    }
    assert!(matches!(
        build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        ),
        Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
    ));

    let (selected, ranges, recovery, row) = same_instruction_multiple_future_fixture();
    let usage = required_usage(&selected, 1, 2).unwrap();
    assert_eq!(usage.validation_steps, 10);
    let insufficient = OptimizationWorkBudget::new(
        usage.rule_evaluations,
        usage.candidates,
        usage.validation_steps - 1,
        usage.commits,
        usage.iterations,
    )
    .unwrap();
    assert_eq!(
        ensure_budget(usage, insufficient),
        Err(PressureRematerializationError::BudgetExceeded {
            required: usage,
            budget: insufficient,
        })
    );
    let (functions, transformed) = build_functions(
        &selected,
        &ranges,
        &recovery,
        &row,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
    )
    .unwrap();
    assert_eq!(functions[0].action.as_ref().unwrap().rewrites.len(), 2);
    assert_eq!(
        transformed.functions[0].blocks[0].instructions[4]
            .operands
            .iter()
            .map(|operand| operand.virtual_register)
            .collect::<Vec<_>>(),
        vec![VirtualRegisterId(3), VirtualRegisterId(3)]
    );

    let (mut selected, ranges, recovery, row) = multiple_future_fixture();
    selected.functions[0].blocks[0].instructions[3].operands[0].fixed_view =
        Some(RegisterViewId(0));
    assert!(matches!(
        build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        ),
        Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
    ));
}
