use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use optimization_unit::ValueDefinitionSite;
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ScalarType, ValueId,
};

use crate::*;

pub(super) fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(10, 10, 20, 10, 1).unwrap()
}

pub(super) fn source() -> ValidatedLogicalSpillOperations {
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let storage = LogicalSpillStorage {
        id: LogicalSpillStorageId(0),
        class: LogicalSpillStorageClass::NonAddressUnsignedU64V1,
    };
    validated_source(LogicalSpillOperationPlan {
        selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
        ranges: LiveRangeIdentity::from_bytes([2; 32]),
        legality: AllocationLegalityIdentity::from_bytes([3; 32]),
        spill_choices: SpillChoiceIdentity::from_bytes([4; 32]),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([5; 32]),
        allocator_availability: AllocatorAvailabilityIdentity::from_bytes([6; 32]),
        optimization_unit: OptimizationUnitIdentity::from_bytes([7; 32]),
        fuel_schedule: FuelScheduleIdentity::new(8).unwrap(),
        policy: LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
        budget: OptimizationWorkBudget::new(1, 1, 3, 1, 1).unwrap(),
        usage: OptimizationWorkUsage {
            rule_evaluations: 1,
            candidates: 1,
            validation_steps: 3,
            commits: 1,
            iterations: 1,
        },
        functions: vec![FunctionLogicalSpillOperations {
            machine: MachineId::new(9).unwrap(),
            action: Some(LogicalSpillAction {
                block: SelectedBlockId(0),
                pressure_point: LiveRangePoint(5),
                incoming: VirtualRegisterId(3),
                incoming_class: RegisterClassId(0),
                victim: VirtualRegisterId(1),
                victim_class: RegisterClassId(0),
                victim_scalar_type: scalar,
                victim_origin: VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(1),
                    source_value: ValueId::new(10).unwrap(),
                },
                victim_definition_site: ValueDefinitionSite::Node {
                    block: BlockId::new(11).unwrap(),
                    node: 1,
                },
                current_view: RegisterViewId(1),
                reclaimed_view: RegisterViewId(0),
                storage,
                store: LogicalSpillStore {
                    before_instruction: SelectedInstructionId(3),
                    source: VirtualRegisterId(1),
                    storage: storage.id,
                },
                reload: LogicalSpillReload {
                    before_instruction: SelectedInstructionId(4),
                    storage: storage.id,
                    result: LogicalReloadValueId(0),
                },
                rewrites: vec![LogicalSpillUseRewrite {
                    block: SelectedBlockId(0),
                    point: LiveRangePoint(8),
                    instruction: SelectedInstructionId(4),
                    operand: 0,
                    result: LogicalReloadValueId(0),
                }],
            }),
        }],
    })
}

pub(super) fn validated_source(plan: LogicalSpillOperationPlan) -> ValidatedLogicalSpillOperations {
    let planned_function_count = plan
        .functions
        .iter()
        .filter(|function| function.action.is_some())
        .count();
    let rewritten_use_count = plan
        .functions
        .iter()
        .filter_map(|function| function.action.as_ref())
        .map(|action| action.rewrites.len())
        .sum();
    let receipt = LogicalSpillOperationValidationReceipt {
        identity: logical_spill_operation_identity(&plan),
        selected: plan.selected,
        ranges: plan.ranges,
        legality: plan.legality,
        spill_choices: plan.spill_choices,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.functions.len(),
        planned_function_count,
        store_count: planned_function_count,
        reload_count: planned_function_count,
        rewritten_use_count,
    };
    ValidatedLogicalSpillOperations { plan, receipt }
}
