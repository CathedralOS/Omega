//! Staged extension, copy, indexed load and byte view address inputs.

use super::super::super::super::super::{
    AllocationLegalityIdentity, AllocationLegalityPlan, AllocatorAvailabilityIdentity,
    AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy, FunctionAllocationLegality,
    FunctionRecoveryClassification, FunctionSpillChoices, LiveRangeIdentity, LivenessIdentity,
    PressureRecoveryClassification, RecoveryClassification, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, RecoveryFutureUse,
    RecoveryVictimRole, SpillChoice, SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy,
};
use super::{Inputs, budget, successor, usage};
use crate::{
    AllocationLegalityValidationReceipt, AllocatorAvailabilityValidationReceipt,
    FunctionLiteralFold, LiteralFoldIdentity, LiteralFoldPlan, LiteralFoldPolicy,
    LiteralFoldValidationReceipt, LiveRangeValidationReceipt,
    RecoveryClassificationValidationReceipt, SpillChoiceValidationReceipt,
    ValidatedAllocationLegality, ValidatedAllocatorAvailability, ValidatedLiteralFold,
    ValidatedLiveRanges, ValidatedRecoveryClassifications, ValidatedSpillChoices,
    validated_machine_effect_catalog,
};
use optimization_core::OptimizationUnitIdentity;
use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    BlockPointDomain, FunctionLiveRanges, LiveRangeFragment, LiveRangePlan, LiveRangePoint,
    LivenessPosition, SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionProvenance, SelectedOperand, SelectedTerminator, VirtualLiveRange,
    VirtualOccurrence, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};
use std::sync::Arc;
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

/// A `MaterializeI64` victim feeding the sole `Use` operand of a unary
/// extension consumer whose `Def` result carries `result_scalar`, with the
/// pressure-recovery classification admitted as an `Incoming` rematerialization
/// candidate at operand 0.
pub(super) fn staged_extension_inputs(
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
pub(super) fn staged_copy_inputs(
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

/// A `MaterializeI64` victim feeding operand 1 — the index register — of
/// `Load8Indexed`, whose `Def` result is `VirtualRegisterId(2)`, with the
/// pressure-recovery classification already admitted as an `Incoming`
/// rematerialization candidate. `folded` is the folded byte offset.
pub(super) fn staged_load8_indexed_inputs(target: NativeTarget, folded: u64) -> Inputs {
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

/// A `MaterializeI64` victim feeding the operand-0 backing position of
/// `ByteViewAddress` — the commuted operand wiring of the offset fixture.
/// `VirtualRegisterId(0)` is the surviving offset `Use` at operand 1 and
/// `VirtualRegisterId(2)` is the `Def` result at operand 2; the fold
/// rewrites the projection into the same constant-offset `AddressOffset`
/// form the offset fold uses because the modular address addition
/// commutes.
pub(super) fn staged_byte_view_address_backing_inputs(target: NativeTarget, folded: u64) -> Inputs {
    let mut inputs = staged_byte_view_address_inputs(target, folded);
    let mut plan = inputs.selected.transformed().clone();
    let operands = &mut plan.functions[0].blocks[0].instructions[1].operands;
    operands[0].virtual_register = VirtualRegisterId(1);
    operands[1].virtual_register = VirtualRegisterId(0);
    inputs.selected.transformed = Arc::new(plan);
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
    // Keep the staged live ranges honest with the rewired operand list: the
    // survivor's `Use` moves to operand 1 and the victim's to operand 0.
    let ranges = Arc::make_mut(&mut inputs.ranges.plan);
    for occurrence in &mut ranges.functions[0].virtual_registers[0].occurrences {
        occurrence.operand = 1;
    }
    for occurrence in &mut ranges.functions[0].virtual_registers[1].occurrences {
        if occurrence.access == RegisterOperandAccess::Use {
            occurrence.operand = 0;
        }
    }
    inputs
}

/// A `MaterializeI64` victim feeding the operand-1 offset position of
/// `ByteViewAddress`, with the pressure-recovery classification already
/// admitted as an `Incoming` rematerialization candidate. `VirtualRegisterId(0)`
/// is the surviving base `Use` at operand 0 and `VirtualRegisterId(2)` is the
/// `Def` result at operand 2; the fold rewrites the projection into the
/// constant-offset `AddressOffset` form.
pub(super) fn staged_byte_view_address_inputs(target: NativeTarget, folded: u64) -> Inputs {
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
