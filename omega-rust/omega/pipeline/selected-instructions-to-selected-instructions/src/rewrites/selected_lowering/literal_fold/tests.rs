//! Firing and corruption coverage for the flag-defining compare fold.
//!
//! The upstream analysis artifacts are staged fixtures with self-consistent
//! receipt identities, matching how the spill-recovery tests stage inputs;
//! the fold's own producer and independent replay run for real.

use std::sync::Arc;

use optimization_core::{
    AcceptedObligationFactIdentity, OptimizationUnitIdentity, OptimizationWorkBudget,
    OptimizationWorkUsage,
};
use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    BlockPointDomain, FunctionLiveRanges, LiveRangeFragment, LiveRangePlan, LiveRangePoint,
    LivenessPosition, MachineEffectCatalogIdentity, SelectedBlock, SelectedBlockId,
    SelectedBlockOrigin, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, SelectedOperand, SelectedSuccessor, SelectedSuccessorRole,
    SelectedTerminator, VirtualLiveRange, VirtualOccurrence, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::*;

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

/// A `MaterializeI64` victim feeding the sole `Use` operand of a unary
/// extension consumer whose `Def` result carries `result_scalar`, with the
/// pressure-recovery classification admitted as an `Incoming` rematerialization
/// candidate at operand 0.
fn staged_extension_inputs(
    target: NativeTarget,
    kind: SelectedInstructionKind,
    literal_constant: u64,
    result_scalar: ScalarType,
) -> Inputs {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let machine = MachineId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let copy = environment.constraint(keys.copy_i64).unwrap();
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
            value: IntegerValue::Unsigned(u128::from(literal_constant)),
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
        kind,
        constraint: copy.key,
        operands: vec![
            SelectedOperand {
                operand: 0,
                virtual_register: VirtualRegisterId(1),
                access: RegisterOperandAccess::Use,
                class: gpr,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            SelectedOperand {
                operand: 1,
                virtual_register: VirtualRegisterId(2),
                access: RegisterOperandAccess::Def,
                class: gpr,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
        ],
        implicit_uses: copy.implicit_uses.clone(),
        implicit_defs: copy.implicit_defs.clone(),
        clobbers: copy.clobbers.clone(),
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
                VirtualRegister {
                    id: VirtualRegisterId(2),
                    scalar_type: result_scalar,
                    class: gpr,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(1),
                        source_value: ValueId::new(3).unwrap(),
                    },
                    definition_site: Some(ValueDefinitionSite::Node {
                        block: source_block,
                        node: 1,
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
            policy: LiteralFoldPolicy::EXTENSION_V1,
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
            policy: LiteralFoldPolicy::EXTENSION_V1,
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
                        occurrences: Vec::new(),
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
                                operand: 0,
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
                    VirtualLiveRange {
                        virtual_register: VirtualRegisterId(2),
                        class: gpr,
                        occurrences: vec![VirtualOccurrence {
                            position: LivenessPosition(2),
                            point: LiveRangePoint(2),
                            instruction: SelectedInstructionId(1),
                            operand: 1,
                            access: RegisterOperandAccess::Def,
                        }],
                        fixed_constraints: Vec::new(),
                        fragments: vec![LiveRangeFragment {
                            block: SelectedBlockId(0),
                            start: LiveRangePoint(2),
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
            virtual_register_count: 3,
            virtual_occurrence_count: 3,
            fixed_constraint_count: 0,
            virtual_fragment_count: 3,
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
                            value: IntegerValue::Unsigned(u128::from(literal_constant)),
                            provenance: literal_provenance,
                            future_uses: vec![RecoveryFutureUse {
                                block: SelectedBlockId(0),
                                point: LiveRangePoint(2),
                                instruction: SelectedInstructionId(1),
                                operand: 0,
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

/// A `MaterializeI64` victim feeding the right `Use` operand of an
/// `ExactSubtractI64` consumer whose `Def` result is a scalar register, with
/// the pressure-recovery classification admitted as an `Incoming`
/// rematerialization candidate at operand 1.
fn staged_subtract_inputs(target: NativeTarget) -> Inputs {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let machine = MachineId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let subtract = environment.constraint(keys.subtract_i64).unwrap();
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
        kind: SelectedInstructionKind::ExactSubtractI64 {
            obligation: ObligationId::new(7).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
        },
        constraint: subtract.key,
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
            SelectedOperand {
                operand: 2,
                virtual_register: VirtualRegisterId(2),
                access: RegisterOperandAccess::Def,
                class: gpr,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
        ],
        implicit_uses: subtract.implicit_uses.clone(),
        implicit_defs: subtract.implicit_defs.clone(),
        clobbers: subtract.clobbers.clone(),
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
                VirtualRegister {
                    id: VirtualRegisterId(2),
                    scalar_type: scalar,
                    class: gpr,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(1),
                        source_value: ValueId::new(3).unwrap(),
                    },
                    definition_site: Some(ValueDefinitionSite::Node {
                        block: source_block,
                        node: 1,
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
            policy: LiteralFoldPolicy::EXACT_SUBTRACT_V1,
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
            policy: LiteralFoldPolicy::EXACT_SUBTRACT_V1,
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
                            position: LivenessPosition(2),
                            point: LiveRangePoint(2),
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
                    VirtualLiveRange {
                        virtual_register: VirtualRegisterId(2),
                        class: gpr,
                        occurrences: vec![VirtualOccurrence {
                            position: LivenessPosition(2),
                            point: LiveRangePoint(2),
                            instruction: SelectedInstructionId(1),
                            operand: 2,
                            access: RegisterOperandAccess::Def,
                        }],
                        fixed_constraints: Vec::new(),
                        fragments: vec![LiveRangeFragment {
                            block: SelectedBlockId(0),
                            start: LiveRangePoint(2),
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
            virtual_register_count: 3,
            virtual_occurrence_count: 4,
            fixed_constraint_count: 0,
            virtual_fragment_count: 3,
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

/// A `MaterializeI64` victim feeding operand 1 — the index register — of
/// `Load8Indexed`, whose `Def` result is `VirtualRegisterId(2)`, with the
/// pressure-recovery classification already admitted as an `Incoming`
/// rematerialization candidate. `folded` is the folded byte offset.
fn staged_load8_indexed_inputs(target: NativeTarget, folded: u64) -> Inputs {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let machine = MachineId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let load8_indexed = environment.constraint(keys.load8_indexed.unwrap()).unwrap();
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
            value: IntegerValue::Unsigned(u128::from(folded)),
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
        kind: SelectedInstructionKind::Load8Indexed,
        constraint: load8_indexed.key,
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
            SelectedOperand {
                operand: 2,
                virtual_register: VirtualRegisterId(2),
                access: RegisterOperandAccess::Def,
                class: gpr,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
        ],
        implicit_uses: load8_indexed.implicit_uses.clone(),
        implicit_defs: load8_indexed.implicit_defs.clone(),
        clobbers: load8_indexed.clobbers.clone(),
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
                VirtualRegister {
                    id: VirtualRegisterId(2),
                    scalar_type: scalar,
                    class: gpr,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(1),
                        source_value: ValueId::new(3).unwrap(),
                    },
                    definition_site: Some(ValueDefinitionSite::Node {
                        block: source_block,
                        node: 1,
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
            policy: LiteralFoldPolicy::LOAD8_INDEXED_V1,
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
            policy: LiteralFoldPolicy::LOAD8_INDEXED_V1,
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
                            position: LivenessPosition(2),
                            point: LiveRangePoint(2),
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
                    VirtualLiveRange {
                        virtual_register: VirtualRegisterId(2),
                        class: gpr,
                        occurrences: vec![VirtualOccurrence {
                            position: LivenessPosition(2),
                            point: LiveRangePoint(2),
                            instruction: SelectedInstructionId(1),
                            operand: 2,
                            access: RegisterOperandAccess::Def,
                        }],
                        fixed_constraints: Vec::new(),
                        fragments: vec![LiveRangeFragment {
                            block: SelectedBlockId(0),
                            start: LiveRangePoint(2),
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
            virtual_register_count: 3,
            virtual_occurrence_count: 4,
            fixed_constraint_count: 0,
            virtual_fragment_count: 3,
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
                            value: IntegerValue::Unsigned(u128::from(folded)),
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

/// A `MaterializeI64` victim feeding `ExactAddI64`. `literal_operand` selects
/// which `Use` position carries the folded literal: 1 is the ordinary
/// right-operand grammar, 0 the commutative left-operand grammar. The
/// surviving register `VirtualRegisterId(0)` occupies the other position and
/// `VirtualRegisterId(2)` is the `Def` result either way.
fn staged_add_inputs(target: NativeTarget, literal_operand: u16) -> Inputs {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let machine = MachineId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let add = environment.constraint(keys.add_i64).unwrap();
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
    let use_operand = |operand: u16, register: VirtualRegisterId| SelectedOperand {
        operand,
        virtual_register: register,
        access: RegisterOperandAccess::Use,
        class: gpr,
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    };
    // The victim sits at `literal_operand`; the surviving register takes the
    // other `Use` position.
    let mut consumer_uses = [
        use_operand(literal_operand, VirtualRegisterId(1)),
        use_operand(1 - literal_operand, VirtualRegisterId(0)),
    ];
    consumer_uses.sort_by_key(|operand| operand.operand);
    let mut consumer_operands = consumer_uses.to_vec();
    consumer_operands.push(SelectedOperand {
        operand: 2,
        virtual_register: VirtualRegisterId(2),
        access: RegisterOperandAccess::Def,
        class: gpr,
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    });
    let consumer = SelectedInstruction {
        id: SelectedInstructionId(1),
        kind: SelectedInstructionKind::ExactAddI64 {
            obligation: ObligationId::new(7).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
        },
        constraint: add.key,
        operands: consumer_operands,
        implicit_uses: add.implicit_uses.clone(),
        implicit_defs: add.implicit_defs.clone(),
        clobbers: add.clobbers.clone(),
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
                VirtualRegister {
                    id: VirtualRegisterId(2),
                    scalar_type: scalar,
                    class: gpr,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(1),
                        source_value: ValueId::new(3).unwrap(),
                    },
                    definition_site: Some(ValueDefinitionSite::Node {
                        block: source_block,
                        node: 1,
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
            policy: LiteralFoldPolicy::EXACT_ADD_V1,
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
            policy: LiteralFoldPolicy::EXACT_ADD_V1,
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
                            position: LivenessPosition(2),
                            point: LiveRangePoint(2),
                            instruction: SelectedInstructionId(1),
                            operand: 1 - literal_operand,
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
                                operand: literal_operand,
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
                    VirtualLiveRange {
                        virtual_register: VirtualRegisterId(2),
                        class: gpr,
                        occurrences: vec![VirtualOccurrence {
                            position: LivenessPosition(2),
                            point: LiveRangePoint(2),
                            instruction: SelectedInstructionId(1),
                            operand: 2,
                            access: RegisterOperandAccess::Def,
                        }],
                        fixed_constraints: Vec::new(),
                        fragments: vec![LiveRangeFragment {
                            block: SelectedBlockId(0),
                            start: LiveRangePoint(2),
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
            virtual_register_count: 3,
            virtual_occurrence_count: 4,
            fixed_constraint_count: 0,
            virtual_fragment_count: 3,
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
                                operand: literal_operand,
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

fn fold_with(
    inputs: &Inputs,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: LiteralFoldPolicy,
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
        budget(),
    )
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

#[test]
fn compare_immediate_fold_rewrites_the_flag_defining_consumer_on_both_linux_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let compare_row = environment
            .constraint(
                environment
                    .allocation_constraint_keys()
                    .compare_i64_immediate,
            )
            .unwrap();
        let inputs = staged_inputs(target);
        let result = fold(&inputs, &environment);

        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, None);
        assert_eq!(action.immediate, 5);
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(
            action.immediate_constraint,
            environment
                .allocation_constraint_keys()
                .compare_i64_immediate
        );

        let function = &result.transformed().functions[0];
        assert_eq!(function.virtual_registers.len(), 1);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(5),
            }
        );
        assert_eq!(
            rewritten.constraint,
            environment
                .allocation_constraint_keys()
                .compare_i64_immediate
        );
        assert_eq!(rewritten.operands.len(), 1);
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.implicit_defs, compare_row.implicit_defs);
        // The folded literal's provenance joins the consumer's.
        assert_eq!(rewritten.provenance.operations.len(), 2);

        // Densification keeps instruction ids dense through the terminator.
        let SelectedTerminator::ConditionalBranch { instruction, .. } =
            &function.blocks[0].terminator
        else {
            panic!("conditional branch terminator retained");
        };
        assert_eq!(instruction.id, SelectedInstructionId(1));
        let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator else {
            panic!("return terminator retained");
        };
        assert_eq!(instruction.id, SelectedInstructionId(2));
    }
}

#[test]
fn compare_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_inputs(target);
    let result = fold(&inputs, &environment);

    for mutation in 0..9 {
        let mut plan = result.plan().clone();
        match mutation {
            0 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            1 => plan.functions[0].action.as_mut().unwrap().immediate += 1,
            2 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .consumer_instruction = SelectedInstructionId(9)
            }
            3 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .immediate_constraint
                    .variant += 1
            }
            4 => plan.functions[0].action = None,
            5 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            6 => plan.usage.candidates += 1,
            7 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            // A plan binding a different effect catalog is a root mismatch:
            // the replay refuses to re-derive the fold under a foreign
            // declaration set.
            8 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn subtract_immediate_fold_replaces_the_flag_clobbering_consumer_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let subtract_row = environment.constraint(keys.subtract_i64).unwrap();
        let immediate_row = environment.constraint(keys.subtract_i64_immediate).unwrap();
        let inputs = staged_subtract_inputs(target);

        // x86-64's register subtract clobbers the condition state the
        // immediate form preserves: the pair's declared surface admits the
        // consumer's clobber because the rewrite replaces it wholesale.
        if target == NativeTarget::linux_x64() {
            assert!(!subtract_row.clobbers.is_empty());
        }
        assert!(immediate_row.clobbers.is_empty());
        assert!(immediate_row.implicit_defs.is_empty());

        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1)
            .unwrap_or_else(|error| {
                panic!("subtract fold on {target:?} should validate: {error:?}")
            });

        assert_eq!(
            result.plan().machine_effect_catalog,
            effect_catalog.identity()
        );
        assert_eq!(
            result.receipt().machine_effect_catalog(),
            effect_catalog.identity()
        );
        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, Some(VirtualRegisterId(2)));
        assert_eq!(action.immediate, 5);
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(action.immediate_constraint, keys.subtract_i64_immediate);

        let function = &result.transformed().functions[0];
        assert_eq!(function.virtual_registers.len(), 2);
        assert_eq!(
            function.virtual_registers[1].origin,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(0),
                source_value: ValueId::new(3).unwrap(),
            }
        );
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::ExactSubtractI64Immediate {
                immediate: IntegerValue::Unsigned(5),
                obligation: ObligationId::new(7).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
            }
        );
        assert_eq!(rewritten.constraint, keys.subtract_i64_immediate);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        // The rewritten instruction carries exactly the immediate row's unit
        // surface: the register subtract's flag clobber does not survive.
        assert_eq!(rewritten.implicit_uses, immediate_row.implicit_uses);
        assert_eq!(rewritten.implicit_defs, immediate_row.implicit_defs);
        assert_eq!(rewritten.clobbers, immediate_row.clobbers);
        assert_eq!(rewritten.provenance.operations.len(), 2);
    }
}

#[test]
fn add_immediate_fold_commutes_the_literal_operand_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let immediate_row = environment.constraint(keys.add_i64_immediate).unwrap();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_add_inputs(target, literal_operand);
            let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1)
                .unwrap_or_else(|error| {
                    panic!(
                        "add fold with the literal at operand {literal_operand} on {target:?} \
                         should validate: {error:?}"
                    )
                });

            assert_eq!(
                result.plan().machine_effect_catalog,
                effect_catalog.identity()
            );
            assert_eq!(result.receipt().applied_count(), 1);
            let action = result.plan().functions[0].action.unwrap();
            assert_eq!(action.result, Some(VirtualRegisterId(2)));
            assert_eq!(action.immediate, 5);
            // The action records the surviving register, not the operand
            // position it occupied: `VirtualRegisterId(0)` binds the
            // rewritten row's `Use` position under either grammar.
            assert_eq!(action.surviving, VirtualRegisterId(0));
            assert_eq!(action.victim, VirtualRegisterId(1));
            assert_eq!(action.literal_instruction, SelectedInstructionId(0));
            assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
            assert_eq!(action.immediate_constraint, keys.add_i64_immediate);

            let function = &result.transformed().functions[0];
            assert_eq!(function.virtual_registers.len(), 2);
            assert_eq!(
                function.virtual_registers[1].origin,
                VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(0),
                    source_value: ValueId::new(3).unwrap(),
                }
            );
            let instructions = &function.blocks[0].instructions;
            assert_eq!(instructions.len(), 1);
            let rewritten = &instructions[0];
            assert_eq!(rewritten.id, SelectedInstructionId(0));
            // Both grammars rewrite to the same immediate form with proof
            // custody carried from the source consumer.
            assert_eq!(
                rewritten.kind,
                SelectedInstructionKind::ExactAddI64Immediate {
                    immediate: IntegerValue::Unsigned(5),
                    obligation: ObligationId::new(7).unwrap(),
                    accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
                }
            );
            assert_eq!(rewritten.constraint, keys.add_i64_immediate);
            assert_eq!(rewritten.operands.len(), 2);
            assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
            assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
            assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
            assert_eq!(rewritten.implicit_uses, immediate_row.implicit_uses);
            assert_eq!(rewritten.implicit_defs, immediate_row.implicit_defs);
            assert_eq!(rewritten.clobbers, immediate_row.clobbers);
            assert_eq!(rewritten.provenance.operations.len(), 2);
        }
    }
}

#[test]
fn add_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The left-literal grammar exercises the operand-0 fold.
    let inputs = staged_add_inputs(target, 0);
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1).unwrap();

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            0 => plan.functions[0].action.as_mut().unwrap().result = None,
            1 => plan.functions[0].action.as_mut().unwrap().immediate += 1,
            2 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .consumer_instruction = SelectedInstructionId(9)
            }
            3 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .immediate_constraint
                    .variant += 1
            }
            // The survivor record is decision-bearing: binding the removed
            // victim register into the rewritten `Use` must not validate.
            4 => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(1),
            5 => plan.functions[0].action.as_mut().unwrap().victim = VirtualRegisterId(0),
            6 => plan.functions[0].action = None,
            7 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            8 => plan.usage.candidates += 1,
            9 => plan.policy = LiteralFoldPolicy::COMPARE_V1,
            10 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn add_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Each fixture carries the literal at `literal_operand`; claiming the
        // other `Use` position in the recovery classification selects the
        // disjoint grammar, whose operand check finds the surviving register
        // where the victim must sit.
        for (literal_operand, claimed_operand) in [(0u16, 1u16), (1u16, 0u16)] {
            let mut inputs = staged_add_inputs(target, literal_operand);
            let RecoveryClassification::ImmediateU64RematerializationCandidate {
                future_uses, ..
            } = &mut inputs.recovery.plan.functions[0]
                .classification
                .as_mut()
                .unwrap()
                .classification
            else {
                unreachable!()
            };
            future_uses[0].operand = claimed_operand;
            assert_eq!(
                fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "literal at operand {literal_operand} claiming {claimed_operand} on {target:?}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "literal at operand {literal_operand} claiming {claimed_operand} on {target:?} \
                 replay"
            );
        }
    }
}

#[test]
fn add_fold_rejects_malformed_left_grammar_operand_arrangements() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..4 {
        let mut inputs = staged_add_inputs(target, 0);
        let mut plan = inputs.selected.transformed().clone();
        let consumer = &mut plan.functions[0].blocks[0].instructions[1];
        match mutation {
            // The folded operand is a `Def`, not a `Use`.
            0 => consumer.operands[0].access = RegisterOperandAccess::Def,
            // The surviving position is not a `Use`.
            1 => consumer.operands[1].access = RegisterOperandAccess::Def,
            // The result `Def` is missing from the binary grammar.
            2 => {
                consumer.operands.pop();
            }
            // An extra operand exceeds the binary grammar.
            _ => consumer.operands.push(consumer.operands[1]),
        }
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);
        inputs.selected = selected;
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation}"
        );
        assert_eq!(
            validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation} replay"
        );
    }
}

#[test]
fn fold_rejects_instructions_the_bound_effect_catalog_does_not_declare() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        for mutation in 0..2 {
            let inputs = staged_inputs(target);
            let mut plan = inputs.selected.transformed().clone();
            match mutation {
                // Binding the literal to the compare row leaves no
                // `(MaterializeI64, compare)` declaration to admit.
                0 => plan.functions[0].blocks[0].instructions[0].constraint = keys.compare_i64,
                // Binding the consumer to the materialize row leaves no
                // `(CompareI64, materialize)` declaration either.
                _ => plan.functions[0].blocks[0].instructions[1].constraint = keys.materialize_i64,
            }
            let mut selected = inputs.selected.clone();
            selected.transformed = Arc::new(plan);

            assert_eq!(
                fold_selected_incoming_literal(
                    &selected,
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
                    LiteralFoldPolicy::COMPARE_V1,
                    budget(),
                ),
                Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                "mutation {mutation} on {target:?}"
            );
            assert_eq!(
                validate_literal_fold(
                    &selected,
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
                    inputs.selected.plan().clone(),
                ),
                Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                "mutation {mutation} replay on {target:?}"
            );
        }
    }
}

#[test]
fn fold_rejects_a_literal_record_carrying_implicit_unit_traffic() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let condition_unit = environment
            .constraint(keys.compare_i64)
            .unwrap()
            .implicit_defs[0];
        let inputs = staged_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        // The eliminated instruction must carry no unit traffic at all: an
        // implicit condition-state definition on the literal would be dropped
        // silently by its removal.
        plan.functions[0].blocks[0].instructions[0]
            .implicit_defs
            .push(condition_unit);
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);

        assert_eq!(
            fold_selected_incoming_literal(
                &selected,
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
                LiteralFoldPolicy::COMPARE_V1,
                budget(),
            ),
            Err(LiteralFoldError::LiteralMismatch { function: 0 }),
            "{target:?}"
        );
        assert_eq!(
            validate_literal_fold(
                &selected,
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
                inputs.selected.plan().clone(),
            ),
            Err(LiteralFoldError::LiteralMismatch { function: 0 }),
            "{target:?} replay"
        );
    }
}

#[test]
fn compare_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    for mutation in 0..5 {
        let inputs = staged_inputs(target);
        let mut recovery = inputs.recovery.clone();
        let slot = recovery.plan.functions[0].classification.as_mut().unwrap();
        match mutation {
            0 => {
                slot.role = RecoveryVictimRole::ActiveResident {
                    current_view: register_model::RegisterViewId(0),
                    reclaimed_view: register_model::RegisterViewId(0),
                }
            }
            1 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].operand = 0;
            }
            2 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    value, ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                *value = IntegerValue::Unsigned(4096);
            }
            3 => slot.victim = VirtualRegisterId(0),
            4 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].block = SelectedBlockId(1);
            }
            _ => unreachable!(),
        }
        assert!(
            fold_selected_incoming_literal(
                &inputs.selected,
                &inputs.ranges,
                &inputs.legality,
                &inputs.spill_choices,
                &recovery,
                &inputs.availability,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &keys,
                &effect_catalog,
                LiteralFoldPolicy::COMPARE_V1,
                budget(),
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn scalar_result_shape_on_a_flag_defining_consumer_is_rejected() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let inputs = staged_inputs(target);

    // A `CompareI64` carrying a scalar `Def` operand does not match the
    // flag-defining shape its rule admits. Both the producer and the
    // independent replay must reject it rather than index past the
    // one-operand rewritten constraint row.
    let mut plan = inputs.selected.transformed().clone();
    let consumer = &mut plan.functions[0].blocks[0].instructions[1];
    let gpr = consumer.operands[0].class;
    consumer.operands.push(SelectedOperand {
        operand: 2,
        virtual_register: VirtualRegisterId(0),
        access: RegisterOperandAccess::Def,
        class: gpr,
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    });
    let mut selected = inputs.selected.clone();
    selected.transformed = Arc::new(plan);

    assert_eq!(
        fold_selected_incoming_literal(
            &selected,
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
            LiteralFoldPolicy::COMPARE_V1,
            budget(),
        ),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    assert_eq!(
        validate_literal_fold(
            &selected,
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
            inputs.selected.plan().clone(),
        ),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn compare_fold_rejects_consumer_operands_carrying_unit_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();

    // The rewrite rebuilds the consumer's operands wholesale from the
    // rewritten constraint row. An operand carrying a unit binding — a
    // fixed view, an allocation tie, or an early clobber — would have that
    // binding silently dropped, so the pair's declared unit-effect surface
    // admits only undecorated consumer operands in the producer and the
    // independent replay.
    for mutation in 0..3 {
        let inputs = staged_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        let operand = &mut plan.functions[0].blocks[0].instructions[1].operands[0];
        match mutation {
            0 => operand.fixed_view = Some(register_model::RegisterViewId(0)),
            1 => operand.tied_to = Some(0),
            2 => operand.early_clobber = true,
            _ => unreachable!(),
        }
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);

        assert_eq!(
            fold_selected_incoming_literal(
                &selected,
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
                LiteralFoldPolicy::COMPARE_V1,
                budget(),
            ),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation}"
        );
        assert_eq!(
            validate_literal_fold(
                &selected,
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
                inputs.selected.plan().clone(),
            ),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation}"
        );
    }
}

fn unsigned(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
}

fn signed(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, bits).unwrap())
}

#[test]
fn extension_elimination_folds_every_unary_consumer_to_a_materialization_on_both_targets() {
    let cases = [
        (
            SelectedInstructionKind::ZeroExtendU8,
            0x1FF_u64,
            unsigned(8),
            0xFF_u64,
            IntegerValue::Unsigned(0xFF),
        ),
        (
            SelectedInstructionKind::ZeroExtendU16,
            0x1_FFFF,
            unsigned(16),
            0xFFFF,
            IntegerValue::Unsigned(0xFFFF),
        ),
        (
            SelectedInstructionKind::ZeroExtendU32,
            0x1_FFFF_FFFF,
            unsigned(32),
            0xFFFF_FFFF,
            IntegerValue::Unsigned(0xFFFF_FFFF),
        ),
        (
            SelectedInstructionKind::SignExtendI8,
            0x80,
            signed(8),
            u64::MAX - 0x7F,
            IntegerValue::Signed(-128),
        ),
        (
            SelectedInstructionKind::SignExtendI16,
            0x8000,
            signed(16),
            u64::MAX - 0x7FFF,
            IntegerValue::Signed(-32768),
        ),
        (
            SelectedInstructionKind::SignExtendI32,
            0x8000_0000,
            signed(32),
            u64::MAX - 0x7FFF_FFFF,
            IntegerValue::Signed(-2_147_483_648),
        ),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let materialize_key = environment.allocation_constraint_keys().materialize_i64;
        for (kind, literal_constant, result_scalar, expected_bits, expected_value) in cases {
            let inputs = staged_extension_inputs(target, kind, literal_constant, result_scalar);
            let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1)
                .unwrap_or_else(|error| {
                    panic!("{kind:?} fold on {target:?} should validate: {error:?}")
                });

            assert_eq!(result.receipt().applied_count(), 1, "{kind:?}");
            let action = result.plan().functions[0].action.unwrap();
            assert_eq!(action.result, Some(VirtualRegisterId(2)), "{kind:?}");
            assert_eq!(action.immediate, expected_bits, "{kind:?}");
            assert_eq!(action.surviving, VirtualRegisterId(1), "{kind:?}");
            assert_eq!(action.victim, VirtualRegisterId(1), "{kind:?}");
            assert_eq!(
                action.literal_instruction,
                SelectedInstructionId(0),
                "{kind:?}"
            );
            assert_eq!(
                action.consumer_instruction,
                SelectedInstructionId(1),
                "{kind:?}"
            );
            assert_eq!(action.immediate_constraint, materialize_key, "{kind:?}");

            let function = &result.transformed().functions[0];
            // The victim register is removed; the extension's result register
            // shifts into its slot and keeps its scalar type.
            assert_eq!(function.virtual_registers.len(), 2, "{kind:?}");
            assert_eq!(
                function.virtual_registers[1].id,
                VirtualRegisterId(1),
                "{kind:?}"
            );
            assert_eq!(
                function.virtual_registers[1].scalar_type, result_scalar,
                "{kind:?}"
            );
            assert_eq!(
                function.virtual_registers[1].origin,
                VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(0),
                    source_value: ValueId::new(3).unwrap(),
                },
                "{kind:?}"
            );
            let instructions = &function.blocks[0].instructions;
            assert_eq!(instructions.len(), 1, "{kind:?}");
            let rewritten = &instructions[0];
            assert_eq!(rewritten.id, SelectedInstructionId(0), "{kind:?}");
            assert_eq!(
                rewritten.kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: expected_value,
                },
                "{kind:?}"
            );
            assert_eq!(rewritten.constraint, materialize_key, "{kind:?}");
            assert_eq!(rewritten.operands.len(), 1, "{kind:?}");
            assert_eq!(
                rewritten.operands[0].virtual_register,
                VirtualRegisterId(1),
                "{kind:?}"
            );
            assert_eq!(
                rewritten.operands[0].access,
                RegisterOperandAccess::Def,
                "{kind:?}"
            );
            assert!(rewritten.implicit_uses.is_empty(), "{kind:?}");
            assert!(rewritten.implicit_defs.is_empty(), "{kind:?}");
            // The folded literal's provenance joins the consumer's.
            assert_eq!(rewritten.provenance.operations.len(), 2, "{kind:?}");
        }
    }
}

#[test]
fn extension_fold_is_deterministic_and_terminal_for_unclassified_input() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_extension_inputs(
        target,
        SelectedInstructionKind::ZeroExtendU8,
        0x1FF,
        unsigned(8),
    );
    let first = fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1).unwrap();
    let second = fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1).unwrap();
    assert_eq!(first.plan(), second.plan());

    // A transformed plan carrying no admitted classification is a fixed point:
    // the fold is the identity on it, which is what the staged fixed-point
    // driver requires of its terminal attempt.
    let mut recovery = inputs.recovery.clone();
    recovery.plan.functions[0].classification = None;
    let mut selected = inputs.selected.clone();
    selected.transformed = first.shared_transformed();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let terminal = fold_selected_incoming_literal(
        &selected,
        &inputs.ranges,
        &inputs.legality,
        &inputs.spill_choices,
        &recovery,
        &inputs.availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        &effect_catalog,
        LiteralFoldPolicy::EXTENSION_V1,
        budget(),
    )
    .expect("an unclassified transformed plan is a fixed point");
    assert_eq!(terminal.receipt().applied_count(), 0);
    assert_eq!(terminal.transformed(), first.transformed());
}

#[test]
fn extension_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_extension_inputs(
        target,
        SelectedInstructionKind::SignExtendI8,
        0x80,
        signed(8),
    );
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1).unwrap();

    for mutation in 0..10 {
        let mut plan = result.plan().clone();
        match mutation {
            0 => plan.functions[0].action.as_mut().unwrap().result = None,
            1 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            2 => plan.functions[0].action.as_mut().unwrap().immediate += 1,
            3 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .consumer_instruction = SelectedInstructionId(9)
            }
            4 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .immediate_constraint
                    .variant += 1
            }
            5 => plan.functions[0].action = None,
            6 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            7 => plan.usage.candidates += 1,
            8 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            9 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn extension_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..5 {
        let mut inputs = staged_extension_inputs(
            target,
            SelectedInstructionKind::ZeroExtendU8,
            0x1FF,
            unsigned(8),
        );
        let slot = inputs.recovery.plan.functions[0]
            .classification
            .as_mut()
            .unwrap();
        match mutation {
            0 => {
                slot.role = RecoveryVictimRole::ActiveResident {
                    current_view: register_model::RegisterViewId(0),
                    reclaimed_view: register_model::RegisterViewId(0),
                }
            }
            // The binary right-operand position is the disjoint grammar of the
            // immediate forms; a unary consumer does not admit it.
            1 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].operand = 1;
            }
            2 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].instruction = SelectedInstructionId(0);
            }
            3 => slot.victim = VirtualRegisterId(0),
            4 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].block = SelectedBlockId(1);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1).map(|_| ()),
            Err(match mutation {
                0 => LiteralFoldError::UnsupportedVictimRole { function: 0 },
                1 | 4 => LiteralFoldError::FutureUseMismatch { function: 0 },
                2 => LiteralFoldError::ConsumerMismatch { function: 0 },
                _ => LiteralFoldError::LiteralMismatch { function: 0 },
            }),
            "mutation {mutation}"
        );
    }
}

#[test]
fn extension_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_extension_inputs(
        target,
        SelectedInstructionKind::ZeroExtendU8,
        0x1FF,
        unsigned(8),
    );
    // The extension consumer is unadmitted under every disjoint policy.
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        LiteralFoldPolicy::COMPARE_V1,
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And a binary-positioned consumer is unadmitted under the extension
    // policy even though its producer shape matches.
    let inputs = staged_inputs(target);
    assert_eq!(
        fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn extension_fold_rejects_result_types_that_cannot_admit_the_folded_constant() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    for mutation in 0..4 {
        let inputs = staged_extension_inputs(
            target,
            SelectedInstructionKind::ZeroExtendU8,
            0x1FF,
            unsigned(8),
        );
        let mut plan = inputs.selected.transformed().clone();
        match mutation {
            // A Boolean result register is outside the integer grammar.
            0 => plan.functions[0].virtual_registers[2].scalar_type = ScalarType::Boolean,
            // An i8 result cannot admit the folded 0xFF constant.
            1 => plan.functions[0].virtual_registers[2].scalar_type = signed(8),
            // An extension consumer without a `Def` result does not match the
            // unary grammar.
            2 => {
                let consumer = &mut plan.functions[0].blocks[0].instructions[1];
                consumer.operands.pop();
            }
            // An extra `Use` operand does not match the unary grammar either.
            _ => {
                let consumer = &mut plan.functions[0].blocks[0].instructions[1];
                consumer.operands.push(consumer.operands[0]);
            }
        }
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);

        assert!(
            fold_selected_incoming_literal(
                &selected,
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
                LiteralFoldPolicy::EXTENSION_V1,
                budget(),
            )
            .is_err(),
            "mutation {mutation}"
        );
        assert!(
            validate_literal_fold(
                &selected,
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
                inputs.selected.plan().clone(),
            )
            .is_err(),
            "mutation {mutation} replay"
        );
    }
}

#[test]
fn load8_indexed_fold_rewrites_the_index_operand_on_both_linux_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let load8_key = keys.load8.unwrap();
        let load8_row = environment.constraint(load8_key).unwrap();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_load8_indexed_inputs(target, 5);

        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1)
            .unwrap_or_else(|error| {
                panic!("indexed byte-load fold on {target:?} should validate: {error:?}")
            });

        assert_eq!(
            result.plan().machine_effect_catalog,
            effect_catalog.identity()
        );
        assert_eq!(
            result.receipt().machine_effect_catalog(),
            effect_catalog.identity()
        );
        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, Some(VirtualRegisterId(2)));
        assert_eq!(action.immediate, 5);
        // The surviving register is the base pointer the rewritten row's
        // `Use` position binds; the folded index register is removed.
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(action.immediate_constraint, load8_key);

        let function = &result.transformed().functions[0];
        assert_eq!(function.virtual_registers.len(), 2);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::Load8 { byte_offset: 5 }
        );
        assert_eq!(rewritten.constraint, load8_key);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.implicit_uses, load8_row.implicit_uses);
        assert_eq!(rewritten.implicit_defs, load8_row.implicit_defs);
        assert_eq!(rewritten.clobbers, load8_row.clobbers);
        assert_eq!(rewritten.provenance.operations.len(), 2);
    }
}

#[test]
fn load8_indexed_fold_rejects_an_unencodable_byte_offset() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The declared bound is the narrowest byte-offset field any target's
        // `Load8` encoder admits — aarch64 `ldrb`'s 12-bit unsigned offset —
        // so 4096 cannot fold into a byte offset anywhere.
        let inputs = staged_load8_indexed_inputs(target, 4096);
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1).map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
    }
}

#[test]
fn load8_indexed_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_load8_indexed_inputs(target, 5);
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1).unwrap();

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            0 => plan.functions[0].action.as_mut().unwrap().result = None,
            1 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            2 => plan.functions[0].action.as_mut().unwrap().immediate += 1,
            3 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .consumer_instruction = SelectedInstructionId(9)
            }
            4 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .immediate_constraint
                    .variant += 1
            }
            5 => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(1),
            6 => plan.functions[0].action = None,
            7 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            8 => plan.usage.candidates += 1,
            // A policy without the indexed-load bit cannot replay the fold:
            // no `Load8` row binds and the action reconstructs nothing.
            9 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            10 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn load8_indexed_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..5 {
        let mut inputs = staged_load8_indexed_inputs(target, 5);
        let slot = inputs.recovery.plan.functions[0]
            .classification
            .as_mut()
            .unwrap();
        match mutation {
            0 => {
                slot.role = RecoveryVictimRole::ActiveResident {
                    current_view: register_model::RegisterViewId(0),
                    reclaimed_view: register_model::RegisterViewId(0),
                }
            }
            // The folded operand is the index position — operand 1; the
            // base-pointer position 0 is not a foldable literal site.
            1 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].operand = 0;
            }
            2 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].instruction = SelectedInstructionId(0);
            }
            3 => slot.victim = VirtualRegisterId(0),
            4 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].block = SelectedBlockId(1);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1).map(|_| ()),
            Err(match mutation {
                0 => LiteralFoldError::UnsupportedVictimRole { function: 0 },
                1 | 4 => LiteralFoldError::FutureUseMismatch { function: 0 },
                2 => LiteralFoldError::ConsumerMismatch { function: 0 },
                _ => LiteralFoldError::LiteralMismatch { function: 0 },
            }),
            "mutation {mutation}"
        );
    }
}

#[test]
fn load8_indexed_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_load8_indexed_inputs(target, 5);
    // The indexed byte-load consumer is unadmitted under every other policy.
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        LiteralFoldPolicy::COMPARE_V1,
        LiteralFoldPolicy::EXTENSION_V1,
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And a compare-staged consumer is unadmitted under the indexed-load
    // policy even though its producer shape matches.
    let inputs = staged_inputs(target);
    assert_eq!(
        fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn load8_indexed_fold_rejects_consumer_operands_carrying_unit_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();

    // The rewrite rebuilds the indexed load's operands wholesale from the
    // `Load8` constraint row; an operand carrying a unit binding would have
    // it silently dropped, so the producer and the independent replay both
    // reject decorated consumers.
    for mutation in 0..3 {
        let inputs = staged_load8_indexed_inputs(target, 5);
        let mut plan = inputs.selected.transformed().clone();
        let operand = &mut plan.functions[0].blocks[0].instructions[1].operands[0];
        match mutation {
            0 => operand.fixed_view = Some(register_model::RegisterViewId(0)),
            1 => operand.tied_to = Some(0),
            2 => operand.early_clobber = true,
            _ => unreachable!(),
        }
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);

        assert_eq!(
            fold_selected_incoming_literal(
                &selected,
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
                LiteralFoldPolicy::LOAD8_INDEXED_V1,
                budget(),
            ),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation}"
        );
        assert_eq!(
            validate_literal_fold(
                &selected,
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
                inputs.selected.plan().clone(),
            ),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation} replay"
        );
    }
}
