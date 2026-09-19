//! Fixtures shared by the literal fold tests: fold budgets, staged
//! extension and byte-view inputs and the fold driver.
//! `staged_memory_inputs.rs` and `staged_arithmetic_inputs.rs` hold the staged
//! memory and arithmetic inputs.

mod bitwise_and_ones_copies;
mod bitwise_and_zero_folds;
mod bitwise_xor_zero_copies;
mod compare_left_immediate_folds;
mod compare_subtract_add_folds;
mod divide_and_remainder_folds;
mod extension_and_copy_folds;
mod load_and_byte_view_folds;
mod saturating_add_upper_bound_materializations;
mod saturating_add_zero_copies;
mod saturating_divide_one_copies;
mod saturating_divide_zero_dividend_materializations;
mod saturating_subtract_upper_bound_subtrahend_materializations;
mod saturating_subtract_zero_copies;
mod saturating_subtract_zero_minuend_materializations;
mod staged_arithmetic_inputs;
mod staged_memory_inputs;
mod wrapping_add_zero_copies;

use staged_arithmetic_inputs::{
    BlockZeroTerminator, staged_add_inputs, staged_and_inputs, staged_and_ones_inputs,
    staged_divide_inputs, staged_divide_zero_dividend_inputs, staged_remainder_inputs,
    staged_remainder_minus_one_inputs, staged_remainder_zero_dividend_inputs,
    staged_saturating_add_carrier_inputs, staged_saturating_add_inputs,
    staged_saturating_add_upper_bound_carrier_inputs, staged_saturating_divide_carrier_inputs,
    staged_saturating_divide_inputs, staged_saturating_divide_zero_dividend_carrier_inputs,
    staged_saturating_subtract_carrier_inputs, staged_saturating_subtract_inputs,
    staged_saturating_subtract_upper_bound_carrier_inputs,
    staged_saturating_subtract_zero_minuend_carrier_inputs, staged_subtract_inputs,
    staged_wrapping_add_inputs, staged_xor_inputs,
};
use staged_memory_inputs::{
    staged_byte_view_address_backing_inputs, staged_byte_view_address_inputs, staged_copy_inputs,
    staged_extension_inputs, staged_load8_indexed_inputs,
};

use super::super::super::super::{
    AllocationLegalityIdentity, AllocationLegalityPlan, AllocatorAvailabilityIdentity,
    AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy, FunctionAllocationLegality,
    FunctionRecoveryClassification, FunctionSpillChoices, LiveRangeIdentity, LivenessIdentity,
    PressureRecoveryClassification, RecoveryClassification, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, RecoveryFutureUse,
    RecoveryVictimRole, SpillChoice, SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy,
};
use crate::{
    AllocationLegalityValidationReceipt, AllocatorAvailabilityValidationReceipt,
    FunctionLiteralFold, LiteralFoldError, LiteralFoldIdentity, LiteralFoldPlan, LiteralFoldPolicy,
    LiteralFoldValidationReceipt, LiveRangeValidationReceipt,
    RecoveryClassificationValidationReceipt, SpillChoiceError, SpillChoiceValidationReceipt,
    ValidatedAllocationLegality, ValidatedAllocatorAvailability, ValidatedLiteralFold,
    ValidatedLiveRanges, ValidatedRecoveryClassifications, ValidatedSpillChoices,
    analyze_allocation_legality, analyze_live_ranges, analyze_liveness, choose_spill_victims,
    classify_pressure_recovery, fold_selected_incoming_literal, materialize_allocator_availability,
    validate_literal_fold, validated_machine_effect_catalog,
};
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    BlockPointDomain, FunctionLiveRanges, LiveRangeFragment, LiveRangePlan, LiveRangePoint,
    LivenessPosition, SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionProvenance, SelectedOperand, SelectedSuccessor, SelectedSuccessorRole,
    SelectedTerminator, VirtualLiveRange, VirtualOccurrence, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};
use std::sync::Arc;
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap()
}

fn usage() -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates: 1,
        validation_steps: 1,
        commits: 1,
        iterations: 1,
    }
}

fn successor(block: u32, edge: u64) -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::Semantic,
        psi_edge: EdgeId::new(edge).unwrap(),
        block: SelectedBlockId(block),
        source_target: BlockId::new(2).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        fuel: Vec::new(),
    }
}

struct Inputs {
    selected: ValidatedLiteralFold,
    ranges: ValidatedLiveRanges,
    legality: ValidatedAllocationLegality,
    spill_choices: ValidatedSpillChoices,
    recovery: ValidatedRecoveryClassifications,
    availability: ValidatedAllocatorAvailability,
}

/// A `MaterializeI64` victim feeding operand 1 of `CompareI64`, with the
/// pressure-recovery classification already admitted as an `Incoming`
/// rematerialization candidate.
fn staged_inputs(target: NativeTarget) -> Inputs {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let machine = MachineId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let compare = environment.constraint(keys.compare_i64).unwrap();
    let branch = environment.constraint(keys.conditional_branch).unwrap();
    let terminal = environment.constraint(keys.return_unit).unwrap();
    let gpr = materialize.operands[0].class;
    let source_block = BlockId::new(1).unwrap();
    let literal_operation = OperationId::new(1).unwrap();
    let literal_value = ValueId::new(2).unwrap();
    let literal_provenance = SelectedInstructionProvenance {
        operations: vec![literal_operation],
        values: vec![literal_value],
        edges: Vec::new(),
        obligations: Vec::new(),
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Operation(literal_operation),
            units: 2,
        }],
    };
    let literal = SelectedInstruction {
        id: SelectedInstructionId(0),
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(5),
        },
        constraint: materialize.key,
        operands: vec![SelectedOperand {
            operand: materialize.operands[0].operand,
            virtual_register: VirtualRegisterId(1),
            access: materialize.operands[0].access,
            class: gpr,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }],
        implicit_uses: materialize.implicit_uses.clone(),
        implicit_defs: materialize.implicit_defs.clone(),
        clobbers: materialize.clobbers.clone(),
        provenance: literal_provenance.clone(),
    };
    let consumer = SelectedInstruction {
        id: SelectedInstructionId(1),
        kind: SelectedInstructionKind::CompareI64,
        constraint: compare.key,
        operands: vec![
            SelectedOperand {
                operand: 0,
                virtual_register: VirtualRegisterId(0),
                access: RegisterOperandAccess::Use,
                class: gpr,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            SelectedOperand {
                operand: 1,
                virtual_register: VirtualRegisterId(1),
                access: RegisterOperandAccess::Use,
                class: gpr,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
        ],
        implicit_uses: compare.implicit_uses.clone(),
        implicit_defs: compare.implicit_defs.clone(),
        clobbers: compare.clobbers.clone(),
        provenance: SelectedInstructionProvenance {
            operations: vec![OperationId::new(2).unwrap()],
            values: vec![ValueId::new(1).unwrap()],
            ..Default::default()
        },
    };
    let branch_instruction = SelectedInstruction {
        id: SelectedInstructionId(2),
        kind: SelectedInstructionKind::ConditionalBranchNonZero,
        constraint: branch.key,
        operands: Vec::new(),
        implicit_uses: branch.implicit_uses.clone(),
        implicit_defs: branch.implicit_defs.clone(),
        clobbers: branch.clobbers.clone(),
        provenance: Default::default(),
    };
    let return_instruction = SelectedInstruction {
        id: SelectedInstructionId(3),
        kind: SelectedInstructionKind::ReturnUnit,
        constraint: terminal.key,
        operands: Vec::new(),
        implicit_uses: terminal.implicit_uses.clone(),
        implicit_defs: terminal.implicit_defs.clone(),
        clobbers: terminal.clobbers.clone(),
        provenance: Default::default(),
    };
    let plan = SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target,
        entry: machine,
        functions: vec![SelectedFunction {
            machine,
            attachment: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: vec![
                VirtualRegister {
                    id: VirtualRegisterId(0),
                    scalar_type: scalar,
                    class: gpr,
                    origin: VirtualRegisterOrigin::EntryParameter {
                        source_value: ValueId::new(1).unwrap(),
                        parameter_index: 0,
                    },
                    definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: VirtualRegisterId(1),
                    scalar_type: scalar,
                    class: gpr,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(0),
                        source_value: literal_value,
                    },
                    definition_site: Some(ValueDefinitionSite::Node {
                        block: source_block,
                        node: 0,
                    }),
                    entry_fixed_view: None,
                },
            ],
            blocks: vec![
                SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(source_block),
                    instructions: vec![literal, consumer],
                    terminator: SelectedTerminator::ConditionalBranch {
                        instruction: branch_instruction,
                        when_nonzero: successor(1, 1),
                        when_zero: successor(1, 2),
                    },
                },
                SelectedBlock {
                    id: SelectedBlockId(1),
                    origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Return {
                        instruction: return_instruction,
                        psi_return_edge: EdgeId::new(3).unwrap(),
                    },
                },
            ],
        }]
        .into(),
    };
    let selected_identity = selected_instruction_plan_identity(&plan);
    let unit = OptimizationUnitIdentity::from_bytes([8; 32]);
    let fuel = plan.fuel_schedule;
    let ranges_identity = LiveRangeIdentity::from_bytes([10; 32]);
    let legality_identity = AllocationLegalityIdentity::from_bytes([11; 32]);
    let availability_identity = AllocatorAvailabilityIdentity::from_bytes([12; 32]);
    let spill_identity = SpillChoiceIdentity::from_bytes([13; 32]);
    let recovery_identity = RecoveryClassificationIdentity::from_bytes([14; 32]);
    let environment_identity = environment.identity();
    let effect_catalog_identity =
        validated_machine_effect_catalog(target, environment.constraints())
            .unwrap()
            .identity();

    let selected = ValidatedLiteralFold {
        plan: LiteralFoldPlan {
            source_selected: selected_identity,
            spill_choices: spill_identity,
            recovery_classifications: recovery_identity,
            ranges: ranges_identity,
            legality: legality_identity,
            register_environment: environment_identity,
            allocator_availability: availability_identity,
            machine_effect_catalog: effect_catalog_identity,
            optimization_unit: unit,
            fuel_schedule: fuel,
            policy: LiteralFoldPolicy::COMPARE_V1,
            budget: budget(),
            usage: usage(),
            functions: vec![FunctionLiteralFold {
                machine,
                action: None,
            }],
            transformed_selected: selected_identity,
        },
        transformed: Arc::new(plan),
        receipt: LiteralFoldValidationReceipt {
            identity: LiteralFoldIdentity::from_bytes([15; 32]),
            source_selected: selected_identity,
            spill_choices: spill_identity,
            recovery_classifications: recovery_identity,
            ranges: ranges_identity,
            legality: legality_identity,
            register_environment: environment_identity,
            allocator_availability: availability_identity,
            machine_effect_catalog: effect_catalog_identity,
            optimization_unit: unit,
            fuel_schedule: fuel,
            transformed_selected: selected_identity,
            policy: LiteralFoldPolicy::COMPARE_V1,
            usage: usage(),
            function_count: 1,
            applied_count: 0,
        },
    };

    let ranges = ValidatedLiveRanges {
        plan: Arc::new(LiveRangePlan {
            selected: selected_identity,
            liveness: LivenessIdentity::from_bytes([16; 32]),
            optimization_unit: unit,
            fuel_schedule: fuel,
            target,
            functions: vec![FunctionLiveRanges {
                machine,
                block_domains: vec![
                    BlockPointDomain {
                        block: SelectedBlockId(0),
                        source_block,
                        start: LiveRangePoint(0),
                        end: LiveRangePoint(3),
                    },
                    BlockPointDomain {
                        block: SelectedBlockId(1),
                        source_block: BlockId::new(2).unwrap(),
                        start: LiveRangePoint(3),
                        end: LiveRangePoint(5),
                    },
                ],
                virtual_registers: vec![
                    VirtualLiveRange {
                        virtual_register: VirtualRegisterId(0),
                        class: gpr,
                        occurrences: vec![VirtualOccurrence {
                            position: LivenessPosition(0),
                            point: LiveRangePoint(0),
                            instruction: SelectedInstructionId(1),
                            operand: 0,
                            access: RegisterOperandAccess::Use,
                        }],
                        fixed_constraints: Vec::new(),
                        fragments: vec![LiveRangeFragment {
                            block: SelectedBlockId(0),
                            start: LiveRangePoint(0),
                            end: LiveRangePoint(3),
                        }],
                        edge_connectors: Vec::new(),
                    },
                    VirtualLiveRange {
                        virtual_register: VirtualRegisterId(1),
                        class: gpr,
                        occurrences: vec![
                            VirtualOccurrence {
                                position: LivenessPosition(1),
                                point: LiveRangePoint(1),
                                instruction: SelectedInstructionId(0),
                                operand: 0,
                                access: RegisterOperandAccess::Def,
                            },
                            VirtualOccurrence {
                                position: LivenessPosition(2),
                                point: LiveRangePoint(2),
                                instruction: SelectedInstructionId(1),
                                operand: 1,
                                access: RegisterOperandAccess::Use,
                            },
                        ],
                        fixed_constraints: Vec::new(),
                        fragments: vec![LiveRangeFragment {
                            block: SelectedBlockId(0),
                            start: LiveRangePoint(1),
                            end: LiveRangePoint(3),
                        }],
                        edge_connectors: Vec::new(),
                    },
                ],
                tied_pairs: Vec::new(),
                edge_transfers: Vec::new(),
                copy_affinities: Vec::new(),
                early_clobbers: Vec::new(),
                architectural_units: Vec::new(),
                interference: Vec::new(),
            }],
        }),
        receipt: LiveRangeValidationReceipt {
            identity: ranges_identity,
            selected: selected_identity,
            liveness: LivenessIdentity::from_bytes([16; 32]),
            optimization_unit: unit,
            fuel_schedule: fuel,
            function_count: 1,
            block_count: 2,
            virtual_register_count: 2,
            virtual_occurrence_count: 3,
            fixed_constraint_count: 0,
            virtual_fragment_count: 2,
            architectural_unit_count: 0,
            architectural_action_count: 0,
            architectural_fragment_count: 0,
            virtual_edge_connector_count: 0,
            architectural_edge_connector_count: 0,
            interference_count: 0,
            tied_pair_count: 0,
            copy_affinity_count: 0,
            tied_component_count: 0,
            early_clobber_count: 0,
            early_clobber_use_count: 0,
        },
    };

    let availability = ValidatedAllocatorAvailability {
        plan: AllocatorAvailabilityPlan {
            register_environment: environment_identity,
            physical: environment.physical().identity(),
            policy: AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
            classes: Vec::new(),
        },
        receipt: AllocatorAvailabilityValidationReceipt {
            identity: availability_identity,
            register_environment: environment_identity,
            physical: environment.physical().identity(),
            class_count: 0,
            unconstrained_view_count: 0,
        },
    };

    let legality = ValidatedAllocationLegality {
        plan: Arc::new(AllocationLegalityPlan {
            ranges: ranges_identity,
            register_environment: environment_identity,
            allocator_availability: availability_identity,
            functions: vec![FunctionAllocationLegality {
                machine,
                virtual_registers: Vec::new(),
            }],
        }),
        receipt: AllocationLegalityValidationReceipt {
            identity: legality_identity,
            ranges: ranges_identity,
            register_environment: environment_identity,
            allocator_availability: availability_identity,
            function_count: 1,
            virtual_register_count: 0,
            point_count: 0,
            candidate_count: 0,
            early_clobber_point_count: 0,
            early_clobber_candidate_count: 0,
            entry_transition_count: 0,
        },
    };

    let spill_choices = ValidatedSpillChoices {
        plan: SpillChoicePlan {
            legality: legality_identity,
            ranges: ranges_identity,
            register_environment: environment_identity,
            allocator_availability: availability_identity,
            policy: SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            budget: budget(),
            usage: usage(),
            functions: vec![FunctionSpillChoices {
                machine,
                choice: Some(SpillChoice {
                    block: SelectedBlockId(0),
                    point: LiveRangePoint(2),
                    incoming: VirtualRegisterId(1),
                    incoming_class: gpr,
                    incoming_common_candidates: Vec::new(),
                    active_residents: Vec::new(),
                    contenders: Vec::new(),
                    selected_victim: VirtualRegisterId(1),
                }),
            }],
        },
        receipt: SpillChoiceValidationReceipt {
            identity: spill_identity,
            legality: legality_identity,
            ranges: ranges_identity,
            register_environment: environment_identity,
            allocator_availability: availability_identity,
            policy: SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            usage: usage(),
            function_count: 1,
            choice_count: 1,
            contender_count: 0,
        },
    };

    let recovery = ValidatedRecoveryClassifications {
        plan: RecoveryClassificationPlan {
            selected: selected_identity,
            spill_choices: spill_identity,
            ranges: ranges_identity,
            legality: legality_identity,
            register_environment: environment_identity,
            allocator_availability: availability_identity,
            optimization_unit: unit,
            fuel_schedule: fuel,
            policy: RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            budget: budget(),
            usage: usage(),
            functions: vec![FunctionRecoveryClassification {
                machine,
                classification: Some(PressureRecoveryClassification {
                    block: SelectedBlockId(0),
                    point: LiveRangePoint(2),
                    victim: VirtualRegisterId(1),
                    role: RecoveryVictimRole::Incoming,
                    scalar_type: scalar,
                    class: gpr,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(0),
                        source_value: literal_value,
                    },
                    definition_site: Some(ValueDefinitionSite::Node {
                        block: source_block,
                        node: 0,
                    }),
                    classification:
                        RecoveryClassification::ImmediateU64RematerializationCandidate {
                            defining_instruction: SelectedInstructionId(0),
                            source_value: literal_value,
                            value: IntegerValue::Unsigned(5),
                            provenance: literal_provenance,
                            future_uses: vec![RecoveryFutureUse {
                                block: SelectedBlockId(0),
                                point: LiveRangePoint(2),
                                instruction: SelectedInstructionId(1),
                                operand: 1,
                            }],
                        },
                }),
            }],
        },
        receipt: RecoveryClassificationValidationReceipt {
            identity: recovery_identity,
            selected: selected_identity,
            spill_choices: spill_identity,
            ranges: ranges_identity,
            legality: legality_identity,
            register_environment: environment_identity,
            allocator_availability: availability_identity,
            optimization_unit: unit,
            fuel_schedule: fuel,
            policy: RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            usage: usage(),
            function_count: 1,
            classification_count: 1,
            immediate_candidate_count: 1,
        },
    };

    Inputs {
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
    }
}

/// A `MaterializeI64` victim feeding operand 0 — the minuend — of
/// `CompareI64`, staged by reversing the right-literal fixture's compare
/// operand registers and updating the three artifacts that record operand
/// positions: the plan's operand records, the live-range occurrences, and
/// the classification's future use. The conditional-branch terminator the
/// base fixture carries stays the equality-sensing reader the
/// operand-swapped grammar's flow audit admits.
fn staged_left_inputs(target: NativeTarget) -> Inputs {
    let mut inputs = staged_inputs(target);
    let mut plan = inputs.selected.transformed().clone();
    let compare = plan.functions[0].blocks[0]
        .instructions
        .iter_mut()
        .find(|instruction| instruction.kind == SelectedInstructionKind::CompareI64)
        .expect("the staged fixture carries the compare");
    compare.operands[0].virtual_register = VirtualRegisterId(1);
    compare.operands[1].virtual_register = VirtualRegisterId(0);
    inputs.selected.transformed = Arc::new(plan);
    let ranges = Arc::make_mut(&mut inputs.ranges.plan);
    ranges.functions[0].virtual_registers[0].occurrences[0].operand = 1;
    ranges.functions[0].virtual_registers[1].occurrences[1].operand = 0;
    let RecoveryClassification::ImmediateU64RematerializationCandidate { future_uses, .. } =
        &mut inputs.recovery.plan.functions[0]
            .classification
            .as_mut()
            .expect("the staged fixture admits a candidate")
            .classification
    else {
        unreachable!()
    };
    future_uses[0].operand = 0;
    inputs
}

fn fold_with(
    inputs: &Inputs,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: LiteralFoldPolicy,
) -> Result<ValidatedLiteralFold, LiteralFoldError> {
    fold_with_budget(inputs, environment, policy, budget())
}

fn fold_with_budget(
    inputs: &Inputs,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: LiteralFoldPolicy,
    work_budget: OptimizationWorkBudget,
) -> Result<ValidatedLiteralFold, LiteralFoldError> {
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    fold_selected_incoming_literal(
        &inputs.selected,
        &inputs.ranges,
        &inputs.legality,
        &inputs.spill_choices,
        &inputs.recovery,
        &inputs.availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &keys,
        &effect_catalog,
        policy,
        work_budget,
    )
}

/// Every SelectedLowering literal-fold family enabled except `disabled`: the
/// strongest disabled-policy posture, where the rule's own bit is the only
/// admission gate left closed.
fn policy_without(disabled: LiteralFoldPolicy) -> LiteralFoldPolicy {
    policy_without_all(&[disabled])
}

/// Every SelectedLowering literal-fold family enabled except each policy in
/// `disabled`: the strongest posture for a consumer kind disjoint families
/// share — closing only one `WrappingRemainderI64` or `BitwiseAndI64` bit
/// still admits the kind through its sibling, so a test of the unadmitted
/// kind closes every bit that kind answers to.
fn policy_without_all(disabled: &[LiteralFoldPolicy]) -> LiteralFoldPolicy {
    [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        LiteralFoldPolicy::COMPARE_V1,
        LiteralFoldPolicy::EXTENSION_V1,
        LiteralFoldPolicy::LOAD8_INDEXED_V1,
        LiteralFoldPolicy::COPY_V1,
        LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        LiteralFoldPolicy::EXACT_DIVIDE_V1,
        LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        LiteralFoldPolicy::BITWISE_AND_ONES_V1,
        LiteralFoldPolicy::BITWISE_AND_ZERO_V1,
        LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
        LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
        LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
        LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
        LiteralFoldPolicy::SATURATING_ADD_ZERO_V1,
        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
        LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
        LiteralFoldPolicy::SATURATING_DIVIDE_ZERO_V1,
        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
        LiteralFoldPolicy::SATURATING_ADD_UPPER_BOUND_V1,
        LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
        LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1,
    ]
    .into_iter()
    .filter(|policy| !disabled.contains(policy))
    .fold(LiteralFoldPolicy::empty(), |enabled, policy| {
        enabled.union(policy)
    })
}

/// Restage the fixture's materialized literal at `value`: the producer record
/// and the classification's carried immediate move together, matching the
/// fixture the recovery analysis would have produced for that literal.
fn restage_literal(inputs: &mut Inputs, value: u64) {
    let RecoveryClassification::ImmediateU64RematerializationCandidate {
        defining_instruction,
        ..
    } = &inputs.recovery.plan.functions[0]
        .classification
        .as_ref()
        .expect("the staged fixture admits a candidate")
        .classification
    else {
        panic!("the staged classification is an immediate candidate")
    };
    let defining_instruction = *defining_instruction;
    let mut plan = inputs.selected.transformed().clone();
    let literal = plan.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| block.instructions.iter_mut())
        .find(|instruction| instruction.id == defining_instruction)
        .expect("the classification's defining instruction exists in the plan");
    literal.kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(u128::from(value)),
    };
    inputs.selected.transformed = Arc::new(plan);
    let RecoveryClassification::ImmediateU64RematerializationCandidate {
        value: recorded, ..
    } = &mut inputs.recovery.plan.functions[0]
        .classification
        .as_mut()
        .expect("the staged fixture admits a candidate")
        .classification
    else {
        unreachable!()
    };
    *recorded = IntegerValue::Unsigned(u128::from(value));
}

/// The producer refuses to publish once measured usage exceeds the supplied
/// budget on any axis, and the replay refuses a plan whose recorded budget
/// starves its recorded usage. `validation_steps` is the one axis a
/// single-function staged fold measures above one — the only axis a nonzero
/// budget can starve below the requirement — so it carries the starved leg.
fn assert_budget_is_enforced(
    inputs: &Inputs,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: LiteralFoldPolicy,
) {
    let result = fold_with(inputs, environment, policy).expect("the staged fold validates");
    let required = result.plan().usage;
    assert_eq!(required.candidates, 1, "the staged fold applies once");

    // The exact measured requirement is the admission boundary: a budget
    // equal to it still admits the fold.
    let exact = OptimizationWorkBudget::new(
        required.rule_evaluations,
        required.candidates,
        required.validation_steps,
        required.commits,
        required.iterations,
    )
    .expect("the measured usage is nonzero on every axis");
    fold_with_budget(inputs, environment, policy, exact)
        .expect("a budget exactly meeting the measured usage admits the fold");

    let starved = OptimizationWorkBudget::new(
        required.rule_evaluations,
        required.candidates,
        required.validation_steps - 1,
        required.commits,
        required.iterations,
    )
    .expect("one below the measured validation steps is still nonzero");
    assert_eq!(
        fold_with_budget(inputs, environment, policy, starved).map(|_| ()),
        Err(LiteralFoldError::BudgetExceeded {
            required,
            budget: starved,
        }),
        "the producer refuses to publish a fold whose work exceeds the budget"
    );
    let mut forged = result.plan().clone();
    forged.budget = starved;
    assert_eq!(
        validate(inputs, environment, forged).map(|_| ()),
        Err(LiteralFoldError::BudgetExceeded {
            required,
            budget: starved,
        }),
        "the replay refuses a plan whose recorded budget starves its usage"
    );
}

/// Repeated computation is deterministic — identical plan, receipt, and
/// transformed program — and the published fold is itself the legal second
/// input: feeding it back with every supporting analysis re-derived over its
/// transformed plan, the staged pipeline's next-attempt custody, finds no
/// admitted candidate, so the pass is a fixed point on its own output rather
/// than merely reconstructible.
fn assert_deterministic_fixed_point(
    inputs: &Inputs,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: LiteralFoldPolicy,
) {
    let first = fold_with(inputs, environment, policy).expect("the staged fold validates");
    let second = fold_with(inputs, environment, policy).expect("the staged fold validates");
    assert_eq!(first.plan(), second.plan());
    assert_eq!(first.receipt(), second.receipt());
    assert_eq!(first.transformed(), second.transformed());

    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    // `first` is the sealed selected input the pipeline's `build_attempt`
    // consumes; every artifact below is re-derived over its transformed plan
    // by the real analyses, so each receipt names the published transformed
    // identity rather than inheriting a stale source-stage binding. The
    // availability is materialized for real — the staged fixture carries an
    // intentionally empty class set that cannot drive legality.
    let availability = materialize_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &keys,
        AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
    )
    .expect("allocator availability for the published fold");
    let liveness = analyze_liveness(&first).expect("liveness over the published fold");
    let ranges =
        analyze_live_ranges(&first, &liveness).expect("live ranges over the published fold");
    let legality = analyze_allocation_legality(
        &ranges,
        &availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &keys,
    )
    .expect("allocation legality over the published fold");
    let spill_choices = match choose_spill_victims(
        &legality,
        &ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &keys,
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        budget(),
    ) {
        Ok(choices) => choices,
        // A register the transformed plan leaves entirely unreferenced —
        // the extension and copy fixtures' dead entry parameter — is an
        // input victim selection cannot measure. Stage the no-victim verdict
        // its receipt would have carried, bound to the real re-derived
        // roots, and let the real classifier judge it.
        Err(SpillChoiceError::NoLivePoints { .. }) => ValidatedSpillChoices {
            plan: SpillChoicePlan {
                legality: legality.receipt().identity(),
                ranges: ranges.receipt().identity(),
                register_environment: environment.identity(),
                allocator_availability: availability.receipt().identity(),
                policy: SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                budget: budget(),
                usage: usage(),
                functions: first
                    .transformed()
                    .functions
                    .iter()
                    .map(|function| FunctionSpillChoices {
                        machine: function.machine,
                        choice: None,
                    })
                    .collect(),
            },
            receipt: SpillChoiceValidationReceipt {
                identity: SpillChoiceIdentity::from_bytes([17; 32]),
                legality: legality.receipt().identity(),
                ranges: ranges.receipt().identity(),
                register_environment: environment.identity(),
                allocator_availability: availability.receipt().identity(),
                policy: SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                usage: usage(),
                function_count: first.transformed().functions.len(),
                choice_count: 0,
                contender_count: 0,
            },
        },
        Err(error) => panic!("spill choices over the published fold: {error:?}"),
    };
    let recovery = classify_pressure_recovery(
        &first,
        &ranges,
        &legality,
        &spill_choices,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        budget(),
    )
    .expect("recovery classification over the published fold");
    let terminal = fold_selected_incoming_literal(
        &first,
        &ranges,
        &legality,
        &spill_choices,
        &recovery,
        &availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &keys,
        &effect_catalog,
        policy,
        budget(),
    )
    .expect("the published fold is a legal second input");
    assert_eq!(terminal.receipt().applied_count(), 0);
    assert_eq!(
        terminal.receipt().source_selected(),
        terminal.receipt().transformed_selected()
    );
    assert_eq!(
        terminal.receipt().transformed_selected(),
        first.receipt().transformed_selected()
    );
    assert_eq!(terminal.transformed(), first.transformed());
}

fn fold(inputs: &Inputs, environment: &ValidatedTargetRegisterEnvironment) -> ValidatedLiteralFold {
    fold_with(inputs, environment, LiteralFoldPolicy::COMPARE_V1)
        .expect("the staged compare fold should validate")
}

fn validate(
    inputs: &Inputs,
    environment: &ValidatedTargetRegisterEnvironment,
    plan: LiteralFoldPlan,
) -> Result<ValidatedLiteralFold, LiteralFoldError> {
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    validate_literal_fold(
        &inputs.selected,
        &inputs.ranges,
        &inputs.legality,
        &inputs.spill_choices,
        &inputs.recovery,
        &inputs.availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &keys,
        &effect_catalog,
        plan,
    )
}

fn unsigned(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
}

fn signed(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, bits).unwrap())
}
