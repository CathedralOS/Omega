use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    SelectedOperand, SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    ScalarType, ValueId,
};

use crate::*;

pub(super) struct Fixture {
    pub(super) plan: LogicalSpillOperationPlan,
}

pub(super) struct RawFixture {
    pub(super) selected: SelectedFunction,
    pub(super) ranges: FunctionLiveRanges,
    pub(super) legality: FunctionAllocationLegality,
    pub(super) choices: FunctionSpillChoices,
}

pub(super) fn fixture() -> Fixture {
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let storage = LogicalSpillStorage {
        id: LogicalSpillStorageId(0),
        class: LogicalSpillStorageClass::NonAddressUnsignedU64V1,
    };
    Fixture {
        plan: LogicalSpillOperationPlan {
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
                    rewrites: vec![
                        LogicalSpillUseRewrite {
                            block: SelectedBlockId(0),
                            point: LiveRangePoint(6),
                            instruction: SelectedInstructionId(4),
                            operand: 0,
                            result: LogicalReloadValueId(0),
                        },
                        LogicalSpillUseRewrite {
                            block: SelectedBlockId(0),
                            point: LiveRangePoint(8),
                            instruction: SelectedInstructionId(5),
                            operand: 1,
                            result: LogicalReloadValueId(0),
                        },
                    ],
                }),
            }],
        },
    }
}

pub(super) fn raw_fixture() -> RawFixture {
    let machine = MachineId::new(20).unwrap();
    let source_block = BlockId::new(21).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let key = omega_register_model::RegisterConstraintKey {
        family: omega_register_model::RegisterConstraintFamily::Instruction,
        variant: 1,
    };
    let operand = |register, access| SelectedOperand {
        operand: 0,
        virtual_register: VirtualRegisterId(register),
        access,
        class: RegisterClassId(0),
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    };
    let instruction = |id, register, access| SelectedInstruction {
        id: SelectedInstructionId(id),
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(register) + 1),
        },
        constraint: key,
        operands: vec![operand(register, access)],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    };
    let registers = (0..3_u32)
        .map(|id| VirtualRegister {
            id: VirtualRegisterId(id),
            scalar_type: scalar,
            class: RegisterClassId(0),
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(id),
                source_value: ValueId::new(u64::from(id) + 30).unwrap(),
            },
            definition_site: ValueDefinitionSite::Node {
                block: source_block,
                node: id,
            },
            entry_fixed_view: None,
        })
        .collect();
    let future_use = SelectedInstruction {
        id: SelectedInstructionId(3),
        kind: SelectedInstructionKind::CompareI64Zero,
        constraint: key,
        operands: vec![operand(0, omega_register_model::RegisterOperandAccess::Use)],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    };
    let returned = SelectedInstruction {
        id: SelectedInstructionId(4),
        kind: SelectedInstructionKind::ReturnI64,
        constraint: key,
        operands: vec![operand(0, omega_register_model::RegisterOperandAccess::Use)],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    };
    let selected = SelectedFunction {
        machine,
        attachment: None,
        provenance: Default::default(),
        entry_block: SelectedBlockId(0),
        virtual_registers: registers,
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            source_block,
            instructions: vec![
                instruction(0, 0, omega_register_model::RegisterOperandAccess::Def),
                instruction(1, 1, omega_register_model::RegisterOperandAccess::Def),
                instruction(2, 2, omega_register_model::RegisterOperandAccess::Def),
                future_use,
            ],
            terminator: SelectedTerminator::Return {
                instruction: returned,
                psi_return_edge: EdgeId::new(22).unwrap(),
            },
        }],
    };
    let occurrence = |position, point, instruction, access| VirtualOccurrence {
        position: LivenessPosition(position),
        point: LiveRangePoint(point),
        instruction: SelectedInstructionId(instruction),
        operand: 0,
        access,
    };
    let range = |id, occurrences: Vec<VirtualOccurrence>, start, end| VirtualLiveRange {
        virtual_register: VirtualRegisterId(id),
        class: RegisterClassId(0),
        occurrences,
        fixed_constraints: Vec::new(),
        fragments: vec![LiveRangeFragment {
            block: SelectedBlockId(0),
            start: LiveRangePoint(start),
            end: LiveRangePoint(end),
        }],
        edge_connectors: Vec::new(),
    };
    let ranges = FunctionLiveRanges {
        machine,
        block_domains: vec![BlockPointDomain {
            block: SelectedBlockId(0),
            source_block,
            start: LiveRangePoint(0),
            end: LiveRangePoint(10),
        }],
        virtual_registers: vec![
            range(
                0,
                vec![
                    occurrence(0, 1, 0, omega_register_model::RegisterOperandAccess::Def),
                    occurrence(3, 6, 3, omega_register_model::RegisterOperandAccess::Use),
                    occurrence(4, 8, 4, omega_register_model::RegisterOperandAccess::Use),
                ],
                1,
                9,
            ),
            range(
                1,
                vec![occurrence(
                    1,
                    3,
                    1,
                    omega_register_model::RegisterOperandAccess::Def,
                )],
                3,
                7,
            ),
            range(
                2,
                vec![occurrence(
                    2,
                    5,
                    2,
                    omega_register_model::RegisterOperandAccess::Def,
                )],
                5,
                7,
            ),
        ],
        tied_pairs: Vec::new(),
        early_clobbers: Vec::new(),
        architectural_units: Vec::new(),
        interference: Vec::new(),
    };
    let legality = FunctionAllocationLegality {
        machine,
        virtual_registers: (0..3_u32)
            .map(|id| VirtualRegisterAllocationLegality {
                virtual_register: VirtualRegisterId(id),
                class: RegisterClassId(0),
                points: Vec::new(),
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
            incoming_common_candidates: vec![RegisterViewId(0)],
            active_residents: vec![
                PressureResident {
                    virtual_register: VirtualRegisterId(0),
                    class: RegisterClassId(0),
                    start: LiveRangePoint(1),
                    exclusive_end: LiveRangePoint(9),
                    view: RegisterViewId(0),
                },
                PressureResident {
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
                    exclusive_end: LiveRangePoint(9),
                    reclaimed_view: Some(RegisterViewId(0)),
                },
                PressureContender {
                    virtual_register: VirtualRegisterId(2),
                    exclusive_end: LiveRangePoint(7),
                    reclaimed_view: None,
                },
            ],
            selected_victim: VirtualRegisterId(0),
        }),
    };
    RawFixture {
        selected,
        ranges,
        legality,
        choices,
    }
}
