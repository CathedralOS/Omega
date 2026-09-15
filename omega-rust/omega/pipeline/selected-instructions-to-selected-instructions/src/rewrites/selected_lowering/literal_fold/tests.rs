//! Firing and corruption coverage for the flag-defining compare fold.
//!
//! The upstream analysis artifacts are staged fixtures with self-consistent
//! receipt identities, matching how the spill-recovery tests stage inputs;
//! the fold's own producer and independent replay run for real.
use super::super::super::super::{
    AllocationLegalityIdentity, AllocationLegalityPlan, AllocatorAvailabilityIdentity,
    AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy, FunctionAllocationLegality,
    FunctionRecoveryClassification, FunctionSpillChoices, LiveRangeIdentity, LivenessIdentity,
    PressureRecoveryClassification, RecoveryClassification, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, RecoveryFutureUse,
    RecoveryVictimRole, SpillChoice, SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy,
};

use crate::AllocationLegalityValidationReceipt;
use crate::AllocatorAvailabilityValidationReceipt;
use crate::FunctionLiteralFold;
use crate::LiteralFoldError;
use crate::LiteralFoldIdentity;
use crate::LiteralFoldPlan;
use crate::LiteralFoldPolicy;
use crate::LiteralFoldValidationReceipt;
use crate::LiveRangeValidationReceipt;
use crate::RecoveryClassificationValidationReceipt;
use crate::SpillChoiceError;
use crate::SpillChoiceValidationReceipt;
use crate::ValidatedAllocationLegality;
use crate::ValidatedAllocatorAvailability;
use crate::ValidatedLiteralFold;
use crate::ValidatedLiveRanges;
use crate::ValidatedRecoveryClassifications;
use crate::ValidatedSpillChoices;
use crate::analyze_allocation_legality;
use crate::analyze_live_ranges;
use crate::analyze_liveness;
use crate::choose_spill_victims;
use crate::classify_pressure_recovery;
use crate::fold_selected_incoming_literal;
use crate::materialize_allocator_availability;
use crate::validate_literal_fold;
use crate::validated_machine_effect_catalog;

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

/// A `MaterializeI64` victim feeding the sole `Use` operand of a `CopyI64`
/// consumer whose `Def` result is a scalar register — the same staged shape
/// the unary extension fixture builds, with the consumer kind and the
/// copy-materialization policy bound.
fn staged_copy_inputs(
    target: NativeTarget,
    literal_constant: u64,
    result_scalar: ScalarType,
) -> Inputs {
    let mut inputs = staged_extension_inputs(
        target,
        SelectedInstructionKind::CopyI64,
        literal_constant,
        result_scalar,
    );
    inputs.selected.plan.policy = LiteralFoldPolicy::COPY_V1;
    inputs.selected.receipt.policy = LiteralFoldPolicy::COPY_V1;
    inputs
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

/// A `MaterializeI64` victim feeding the operand-1 offset position of
/// `ByteViewAddress`, with the pressure-recovery classification already
/// admitted as an `Incoming` rematerialization candidate. `VirtualRegisterId(0)`
/// is the surviving base `Use` at operand 0 and `VirtualRegisterId(2)` is the
/// `Def` result at operand 2; the fold rewrites the projection into the
/// constant-offset `AddressOffset` form.
fn staged_byte_view_address_inputs(target: NativeTarget, folded: u64) -> Inputs {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let machine = MachineId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    // `ByteViewAddress` binds the ordinary add row: its declaration resolves
    // under the add-i64 constraint key in the effect catalog.
    let byte_view_address = environment.constraint(keys.add_i64).unwrap();
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
        kind: SelectedInstructionKind::ByteViewAddress,
        constraint: byte_view_address.key,
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
        implicit_uses: byte_view_address.implicit_uses.clone(),
        implicit_defs: byte_view_address.implicit_defs.clone(),
        clobbers: byte_view_address.clobbers.clone(),
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
            policy: LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
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
            policy: LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
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

/// A `MaterializeI64` victim producing the literal `1` feeding operand 1 —
/// the divisor — of `ExactDivideU64`, whose operand-2 `Def` result is
/// `VirtualRegisterId(2)`, with the pressure-recovery classification already
/// admitted as an `Incoming` rematerialization candidate. The consumer's
/// operand decorations come from the target's real divide row: on x86-64
/// that is the pinned `div` form whose operand 3 `Use` is the high-half
/// dividend, staged as a `VirtualRegisterId(3)` scratch defined by a
/// `MaterializeI64(0)` emitted immediately before the literal — the shape
/// selected construction produces; aarch64's `udiv` row carries no
/// auxiliary `Use` and stages the bare three-operand form.
fn staged_divide_inputs(target: NativeTarget) -> Inputs {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let machine = MachineId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let divide = environment.constraint(keys.divide_u64).unwrap();
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
    // The operand grammar fixes positions 0 through 2 — dividend `Use`,
    // folded divisor `Use`, result `Def`; every `Use` operand past the
    // result is an auxiliary input the fold drops, staged with its own
    // zero-materializing scratch definition.
    let auxiliary_uses = divide.operands.len() - 3;
    let literal_id = SelectedInstructionId(u32::try_from(auxiliary_uses).unwrap());
    let consumer_id = SelectedInstructionId(literal_id.0 + 1);
    let auxiliaries = (0..auxiliary_uses)
        .map(|auxiliary| {
            let register = VirtualRegisterId(u32::try_from(3 + auxiliary).unwrap());
            (
                register,
                SelectedInstruction {
                    id: SelectedInstructionId(u32::try_from(auxiliary).unwrap()),
                    kind: SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(0),
                    },
                    constraint: materialize.key,
                    operands: vec![SelectedOperand {
                        operand: materialize.operands[0].operand,
                        virtual_register: register,
                        access: materialize.operands[0].access,
                        class: gpr,
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    }],
                    implicit_uses: materialize.implicit_uses.clone(),
                    implicit_defs: materialize.implicit_defs.clone(),
                    clobbers: materialize.clobbers.clone(),
                    provenance: Default::default(),
                },
            )
        })
        .collect::<Vec<_>>();
    let literal = SelectedInstruction {
        id: literal_id,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(1),
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
        id: consumer_id,
        kind: SelectedInstructionKind::ExactDivideU64 {
            obligation: ObligationId::new(7).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
        },
        constraint: divide.key,
        operands: divide
            .operands
            .iter()
            .map(|operand| SelectedOperand {
                operand: operand.operand,
                virtual_register: VirtualRegisterId(u32::from(operand.operand)),
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: divide.implicit_uses.clone(),
        implicit_defs: divide.implicit_defs.clone(),
        clobbers: divide.clobbers.clone(),
        provenance: SelectedInstructionProvenance {
            operations: vec![OperationId::new(2).unwrap()],
            values: vec![ValueId::new(1).unwrap()],
            obligations: vec![ObligationId::new(7).unwrap()],
            ..Default::default()
        },
    };
    let branch_instruction = SelectedInstruction {
        id: SelectedInstructionId(consumer_id.0 + 1),
        kind: SelectedInstructionKind::ConditionalBranchNonZero,
        constraint: branch.key,
        operands: Vec::new(),
        implicit_uses: branch.implicit_uses.clone(),
        implicit_defs: branch.implicit_defs.clone(),
        clobbers: branch.clobbers.clone(),
        provenance: Default::default(),
    };
    let return_instruction = SelectedInstruction {
        id: SelectedInstructionId(consumer_id.0 + 2),
        kind: SelectedInstructionKind::ReturnUnit,
        constraint: terminal.key,
        operands: Vec::new(),
        implicit_uses: terminal.implicit_uses.clone(),
        implicit_defs: terminal.implicit_defs.clone(),
        clobbers: terminal.clobbers.clone(),
        provenance: Default::default(),
    };
    let mut virtual_registers = vec![
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
                instruction: literal_id,
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
                instruction: consumer_id,
                source_value: ValueId::new(3).unwrap(),
            },
            definition_site: Some(ValueDefinitionSite::Node {
                block: source_block,
                node: 1,
            }),
            entry_fixed_view: None,
        },
    ];
    virtual_registers.extend(
        auxiliaries
            .iter()
            .map(|(register, instruction)| VirtualRegister {
                id: *register,
                scalar_type: scalar,
                class: gpr,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: instruction.id,
                    operand: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            }),
    );
    let mut block_instructions = auxiliaries
        .iter()
        .map(|(_, instruction)| instruction.clone())
        .collect::<Vec<_>>();
    block_instructions.extend([literal, consumer]);
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
            virtual_registers,
            blocks: vec![
                SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(source_block),
                    instructions: block_instructions,
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
            policy: LiteralFoldPolicy::EXACT_DIVIDE_V1,
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
            policy: LiteralFoldPolicy::EXACT_DIVIDE_V1,
            usage: usage(),
            function_count: 1,
            applied_count: 0,
        },
    };

    let occurrence = |position: u32,
                      instruction: SelectedInstructionId,
                      operand: u16,
                      access: RegisterOperandAccess| VirtualOccurrence {
        position: LivenessPosition(position),
        point: LiveRangePoint(position),
        instruction,
        operand,
        access,
    };
    let fragment = |start: u32, end: u32| LiveRangeFragment {
        block: SelectedBlockId(0),
        start: LiveRangePoint(start),
        end: LiveRangePoint(end),
    };
    let live = |virtual_register, occurrences, fragments| VirtualLiveRange {
        virtual_register,
        class: gpr,
        occurrences,
        fixed_constraints: Vec::new(),
        fragments,
        edge_connectors: Vec::new(),
    };
    let consumer_point = consumer_id.0;
    let mut virtual_live_ranges = vec![
        live(
            VirtualRegisterId(0),
            vec![occurrence(
                consumer_point,
                consumer_id,
                0,
                RegisterOperandAccess::Use,
            )],
            vec![fragment(0, consumer_point + 1)],
        ),
        live(
            VirtualRegisterId(1),
            vec![
                occurrence(literal_id.0, literal_id, 0, RegisterOperandAccess::Def),
                occurrence(consumer_point, consumer_id, 1, RegisterOperandAccess::Use),
            ],
            vec![fragment(literal_id.0, consumer_point + 1)],
        ),
        live(
            VirtualRegisterId(2),
            vec![occurrence(
                consumer_point,
                consumer_id,
                2,
                RegisterOperandAccess::Def,
            )],
            vec![fragment(consumer_point, consumer_point + 1)],
        ),
    ];
    virtual_live_ranges.extend(auxiliaries.iter().enumerate().map(
        |(index, (register, instruction))| {
            let position = u32::try_from(index).unwrap();
            live(
                *register,
                vec![
                    occurrence(position, instruction.id, 0, RegisterOperandAccess::Def),
                    occurrence(
                        consumer_point,
                        consumer_id,
                        u16::try_from(3 + index).unwrap(),
                        RegisterOperandAccess::Use,
                    ),
                ],
                vec![fragment(position, consumer_point + 1)],
            )
        },
    ));
    let register_count = 3 + auxiliary_uses;
    let occurrence_count = 4 + auxiliary_uses * 2;
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
                        end: LiveRangePoint(consumer_point + 1),
                    },
                    BlockPointDomain {
                        block: SelectedBlockId(1),
                        source_block: BlockId::new(2).unwrap(),
                        start: LiveRangePoint(consumer_point + 1),
                        end: LiveRangePoint(consumer_point + 3),
                    },
                ],
                virtual_registers: virtual_live_ranges,
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
            virtual_register_count: register_count,
            virtual_occurrence_count: occurrence_count,
            fixed_constraint_count: 0,
            virtual_fragment_count: register_count,
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
                    point: LiveRangePoint(consumer_point),
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
                    point: LiveRangePoint(consumer_point),
                    victim: VirtualRegisterId(1),
                    role: RecoveryVictimRole::Incoming,
                    scalar_type: scalar,
                    class: gpr,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: literal_id,
                        source_value: literal_value,
                    },
                    definition_site: Some(ValueDefinitionSite::Node {
                        block: source_block,
                        node: 0,
                    }),
                    classification:
                        RecoveryClassification::ImmediateU64RematerializationCandidate {
                            defining_instruction: literal_id,
                            source_value: literal_value,
                            value: IntegerValue::Unsigned(1),
                            provenance: literal_provenance,
                            future_uses: vec![RecoveryFutureUse {
                                block: SelectedBlockId(0),
                                point: LiveRangePoint(consumer_point),
                                instruction: consumer_id,
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

/// A `MaterializeI64` victim producing the literal `1` feeding operand 1 —
/// the divisor — of `WrappingRemainderI64`, whose operand-2 `Def` result is
/// `VirtualRegisterId(2)`, with the pressure-recovery classification already
/// admitted as an `Incoming` rematerialization candidate. The consumer's
/// operand decorations come from the target's real remainder row: on x86-64
/// that is the pinned `idiv` form — the operand-0 `Use` and operand-2 `Def`
/// result both pinned to `rax`, plus the operand-3 early-clobber `Def`
/// quotient scratch pinned to `rdx`, staged as `VirtualRegisterId(3)` —
/// while aarch64's `udiv`/`msub` row carries only an early-clobber
/// operand-2 `Def` and no scratch tail. Every `Def` operand past the result
/// is a scratch output the fold drops; the fixture gives each a register
/// occurring nowhere else in the function, which is the dead-definition
/// custody the grammar requires.
fn staged_remainder_inputs(target: NativeTarget) -> Inputs {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let machine = MachineId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let remainder = environment.constraint(keys.remainder_i64).unwrap();
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
    // The operand grammar fixes positions 0 through 2 — dividend `Use`,
    // folded divisor `Use`, result `Def`; every `Def` operand past the
    // result is a scratch output the fold drops, staged as a register the
    // consumer alone defines.
    let scratch_defs = remainder.operands.len() - 3;
    let literal_id = SelectedInstructionId(0);
    let consumer_id = SelectedInstructionId(1);
    let literal = SelectedInstruction {
        id: literal_id,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(1),
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
        id: consumer_id,
        kind: SelectedInstructionKind::WrappingRemainderI64 {
            obligation: ObligationId::new(7).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
        },
        constraint: remainder.key,
        operands: remainder
            .operands
            .iter()
            .map(|operand| SelectedOperand {
                operand: operand.operand,
                virtual_register: VirtualRegisterId(u32::from(operand.operand)),
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: remainder.implicit_uses.clone(),
        implicit_defs: remainder.implicit_defs.clone(),
        clobbers: remainder.clobbers.clone(),
        provenance: SelectedInstructionProvenance {
            operations: vec![OperationId::new(2).unwrap()],
            values: vec![ValueId::new(1).unwrap()],
            obligations: vec![ObligationId::new(7).unwrap()],
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
    let mut virtual_registers = vec![
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
                instruction: literal_id,
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
                instruction: consumer_id,
                source_value: ValueId::new(3).unwrap(),
            },
            definition_site: Some(ValueDefinitionSite::Node {
                block: source_block,
                node: 1,
            }),
            entry_fixed_view: None,
        },
    ];
    virtual_registers.extend((0..scratch_defs).map(|scratch| VirtualRegister {
        id: VirtualRegisterId(u32::try_from(3 + scratch).unwrap()),
        scalar_type: scalar,
        class: gpr,
        origin: VirtualRegisterOrigin::InstructionScratch {
            instruction: consumer_id,
            operand: u16::try_from(3 + scratch).unwrap(),
        },
        definition_site: None,
        entry_fixed_view: None,
    }));
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
            virtual_registers,
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
            policy: LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
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
            policy: LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
            usage: usage(),
            function_count: 1,
            applied_count: 0,
        },
    };

    let occurrence = |position: u32,
                      instruction: SelectedInstructionId,
                      operand: u16,
                      access: RegisterOperandAccess| VirtualOccurrence {
        position: LivenessPosition(position),
        point: LiveRangePoint(position),
        instruction,
        operand,
        access,
    };
    let fragment = |start: u32, end: u32| LiveRangeFragment {
        block: SelectedBlockId(0),
        start: LiveRangePoint(start),
        end: LiveRangePoint(end),
    };
    let live = |virtual_register, occurrences, fragments| VirtualLiveRange {
        virtual_register,
        class: gpr,
        occurrences,
        fixed_constraints: Vec::new(),
        fragments,
        edge_connectors: Vec::new(),
    };
    let consumer_point = consumer_id.0;
    let mut virtual_live_ranges = vec![
        live(
            VirtualRegisterId(0),
            vec![occurrence(
                consumer_point,
                consumer_id,
                0,
                RegisterOperandAccess::Use,
            )],
            vec![fragment(0, consumer_point + 1)],
        ),
        live(
            VirtualRegisterId(1),
            vec![
                occurrence(literal_id.0, literal_id, 0, RegisterOperandAccess::Def),
                occurrence(consumer_point, consumer_id, 1, RegisterOperandAccess::Use),
            ],
            vec![fragment(literal_id.0, consumer_point + 1)],
        ),
        live(
            VirtualRegisterId(2),
            vec![occurrence(
                consumer_point,
                consumer_id,
                2,
                RegisterOperandAccess::Def,
            )],
            vec![fragment(consumer_point, consumer_point + 1)],
        ),
    ];
    virtual_live_ranges.extend((0..scratch_defs).map(|scratch| {
        live(
            VirtualRegisterId(u32::try_from(3 + scratch).unwrap()),
            vec![occurrence(
                consumer_point,
                consumer_id,
                u16::try_from(3 + scratch).unwrap(),
                RegisterOperandAccess::Def,
            )],
            vec![fragment(consumer_point, consumer_point + 1)],
        )
    }));
    let register_count = 3 + scratch_defs;
    let occurrence_count = 4 + scratch_defs;
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
                        end: LiveRangePoint(consumer_point + 1),
                    },
                    BlockPointDomain {
                        block: SelectedBlockId(1),
                        source_block: BlockId::new(2).unwrap(),
                        start: LiveRangePoint(consumer_point + 1),
                        end: LiveRangePoint(consumer_point + 3),
                    },
                ],
                virtual_registers: virtual_live_ranges,
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
            virtual_register_count: register_count,
            virtual_occurrence_count: occurrence_count,
            fixed_constraint_count: 0,
            virtual_fragment_count: register_count,
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
                    point: LiveRangePoint(consumer_point),
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
                    point: LiveRangePoint(consumer_point),
                    victim: VirtualRegisterId(1),
                    role: RecoveryVictimRole::Incoming,
                    scalar_type: scalar,
                    class: gpr,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: literal_id,
                        source_value: literal_value,
                    },
                    definition_site: Some(ValueDefinitionSite::Node {
                        block: source_block,
                        node: 0,
                    }),
                    classification:
                        RecoveryClassification::ImmediateU64RematerializationCandidate {
                            defining_instruction: literal_id,
                            source_value: literal_value,
                            value: IntegerValue::Unsigned(1),
                            provenance: literal_provenance,
                            future_uses: vec![RecoveryFutureUse {
                                block: SelectedBlockId(0),
                                point: LiveRangePoint(consumer_point),
                                instruction: consumer_id,
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
    ]
    .into_iter()
    .filter(|policy| *policy != disabled)
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
fn compare_fold_admits_the_u12_boundary_immediate_and_rejects_beyond_it() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The declared bound is the narrowest immediate any target's
        // compare-immediate encoder admits — the shared 12-bit encoding
        // limit: 4095 folds and 4096 cannot.
        let mut inputs = staged_inputs(target);
        restage_literal(&mut inputs, 4095);
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1)
            .expect("the boundary immediate folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(4095),
            }
        );

        let mut inputs = staged_inputs(target);
        restage_literal(&mut inputs, 4096);
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1).map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
    }
}

#[test]
fn compare_fold_is_disabled_without_the_compare_bit() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_inputs(target);
        // Every other selected-lowering family enabled — and the fully empty
        // policy — both leave the compare bit's gate closed: the fold cannot
        // fire without it.
        for policy in [
            policy_without(LiteralFoldPolicy::COMPARE_V1),
            LiteralFoldPolicy::empty(),
        ] {
            assert_eq!(
                fold_with(&inputs, &environment, policy).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "{target:?}"
            );
        }
    }
}

#[test]
fn compare_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_inputs(target);
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1);
    }
}

#[test]
fn compare_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_inputs(target);
        assert_deterministic_fixed_point(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1);
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
fn subtract_fold_admits_the_u12_boundary_immediate_and_rejects_beyond_it() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The declared bound is the narrowest immediate any target's
        // subtract-immediate encoder admits — the shared 12-bit encoding
        // limit: 4095 folds and 4096 cannot.
        let mut inputs = staged_subtract_inputs(target);
        restage_literal(&mut inputs, 4095);
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1)
            .expect("the boundary immediate folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::ExactSubtractI64Immediate {
                immediate: IntegerValue::Unsigned(4095),
                obligation: ObligationId::new(7).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
            }
        );

        let mut inputs = staged_subtract_inputs(target);
        restage_literal(&mut inputs, 4096);
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
    }
}

#[test]
fn subtract_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..6 {
        let mut inputs = staged_subtract_inputs(target);
        let slot = inputs.recovery.plan.functions[0]
            .classification
            .as_mut()
            .unwrap();
        let (producer_expected, replay_expected) = match mutation {
            // Only an `Incoming` victim may fold; a reclaimed resident is not
            // the operand the consumer reads.
            0 => {
                slot.role = RecoveryVictimRole::ActiveResident {
                    current_view: register_model::RegisterViewId(0),
                    reclaimed_view: register_model::RegisterViewId(0),
                };
                (
                    LiteralFoldError::UnsupportedVictimRole { function: 0 },
                    LiteralFoldError::UnsupportedVictimRole { function: 0 },
                )
            }
            // The subtract grammar folds the literal at operand 1 only:
            // claiming the left `Use` is a position no admitted grammar
            // covers.
            1 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].operand = 0;
                (
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                )
            }
            // An over-bound immediate fails admission in the producer; the
            // replay first refuses the literal record itself, since it no
            // longer materializes the value the classification carries.
            2 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    value, ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                *value = IntegerValue::Unsigned(4096);
                (
                    LiteralFoldError::UnsupportedImmediate { function: 0 },
                    LiteralFoldError::LiteralMismatch { function: 0 },
                )
            }
            // The classified victim must be the register the literal record
            // defines and the claimed `Use` operand binds.
            3 => {
                slot.victim = VirtualRegisterId(0);
                (
                    LiteralFoldError::LiteralMismatch { function: 0 },
                    LiteralFoldError::LiteralMismatch { function: 0 },
                )
            }
            // The use must sit in the classified block itself.
            4 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].block = SelectedBlockId(1);
                (
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                )
            }
            // A second recorded use means the literal is not single-use.
            _ => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                let extra = future_uses[0];
                future_uses.push(extra);
                (
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                )
            }
        };
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).map(|_| ()),
            Err(producer_expected),
            "mutation {mutation}"
        );
        assert_eq!(
            validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
            Err(replay_expected),
            "mutation {mutation} replay"
        );
    }
}

#[test]
fn subtract_fold_is_disabled_without_the_subtract_bit() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_subtract_inputs(target);
        // Every other selected-lowering family enabled — and the fully empty
        // policy — both leave the subtract bit's gate closed: the fold cannot
        // fire without it.
        for policy in [
            policy_without(LiteralFoldPolicy::EXACT_SUBTRACT_V1),
            LiteralFoldPolicy::empty(),
        ] {
            assert_eq!(
                fold_with(&inputs, &environment, policy).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "{target:?}"
            );
        }
    }
}

#[test]
fn subtract_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_subtract_inputs(target);
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).unwrap();

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            // The subtract's `Def` result is decision-bearing: dropping it or
            // rebinding it to the surviving operand replays differently.
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
            // The rewritten `Use` must bind the surviving register, not the
            // removed victim.
            5 => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(1),
            6 => plan.functions[0].action.as_mut().unwrap().victim = VirtualRegisterId(0),
            7 => plan.functions[0].action = None,
            8 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            9 => plan.usage.candidates += 1,
            // A policy without the subtract bit cannot replay the fold: no
            // subtract-immediate row binds and the action reconstructs
            // nothing.
            10 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn subtract_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_subtract_inputs(target);
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1);
    }
}

#[test]
fn subtract_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_subtract_inputs(target);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        );
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
fn add_fold_admits_the_u12_boundary_immediate_and_rejects_beyond_it() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The declared bound is the narrowest immediate any target's
        // add-immediate encoder admits — the shared 12-bit encoding limit:
        // 4095 folds and 4096 cannot, under either operand grammar.
        for literal_operand in [0u16, 1u16] {
            let mut inputs = staged_add_inputs(target, literal_operand);
            restage_literal(&mut inputs, 4095);
            let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1)
                .expect("the boundary immediate folds");
            assert_eq!(result.receipt().applied_count(), 1);
            assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
            assert_eq!(
                result.transformed().functions[0].blocks[0].instructions[0].kind,
                SelectedInstructionKind::ExactAddI64Immediate {
                    immediate: IntegerValue::Unsigned(4095),
                    obligation: ObligationId::new(7).unwrap(),
                    accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
                }
            );

            let mut inputs = staged_add_inputs(target, literal_operand);
            restage_literal(&mut inputs, 4096);
            assert_eq!(
                fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1).map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "literal at operand {literal_operand} on {target:?}"
            );
        }
    }
}

#[test]
fn add_fold_is_disabled_without_the_add_bit() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Every other selected-lowering family enabled — and the fully empty
        // policy — both leave the add bit's gate closed: neither operand
        // grammar can fire without it.
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_add_inputs(target, literal_operand);
            for policy in [
                policy_without(LiteralFoldPolicy::EXACT_ADD_V1),
                LiteralFoldPolicy::empty(),
            ] {
                assert_eq!(
                    fold_with(&inputs, &environment, policy).map(|_| ()),
                    Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                    "literal at operand {literal_operand} on {target:?}"
                );
            }
        }
    }
}

#[test]
fn add_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_add_inputs(target, literal_operand);
            assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1);
        }
    }
}

#[test]
fn add_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_add_inputs(target, literal_operand);
            assert_deterministic_fixed_point(
                &inputs,
                &environment,
                LiteralFoldPolicy::EXACT_ADD_V1,
            );
        }
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
fn extension_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_extension_inputs(
            target,
            SelectedInstructionKind::ZeroExtendU8,
            0x1FF,
            unsigned(8),
        );
        assert_deterministic_fixed_point(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1);
    }
}

#[test]
fn extension_fold_is_the_identity_when_no_candidate_is_classified() {
    // An input carrying no admitted classification is a fixed point: the fold
    // is the identity on it, which is what the staged fixed-point driver
    // requires of its terminal attempt. The classification is a legitimate
    // "not a candidate" verdict on the unchanged input custody.
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let mut inputs = staged_extension_inputs(
            target,
            SelectedInstructionKind::SignExtendI8,
            0x80,
            signed(8),
        );
        inputs.recovery.plan.functions[0].classification = None;
        let terminal = fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1)
            .expect("an unclassified input is a fixed point");
        assert_eq!(terminal.receipt().applied_count(), 0);
        assert_eq!(
            terminal.receipt().source_selected(),
            terminal.receipt().transformed_selected()
        );
        assert_eq!(terminal.transformed(), inputs.selected.transformed());
    }
}

#[test]
fn extension_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_extension_inputs(
            target,
            SelectedInstructionKind::ZeroExtendU8,
            0x1FF,
            unsigned(8),
        );
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1);
    }
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
fn copy_materialization_folds_the_unary_copy_to_a_materialization_on_both_targets() {
    // The copy fold admits the full u64 literal domain — including values no
    // u12 immediate grammar could carry — and preserves the literal at the
    // copy's destination register.
    let cases = [
        (0x1FF_u64, unsigned(64), IntegerValue::Unsigned(0x1FF)),
        (
            0x1_0000_0001,
            unsigned(64),
            IntegerValue::Unsigned(0x1_0000_0001),
        ),
        (u64::MAX, signed(64), IntegerValue::Signed(-1)),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let materialize_key = environment.allocation_constraint_keys().materialize_i64;
        for (literal_constant, result_scalar, expected_value) in cases {
            let inputs = staged_copy_inputs(target, literal_constant, result_scalar);
            let result = fold_with(&inputs, &environment, LiteralFoldPolicy::COPY_V1)
                .unwrap_or_else(|error| {
                    panic!("copy fold on {target:?} should validate: {error:?}")
                });

            assert_eq!(result.receipt().applied_count(), 1);
            let action = result.plan().functions[0].action.unwrap();
            assert_eq!(action.result, Some(VirtualRegisterId(2)));
            assert_eq!(action.immediate, literal_constant);
            assert_eq!(action.surviving, VirtualRegisterId(1));
            assert_eq!(action.victim, VirtualRegisterId(1));
            assert_eq!(action.literal_instruction, SelectedInstructionId(0));
            assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
            assert_eq!(action.immediate_constraint, materialize_key);

            let function = &result.transformed().functions[0];
            // The victim register is removed; the copy's result register
            // shifts into its slot and keeps its scalar type.
            assert_eq!(function.virtual_registers.len(), 2);
            assert_eq!(function.virtual_registers[1].id, VirtualRegisterId(1));
            assert_eq!(function.virtual_registers[1].scalar_type, result_scalar);
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
                SelectedInstructionKind::MaterializeI64 {
                    value: expected_value,
                }
            );
            assert_eq!(rewritten.constraint, materialize_key);
            assert_eq!(rewritten.operands.len(), 1);
            assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(1));
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Def);
            assert!(rewritten.implicit_uses.is_empty());
            assert!(rewritten.implicit_defs.is_empty());
            // The folded literal's provenance joins the consumer's.
            assert_eq!(rewritten.provenance.operations.len(), 2);
        }
    }
}

#[test]
fn copy_materialization_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_copy_inputs(target, 0x1_0000_0001, unsigned(64));
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::COPY_V1).unwrap();

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
            8 => plan.policy = LiteralFoldPolicy::EXTENSION_V1,
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
fn copy_materialization_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..5 {
        let mut inputs = staged_copy_inputs(target, 0x1FF, unsigned(64));
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
            // The binary right-operand position is the disjoint grammar of
            // the immediate forms; a unary copy consumer does not admit it.
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
            fold_with(&inputs, &environment, LiteralFoldPolicy::COPY_V1).map(|_| ()),
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
fn copy_materialization_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_copy_inputs(target, 0x1FF, unsigned(64));
    // The copy consumer is unadmitted under every disjoint policy.
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        LiteralFoldPolicy::COMPARE_V1,
        LiteralFoldPolicy::EXTENSION_V1,
        LiteralFoldPolicy::LOAD8_INDEXED_V1,
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And the unary extension consumers stay unadmitted under the copy
    // policy even though their producer shape is identical: the grammars are
    // disjoint.
    for kind in [
        SelectedInstructionKind::ZeroExtendU8,
        SelectedInstructionKind::SignExtendI32,
    ] {
        let inputs = staged_extension_inputs(target, kind, 0x1FF, unsigned(8));
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::COPY_V1).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{kind:?}"
        );
    }
    // A binary-positioned consumer is likewise unadmitted under the copy
    // policy.
    let inputs = staged_inputs(target);
    assert_eq!(
        fold_with(&inputs, &environment, LiteralFoldPolicy::COPY_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn copy_materialization_fold_rejects_result_types_that_cannot_admit_the_literal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    for mutation in 0..4 {
        let inputs = staged_copy_inputs(target, 0x1FF, unsigned(64));
        let mut plan = inputs.selected.transformed().clone();
        match mutation {
            // A Boolean result register is outside the integer grammar.
            0 => plan.functions[0].virtual_registers[2].scalar_type = ScalarType::Boolean,
            // Unlike the extension fold, the copy cannot narrow: a u8 result
            // cannot admit the unfolded 0x1FF literal.
            1 => plan.functions[0].virtual_registers[2].scalar_type = unsigned(8),
            // A copy consumer without a `Def` result does not match the
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
                LiteralFoldPolicy::COPY_V1,
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
fn copy_materialization_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_copy_inputs(target, 0x1FF, unsigned(64));
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::COPY_V1);
    }
}

#[test]
fn copy_materialization_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_copy_inputs(target, 0x1FF, unsigned(64));
        assert_deterministic_fixed_point(&inputs, &environment, LiteralFoldPolicy::COPY_V1);
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
fn load8_indexed_fold_admits_the_widest_encodable_byte_offset() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // 4095 is the widest offset the shared 12-bit byte-offset bound
        // admits — the boundary the unencodable 4096 sits just past.
        let inputs = staged_load8_indexed_inputs(target, 4095);
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1)
            .expect("the boundary offset folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::Load8 { byte_offset: 4095 }
        );
    }
}

#[test]
fn load8_indexed_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_load8_indexed_inputs(target, 5);
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1);
    }
}

#[test]
fn load8_indexed_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_load8_indexed_inputs(target, 5);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::LOAD8_INDEXED_V1,
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

#[test]
fn byte_view_address_fold_rewrites_the_offset_operand_on_both_linux_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let address_offset_key = keys.address_offset.unwrap();
        let address_offset_row = environment.constraint(address_offset_key).unwrap();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_byte_view_address_inputs(target, 5);

        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        )
        .unwrap_or_else(|error| {
            panic!("byte-view address fold on {target:?} should validate: {error:?}")
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
        // The surviving register is the base the rewritten row's `Use`
        // position binds; the folded offset register is removed.
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(action.immediate_constraint, address_offset_key);

        let function = &result.transformed().functions[0];
        assert_eq!(function.virtual_registers.len(), 2);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::AddressOffset { byte_offset: 5 }
        );
        assert_eq!(rewritten.constraint, address_offset_key);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.implicit_uses, address_offset_row.implicit_uses);
        assert_eq!(rewritten.implicit_defs, address_offset_row.implicit_defs);
        assert_eq!(rewritten.clobbers, address_offset_row.clobbers);
        assert_eq!(rewritten.provenance.operations.len(), 2);
    }
}

#[test]
fn byte_view_address_fold_rejects_an_unencodable_byte_offset() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The declared bound is the narrowest byte-offset field any target's
        // `AddressOffset` encoder admits — aarch64 `add`'s 12-bit unsigned
        // immediate — so 4096 cannot fold into a byte offset anywhere.
        let inputs = staged_byte_view_address_inputs(target, 4096);
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1
            )
            .map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
    }
}

#[test]
fn byte_view_address_fold_admits_the_widest_encodable_byte_offset() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // 4095 is the widest offset the shared 12-bit byte-offset bound
        // admits — the boundary the unencodable 4096 sits just past.
        let inputs = staged_byte_view_address_inputs(target, 4095);
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        )
        .expect("the boundary offset folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::AddressOffset { byte_offset: 4095 }
        );
    }
}

#[test]
fn byte_view_address_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_byte_view_address_inputs(target, 5);
        assert_budget_is_enforced(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        );
    }
}

#[test]
fn byte_view_address_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_byte_view_address_inputs(target, 5);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        );
    }
}

#[test]
fn byte_view_address_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_byte_view_address_inputs(target, 5);
    let result = fold_with(
        &inputs,
        &environment,
        LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
    )
    .unwrap();

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
            // A policy without the byte-view-address bit cannot replay the
            // fold: no `AddressOffset` row binds and the action reconstructs
            // nothing.
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
fn byte_view_address_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..5 {
        let mut inputs = staged_byte_view_address_inputs(target, 5);
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
            // The folded operand is the offset position — operand 1; the
            // base position 0 is not a foldable literal site.
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
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1
            )
            .map(|_| ()),
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
fn byte_view_address_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_byte_view_address_inputs(target, 5);
    // The byte-view address consumer is unadmitted under every other policy.
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        LiteralFoldPolicy::COMPARE_V1,
        LiteralFoldPolicy::EXTENSION_V1,
        LiteralFoldPolicy::LOAD8_INDEXED_V1,
        LiteralFoldPolicy::COPY_V1,
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And a compare-staged consumer is unadmitted under the byte-view
    // address policy even though its producer shape matches.
    let inputs = staged_inputs(target);
    assert_eq!(
        fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn byte_view_address_fold_rejects_consumer_operands_carrying_unit_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();

    // The rewrite rebuilds the projection's operands wholesale from the
    // `AddressOffset` constraint row; an operand carrying a unit binding
    // would have it silently dropped, so the producer and the independent
    // replay both reject decorated consumers.
    for mutation in 0..3 {
        let inputs = staged_byte_view_address_inputs(target, 5);
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
                LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
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

#[test]
fn exact_divide_identity_fold_rewrites_the_divide_to_a_copy_on_both_linux_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let inputs = staged_divide_inputs(target);
        let auxiliary_uses = environment
            .constraint(keys.divide_u64)
            .unwrap()
            .operands
            .len()
            - 3;
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_DIVIDE_V1)
            .expect("the staged divide fold should validate");

        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, Some(VirtualRegisterId(2)));
        assert_eq!(action.immediate, 1);
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(
            action.literal_instruction,
            SelectedInstructionId(auxiliary_uses as u32)
        );
        assert_eq!(
            action.consumer_instruction,
            SelectedInstructionId(auxiliary_uses as u32 + 1)
        );
        assert_eq!(action.immediate_constraint, keys.copy_i64);

        let function = &result.transformed().functions[0];
        // The fold removes only the divisor literal and its register: every
        // auxiliary scratch materialization and register stays, left dead.
        assert_eq!(function.virtual_registers.len(), 2 + auxiliary_uses);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1 + auxiliary_uses);
        for (index, instruction) in instructions[..auxiliary_uses].iter().enumerate() {
            assert_eq!(instruction.id, SelectedInstructionId(index as u32));
            assert_eq!(
                instruction.kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(0),
                }
            );
        }
        let rewritten = &instructions[auxiliary_uses];
        assert_eq!(rewritten.id, SelectedInstructionId(auxiliary_uses as u32));
        assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(rewritten.constraint, keys.copy_i64);
        assert_eq!(rewritten.operands.len(), 2);
        // The rebuilt operand list binds the dividend `Use` and the result
        // `Def` — the register pins and the dropped auxiliary `Use` are
        // gone with the pinned divide form.
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[0].fixed_view, None);
        assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.operands[1].fixed_view, None);
        assert!(rewritten.implicit_uses.is_empty());
        assert!(rewritten.implicit_defs.is_empty());
        assert!(rewritten.clobbers.is_empty());
        // The folded literal's provenance joins the consumer's, and the
        // divide's obligation custody is retained.
        assert_eq!(rewritten.provenance.operations.len(), 2);
        assert_eq!(
            rewritten.provenance.obligations,
            vec![ObligationId::new(7).unwrap()]
        );

        let SelectedTerminator::ConditionalBranch { instruction, .. } =
            &function.blocks[0].terminator
        else {
            panic!("conditional branch terminator retained");
        };
        assert_eq!(
            instruction.id,
            SelectedInstructionId(auxiliary_uses as u32 + 1)
        );
        let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator else {
            panic!("return terminator retained");
        };
        assert_eq!(
            instruction.id,
            SelectedInstructionId(auxiliary_uses as u32 + 2)
        );
    }
}

#[test]
fn divide_fold_rejects_a_non_unit_divisor() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_divide_inputs(target);
        let auxiliary_uses = environment
            .constraint(keys.divide_u64)
            .unwrap()
            .operands
            .len()
            - 3;
        let mut plan = inputs.selected.transformed().clone();
        // The literal is the divisor only under the identity fold: a divisor
        // of two is a different computation both the producer's declared
        // bound and the replay's re-derived grammar reject.
        plan.functions[0].blocks[0].instructions[auxiliary_uses].kind =
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(2),
            };
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);
        let mut recovery = inputs.recovery.clone();
        let classification = recovery.plan.functions[0].classification.as_mut().unwrap();
        let RecoveryClassification::ImmediateU64RematerializationCandidate { value, .. } =
            &mut classification.classification
        else {
            panic!("staged classification is the immediate candidate");
        };
        *value = IntegerValue::Unsigned(2);

        assert_eq!(
            fold_selected_incoming_literal(
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
                &keys,
                &effect_catalog,
                LiteralFoldPolicy::EXACT_DIVIDE_V1,
                budget(),
            ),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
        assert_eq!(
            validate_literal_fold(
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
                &keys,
                &effect_catalog,
                inputs.selected.plan().clone(),
            ),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?} replay"
        );
    }
}

#[test]
fn divide_fold_rejects_an_auxiliary_operand_without_zero_custody() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let divide_operands = environment
        .constraint(keys.divide_u64)
        .unwrap()
        .operands
        .len();
    assert_eq!(
        divide_operands, 4,
        "the x86-64 divide carries one auxiliary use"
    );

    // The dropped operand is inert only when its register is defined solely
    // by zero materializations: a nonzero materialization, a non-materialize
    // definition, and a `Def`-access auxiliary operand each reject — the
    // producer and the independent replay alike.
    for mutation in 0..3 {
        let inputs = staged_divide_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        match mutation {
            // The high-half scratch materializes seven, not zero.
            0 => {
                plan.functions[0].blocks[0].instructions[0].kind =
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(7),
                    };
            }
            // The scratch register is defined by a copy, not a zero
            // materialization at all.
            1 => {
                plan.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::CopyI64;
            }
            // The auxiliary operand defines a register rather than reading
            // the proven-zero scratch.
            _ => {
                plan.functions[0].blocks[0].instructions[2].operands[3].access =
                    RegisterOperandAccess::Def;
            }
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
                LiteralFoldPolicy::EXACT_DIVIDE_V1,
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

#[test]
fn divide_fold_rejects_consumer_operands_carrying_forbidden_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let inputs = staged_divide_inputs(target);

    // The pinned divide form carries `fixed_view` decorations the rewrite
    // deliberately drops; `tied_to` and `early_clobber` have no carried
    // meaning once the operand list is rebuilt and reject under the declared
    // unit-effect surface.
    for mutation in 0..2 {
        let mut plan = inputs.selected.transformed().clone();
        let operand = &mut plan.functions[0].blocks[0].instructions[2].operands[1];
        match mutation {
            0 => operand.tied_to = Some(0),
            _ => operand.early_clobber = true,
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
                LiteralFoldPolicy::EXACT_DIVIDE_V1,
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

#[test]
fn divide_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The divide's operand grammar admits the literal only under the
    // divide-identity policy: another selected family sees no admitted
    // consumer kind.
    let inputs = staged_divide_inputs(target);
    assert_eq!(
        fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    // And the divide policy admits no other consumer: the compare fixture's
    // flag-defining consumer has no `Use` operand 1 the divide grammar
    // could bind.
    let compare = staged_inputs(target);
    assert_eq!(
        fold_with(&compare, &environment, LiteralFoldPolicy::EXACT_DIVIDE_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn divide_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_divide_inputs(target);
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_DIVIDE_V1)
        .expect("the staged divide fold should validate");

    for mutation in 0..10 {
        let mut plan = result.plan().clone();
        match mutation {
            // The recorded result register is the divide's own `Def`, not
            // the dividend the copy reads.
            0 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            // The recorded immediate is the folded divisor: only one is
            // admitted, so any substitution replays differently.
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
            // The surviving register is the dividend: recording the dropped
            // auxiliary scratch instead binds the wrong `Use`.
            _ => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(3),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn divide_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_divide_inputs(target);
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::EXACT_DIVIDE_V1);
    }
}

#[test]
fn divide_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_divide_inputs(target);
        assert_deterministic_fixed_point(&inputs, &environment, LiteralFoldPolicy::EXACT_DIVIDE_V1);
    }
}

#[test]
fn wrapping_remainder_one_fold_rewrites_the_remainder_to_a_zero_materialization_on_both_linux_targets()
 {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let inputs = staged_remainder_inputs(target);
        let scratch_defs = environment
            .constraint(keys.remainder_i64)
            .unwrap()
            .operands
            .len()
            - 3;
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        )
        .expect("the staged remainder fold should validate");

        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, Some(VirtualRegisterId(2)));
        // The recorded immediate is the folded constant the rewritten
        // `MaterializeI64` embeds — zero — not the folded divisor literal.
        assert_eq!(action.immediate, 0);
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(action.immediate_constraint, keys.materialize_i64);

        let function = &result.transformed().functions[0];
        // The fold removes only the divisor literal and its register: the
        // dividend register and every dead scratch `Def` register stay
        // declared, left unreferenced by the rebuilt operand list.
        assert_eq!(function.virtual_registers.len(), 2 + scratch_defs);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0),
            }
        );
        assert_eq!(rewritten.constraint, keys.materialize_i64);
        assert_eq!(rewritten.operands.len(), 1);
        // The rebuilt operand list binds only the result `Def` — the
        // register pins, the early-clobber scratch, and the dropped
        // dividend `Use` are gone with the pinned remainder form.
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.operands[0].fixed_view, None);
        assert!(!rewritten.operands[0].early_clobber);
        assert!(rewritten.implicit_uses.is_empty());
        assert!(rewritten.implicit_defs.is_empty());
        assert!(rewritten.clobbers.is_empty());
        // The folded literal's provenance joins the consumer's, and the
        // remainder's obligation custody is retained.
        assert_eq!(rewritten.provenance.operations.len(), 2);
        assert_eq!(
            rewritten.provenance.obligations,
            vec![ObligationId::new(7).unwrap()]
        );

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
fn remainder_fold_rejects_a_non_unit_divisor() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_remainder_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        // The literal is the divisor only under the constant fold: a
        // divisor of two is a different computation both the producer's
        // declared bound and the replay's re-derived grammar reject.
        plan.functions[0].blocks[0].instructions[0].kind =
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(2),
            };
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);
        let mut recovery = inputs.recovery.clone();
        let classification = recovery.plan.functions[0].classification.as_mut().unwrap();
        let RecoveryClassification::ImmediateU64RematerializationCandidate { value, .. } =
            &mut classification.classification
        else {
            panic!("staged classification is the immediate candidate");
        };
        *value = IntegerValue::Unsigned(2);

        assert_eq!(
            fold_selected_incoming_literal(
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
                &keys,
                &effect_catalog,
                LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
                budget(),
            ),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
        assert_eq!(
            validate_literal_fold(
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
                &keys,
                &effect_catalog,
                inputs.selected.plan().clone(),
            ),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?} replay"
        );
    }
}

#[test]
fn remainder_fold_rejects_a_dropped_def_without_dead_custody() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let remainder_operands = environment
        .constraint(keys.remainder_i64)
        .unwrap()
        .operands
        .len();
    assert_eq!(
        remainder_operands, 4,
        "the x86-64 remainder carries one dropped scratch def"
    );

    // A `Def` operand past the result is droppable only when its register
    // occurs nowhere else in the function: a `Use` in the scratch position,
    // a scratch register another operand also reads, and a scratch register
    // another operand also defines each reject — the producer and the
    // independent replay alike.
    for mutation in 0..3 {
        let inputs = staged_remainder_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        match mutation {
            // The operand past the result reads a register rather than
            // writing a dead scratch — not a `Def` the grammar may drop.
            0 => {
                plan.functions[0].blocks[0].instructions[1].operands[3].access =
                    RegisterOperandAccess::Use;
            }
            // The scratch operand binds the dividend register, which the
            // operand-0 `Use` also reads — the dropped `Def` would strand
            // that read's only definition's own operand position.
            1 => {
                plan.functions[0].blocks[0].instructions[1].operands[3].virtual_register =
                    VirtualRegisterId(0);
            }
            // The dividend operand reads the scratch register, which the
            // operand-3 `Def` also writes — the dropped `Def` is not dead.
            _ => {
                plan.functions[0].blocks[0].instructions[1].operands[0].virtual_register =
                    VirtualRegisterId(3);
            }
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
                LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
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

#[test]
fn remainder_fold_rejects_consumer_operands_carrying_forbidden_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let inputs = staged_remainder_inputs(target);

    // The pinned remainder form carries `fixed_view` decorations and an
    // early-clobber scratch the rewrite deliberately drops with the folded
    // operand list; `tied_to` has no carried meaning once the operand list
    // is rebuilt and rejects under the declared unit-effect surface.
    let mut plan = inputs.selected.transformed().clone();
    plan.functions[0].blocks[0].instructions[1].operands[1].tied_to = Some(0);
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
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
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
        Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
        "replay"
    );
}

#[test]
fn remainder_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The remainder's operand grammar admits the literal only under the
    // remainder policy: every other selected family sees no admitted
    // consumer kind — including the strongest posture, every other rule
    // enabled at once.
    let inputs = staged_remainder_inputs(target);
    for policy in [
        LiteralFoldPolicy::EXACT_DIVIDE_V1,
        policy_without(LiteralFoldPolicy::WRAPPING_REMAINDER_V1),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And the remainder policy admits no other consumer: the compare
    // fixture's flag-defining consumer has no `Def` operand 2 the
    // constant-result grammar could bind.
    let compare = staged_inputs(target);
    assert_eq!(
        fold_with(
            &compare,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn remainder_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_remainder_inputs(target);
    let result = fold_with(
        &inputs,
        &environment,
        LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
    )
    .expect("the staged remainder fold should validate");

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            // The recorded result register is the remainder's own `Def`,
            // not the dropped dividend `Use`.
            0 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            1 => plan.functions[0].action.as_mut().unwrap().result = None,
            // The recorded immediate is the materialized constant zero;
            // any substitution replays differently.
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
            // A policy without the remainder bit cannot replay the fold:
            // no `MaterializeI64` row binds for this consumer and the
            // action reconstructs nothing.
            8 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            9 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            // The surviving register is the dropped dividend: recording
            // the scratch `Def` register instead fails the re-derived
            // action.
            _ => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(3),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn remainder_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_remainder_inputs(target);
        assert_budget_is_enforced(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        );
    }
}

#[test]
fn remainder_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_remainder_inputs(target);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        );
    }
}
