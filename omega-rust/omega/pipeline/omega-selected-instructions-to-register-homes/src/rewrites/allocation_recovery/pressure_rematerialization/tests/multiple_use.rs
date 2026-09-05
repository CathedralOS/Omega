use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{
    PhysicalRegisterModel, RegisterClass, RegisterClassId, RegisterUnit, RegisterUnitId,
    RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
    TargetRegisterEnvironmentIdentity, validate_physical_register_model,
};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, SelectedTerminator,
    VirtualRegisterId,
};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;
use psi_core::ValueId;

use super::super::compute::build_functions;
use super::super::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FunctionAllocationLegality,
    LiveRangeIdentity, LiveRangePoint, PressureRematerializationError,
    PressureRematerializationPlan, PressureRematerializationPolicy,
    PressureRematerializationValidationReceipt, RecoveryClassificationIdentity,
    SpillChoiceIdentity, ValidatedPressureRematerialization, VirtualPointLegality,
    VirtualRegisterAllocationLegality, analyze_live_ranges, analyze_liveness,
    pressure_rematerialization_identity,
};
use super::fixtures::multiple_future_fixture;

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
        transformed: transformed.into(),
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
    let homes = crate::assignment::home_assignment::compute::compute_function(
        0,
        &legality,
        &post_ranges.plan().functions[0],
        &physical,
    )
    .unwrap();
    assert_eq!(
        homes,
        crate::assignment::home_assignment::validate::replay_function(
            0,
            &legality,
            &post_ranges.plan().functions[0],
            &physical,
        )
        .unwrap()
    );
    assert_eq!(homes.assignments.len(), 4);
}
