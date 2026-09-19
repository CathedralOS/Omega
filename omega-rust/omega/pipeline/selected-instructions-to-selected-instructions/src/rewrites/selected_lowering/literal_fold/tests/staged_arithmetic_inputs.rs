//! Staged add, subtract, divide and remainder inputs.

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
    machine_semantic_kind, validated_machine_effect_catalog,
};
use optimization_core::{AcceptedObligationFactIdentity, OptimizationUnitIdentity};
use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    BlockPointDomain, FunctionLiveRanges, LiveRangeFragment, LiveRangePlan, LiveRangePoint,
    LivenessPosition, SaturatingCarrier, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionProvenance, SelectedOperand, SelectedTerminator,
    VirtualLiveRange, VirtualOccurrence, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, ScalarType, ValueId,
};
use std::sync::Arc;
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

/// A `MaterializeI64` victim feeding the right `Use` operand of an
/// `ExactSubtractI64` consumer whose `Def` result is a scalar register, with
/// the pressure-recovery classification admitted as an `Incoming`
/// rematerialization candidate at operand 1.
pub(super) fn staged_subtract_inputs(target: NativeTarget) -> Inputs {
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

/// A `MaterializeI64` victim feeding `ExactAddI64`. `literal_operand` selects
/// which `Use` position carries the folded literal: 1 is the ordinary
/// right-operand grammar, 0 the commutative left-operand grammar. The
/// surviving register `VirtualRegisterId(0)` occupies the other position and
/// `VirtualRegisterId(2)` is the `Def` result either way.
pub(super) fn staged_add_inputs(target: NativeTarget, literal_operand: u16) -> Inputs {
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
/// the divisor — of `ExactDivideU64`: `x / 1` is `x`.
pub(super) fn staged_divide_inputs(target: NativeTarget) -> Inputs {
    staged_divide_family_inputs(
        target,
        1,
        IntegerValue::Unsigned(1),
        LiteralFoldPolicy::EXACT_DIVIDE_V1,
    )
}

/// A `MaterializeI64` victim producing `Unsigned(0)` feeding operand 0 —
/// the dividend — of an `ExactDivideU64` consumer: `0 / x` is `0` under
/// the carried nonzero-divisor obligation. The surviving register
/// `VirtualRegisterId(0)` occupies the operand-1 divisor `Use` the fold
/// drops; the fold's fault discharge is the consumer's recorded
/// obligation, not the folded literal.
pub(super) fn staged_divide_zero_dividend_inputs(target: NativeTarget) -> Inputs {
    staged_divide_family_inputs(
        target,
        0,
        IntegerValue::Unsigned(0),
        LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
    )
}

/// One `ExactDivideU64` consumer whose `literal_operand` `Use` position
/// binds the `MaterializeI64` victim's register `VirtualRegisterId(1)`
/// carrying `literal_immediate`, with the other `Use` position binding the
/// entry-parameter register `VirtualRegisterId(0)` and
/// `VirtualRegisterId(2)` the `Def` result either way, with the
/// pressure-recovery classification already admitted as an `Incoming`
/// rematerialization candidate. `policy` is the fold policy the staged
/// selected input declares. The consumer's operand decorations come from
/// the target's real divide row: on x86-64 that is the pinned `div` form
/// whose operand 3 `Use` is the high-half dividend, staged as a
/// `VirtualRegisterId(3)` scratch defined by a `MaterializeI64(0)` emitted
/// immediately before the literal — the shape selected construction
/// produces, and the zero-provenance custody the auxiliary-`Use` grammar
/// requires; aarch64's `udiv` row carries no auxiliary `Use` and stages
/// the bare three-operand form.
fn staged_divide_family_inputs(
    target: NativeTarget,
    literal_operand: u16,
    literal_immediate: IntegerValue,
    policy: LiteralFoldPolicy,
) -> Inputs {
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
            value: literal_immediate,
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
            .map(|operand| {
                // `VirtualRegisterId(1)` is the literal's result register:
                // it binds whichever `Use` position the fold admits. The
                // entry-parameter `VirtualRegisterId(0)` binds the other
                // leading `Use`; `Def` positions and the auxiliary `Use`
                // tail bind their own index onward.
                let index = u32::from(operand.operand);
                let virtual_register = if operand.access == RegisterOperandAccess::Use
                    && operand.operand == literal_operand
                {
                    VirtualRegisterId(1)
                } else if operand.access == RegisterOperandAccess::Use
                    && operand.operand == 1 - literal_operand
                {
                    VirtualRegisterId(0)
                } else {
                    VirtualRegisterId(index)
                };
                SelectedOperand {
                    operand: operand.operand,
                    virtual_register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                }
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
            normalized_foreign_calls: Vec::new(),
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
            policy,
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
            policy,
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
                1 - literal_operand,
                RegisterOperandAccess::Use,
            )],
            vec![fragment(0, consumer_point + 1)],
        ),
        live(
            VirtualRegisterId(1),
            vec![
                occurrence(literal_id.0, literal_id, 0, RegisterOperandAccess::Def),
                occurrence(
                    consumer_point,
                    consumer_id,
                    literal_operand,
                    RegisterOperandAccess::Use,
                ),
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
                            value: literal_immediate,
                            provenance: literal_provenance,
                            future_uses: vec![RecoveryFutureUse {
                                block: SelectedBlockId(0),
                                point: LiveRangePoint(consumer_point),
                                instruction: consumer_id,
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

/// A `MaterializeI64` victim producing `Unsigned(1)` feeding operand 1 —
/// the divisor — of a `WrappingRemainderI64` consumer: `x % 1` is `0`.
pub(super) fn staged_remainder_inputs(target: NativeTarget) -> Inputs {
    staged_remainder_family_inputs(
        target,
        1,
        IntegerValue::Unsigned(1),
        LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
    )
}

/// A `MaterializeI64` victim producing `Unsigned(0)` feeding operand 0 —
/// the dividend — of a `WrappingRemainderI64` consumer: `0 % x` is `0`
/// under the carried nonzero-divisor obligation. The surviving register
/// `VirtualRegisterId(0)` occupies the operand-1 divisor `Use` the fold
/// drops; the fold's fault discharge is the consumer's recorded
/// obligation, not the folded literal.
pub(super) fn staged_remainder_zero_dividend_inputs(target: NativeTarget) -> Inputs {
    staged_remainder_family_inputs(
        target,
        0,
        IntegerValue::Unsigned(0),
        LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
    )
}

/// A `MaterializeI64` victim producing `Unsigned(u64::MAX)` — the
/// normalized-i64 divisor `-1` — feeding operand 1 of a
/// `WrappingRemainderI64` consumer: `x % -1` is `0` for every `x`,
/// including the `i64::MIN` dividend the kind's semantics defines to
/// produce zero rather than trap. The fold's fault discharge is the
/// folded divisor literal itself, not the consumer's carried obligation.
pub(super) fn staged_remainder_minus_one_inputs(target: NativeTarget) -> Inputs {
    staged_remainder_family_inputs(
        target,
        1,
        IntegerValue::Unsigned(u128::from(u64::MAX)),
        LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
    )
}

/// One `WrappingRemainderI64` consumer whose `literal_operand` `Use`
/// position binds the `MaterializeI64` victim's register
/// `VirtualRegisterId(1)` carrying `literal_value`, with the other `Use`
/// position binding the entry-parameter register `VirtualRegisterId(0)`
/// and `VirtualRegisterId(2)` the `Def` result either way. `policy` is
/// the fold policy the staged selected input declares. The consumer's
/// operand decorations come from the target's real remainder row: on
/// x86-64 that is the pinned `idiv` form — the operand-0 `Use` and
/// operand-2 `Def` result both pinned to `rax`, plus the operand-3
/// early-clobber `Def` quotient scratch pinned to `rdx`, staged as
/// `VirtualRegisterId(3)` — while aarch64's `udiv`/`msub` row carries
/// only an early-clobber operand-2 `Def` and no scratch tail. Every `Def`
/// operand past the result is a scratch output the fold drops; the
/// fixture gives each a register occurring nowhere else in the function,
/// which is the dead-definition custody the grammar requires.
fn staged_remainder_family_inputs(
    target: NativeTarget,
    literal_operand: u16,
    literal_value: IntegerValue,
    policy: LiteralFoldPolicy,
) -> Inputs {
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
    let literal_source = ValueId::new(2).unwrap();
    let literal_provenance = SelectedInstructionProvenance {
        operations: vec![literal_operation],
        values: vec![literal_source],
        edges: Vec::new(),
        obligations: Vec::new(),
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Operation(literal_operation),
            units: 2,
        }],
    };
    // The operand grammar fixes positions 0 through 2 — dividend `Use`,
    // divisor `Use`, result `Def` — with `literal_operand` naming the
    // `Use` position the folded literal feeds; every `Def` operand past
    // the result is a scratch output the fold drops, staged as a register
    // the consumer alone defines.
    let scratch_defs = remainder.operands.len() - 3;
    let literal_id = SelectedInstructionId(0);
    let consumer_id = SelectedInstructionId(1);
    let literal = SelectedInstruction {
        id: literal_id,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: literal_value,
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
            .map(|operand| {
                // `VirtualRegisterId(1)` is the literal's result register:
                // it binds whichever `Use` position the fold admits. The
                // entry-parameter `VirtualRegisterId(0)` binds the other
                // `Use`; `Def` positions bind their own index onward.
                let index = u32::from(operand.operand);
                let virtual_register = if operand.access == RegisterOperandAccess::Use
                    && operand.operand == literal_operand
                {
                    VirtualRegisterId(1)
                } else if operand.access == RegisterOperandAccess::Use
                    && operand.operand == 1 - literal_operand
                {
                    VirtualRegisterId(0)
                } else {
                    VirtualRegisterId(index)
                };
                SelectedOperand {
                    operand: operand.operand,
                    virtual_register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                }
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
                source_value: literal_source,
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
            normalized_foreign_calls: Vec::new(),
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
            policy,
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
            policy,
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
                1 - literal_operand,
                RegisterOperandAccess::Use,
            )],
            vec![fragment(0, consumer_point + 1)],
        ),
        live(
            VirtualRegisterId(1),
            vec![
                occurrence(literal_id.0, literal_id, 0, RegisterOperandAccess::Def),
                occurrence(
                    consumer_point,
                    consumer_id,
                    literal_operand,
                    RegisterOperandAccess::Use,
                ),
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
                        source_value: literal_source,
                    },
                    definition_site: Some(ValueDefinitionSite::Node {
                        block: source_block,
                        node: 0,
                    }),
                    classification:
                        RecoveryClassification::ImmediateU64RematerializationCandidate {
                            defining_instruction: literal_id,
                            source_value: literal_source,
                            value: literal_value,
                            provenance: literal_provenance,
                            future_uses: vec![RecoveryFutureUse {
                                block: SelectedBlockId(0),
                                point: LiveRangePoint(consumer_point),
                                instruction: consumer_id,
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

/// A `MaterializeI64` victim producing `Unsigned(0)` feeding one `Use` of a
/// `BitwiseAndI64` consumer whose `Def` result is a scalar register, with
/// the pressure-recovery classification admitted as an `Incoming`
/// rematerialization candidate at `literal_operand`. `literal_operand`
/// selects which `Use` position carries the folded literal: 1 is the
/// ordinary right-operand grammar, 0 the left-operand annihilator grammar.
/// The surviving register `VirtualRegisterId(0)` occupies the other `Use`
/// position and `VirtualRegisterId(2)` is the `Def` result either way.
/// Operands past the operand-2 `Def` result — none on either Linux target's
/// row — would be scratch `Def` outputs the fold drops dead.
pub(super) fn staged_and_inputs(target: NativeTarget, literal_operand: u16) -> Inputs {
    staged_literal_binary_inputs(
        target,
        literal_operand,
        SelectedInstructionKind::BitwiseAndI64,
        LiteralFoldPolicy::BITWISE_AND_ZERO_V1,
        0,
        BlockZeroTerminator::ConditionalBranch,
    )
}

/// The same `BitwiseAndI64` fixture with the all-ones literal: the
/// and-ones family folds `x & MAX` and `MAX & x` into a `CopyI64` of the
/// surviving operand — all-ones is the bitwise-and identity element,
/// disjoint on the literal's value from the and-zero annihilator family
/// the same consumer kind admits.
pub(super) fn staged_and_ones_inputs(target: NativeTarget, literal_operand: u16) -> Inputs {
    staged_literal_binary_inputs(
        target,
        literal_operand,
        SelectedInstructionKind::BitwiseAndI64,
        LiteralFoldPolicy::BITWISE_AND_ONES_V1,
        u64::MAX,
        BlockZeroTerminator::ConditionalBranch,
    )
}

/// The same zero-literal fixture for `BitwiseXorI64`: the xor family
/// folds `x ^ 0` and `0 ^ x` into a `CopyI64` of the surviving operand
/// rather than materializing the annihilator constant.
pub(super) fn staged_xor_inputs(target: NativeTarget, literal_operand: u16) -> Inputs {
    staged_literal_binary_inputs(
        target,
        literal_operand,
        SelectedInstructionKind::BitwiseXorI64,
        LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
        0,
        BlockZeroTerminator::ConditionalBranch,
    )
}

/// The same zero-literal fixture for `WrappingAddI64`: the wrapping-add
/// family folds `x + 0` and `0 + x` into a `CopyI64` of the surviving
/// operand — zero is the additive identity under modulo-2^64 wrap.
pub(super) fn staged_wrapping_add_inputs(target: NativeTarget, literal_operand: u16) -> Inputs {
    staged_literal_binary_inputs(
        target,
        literal_operand,
        SelectedInstructionKind::WrappingAddI64,
        LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
        0,
        BlockZeroTerminator::ConditionalBranch,
    )
}

/// The instruction record block 0's terminator wraps. Every family defaults
/// to the flag-reading `ConditionalBranch`; the saturating-add fixture needs
/// `Jump` for its positive aarch64 cases because that consumer implicitly
/// defines `nzcv`, so a branch reading condition state would keep the
/// retired definition live and the dead-definitions gate must see it go.
#[derive(Clone, Copy)]
pub(super) enum BlockZeroTerminator {
    ConditionalBranch,
    Jump,
}

/// The same zero-literal fixture for `SaturatingAdd` on the `U64` carrier:
/// `x +| 0` and `0 +| x` fold into a `CopyI64` of the surviving operand.
/// Unlike the isolated bitwise and wrapping consumers, this one retires
/// target-specific unit effects — aarch64 defines `nzcv`, x86-64 clobbers
/// `rflags` — so `block0` chooses the block-0 terminator: `Jump` keeps
/// every condition-state reader out of the function for the positive
/// cases, while `ConditionalBranch` stages a live `nzcv` use on aarch64
/// that the fold must refuse.
pub(super) fn staged_saturating_add_inputs(
    target: NativeTarget,
    literal_operand: u16,
    block0: BlockZeroTerminator,
) -> Inputs {
    staged_saturating_add_carrier_inputs(target, SaturatingCarrier::U64, literal_operand, block0)
}

/// The same zero-literal fixture for `SaturatingAdd` on `carrier`: every
/// non-u64 carrier binds the clamped row — two `Use` operands, an
/// early-clobber `Def` result, and a bound scratch `Def` at operand 3 the
/// fold drops under occurrence-free custody — so the staged consumer
/// carries four operands and the scratch register the consumer alone
/// defines.
pub(super) fn staged_saturating_add_carrier_inputs(
    target: NativeTarget,
    carrier: SaturatingCarrier,
    literal_operand: u16,
    block0: BlockZeroTerminator,
) -> Inputs {
    staged_literal_binary_inputs(
        target,
        literal_operand,
        SelectedInstructionKind::SaturatingAdd { carrier },
        LiteralFoldPolicy::SATURATING_ADD_ZERO_V1,
        0,
        block0,
    )
}

/// A `MaterializeI64` victim producing `Unsigned(carrier.maximum_bits())`
/// feeding a `Use` operand of a `SaturatingAdd` consumer on `carrier`,
/// under the upper-bound policy: `x +| MAX` and `MAX +| x` are both `MAX`
/// for every `x` an unsigned carrier admits, because `x + MAX` reaches
/// the carrier's upper bound and saturates to it, so the fold rewrites
/// the consumer into a `MaterializeI64` of the maximum at the result
/// register and drops the other `Use` the constant result never reads.
/// Signed carriers admit no maximum fold — `x +| MAX` there is `x + MAX`
/// unclamped for every negative `x`, not a constant — so staging `carrier`
/// signed produces the arrangement the family refuses. The u64 carrier's
/// consumer carries exactly three operands; every other carrier binds the
/// clamped row whose bound scratch `Def` at operand 3 the fold drops
/// under occurrence-free custody. The consumer retires the same
/// target-specific unit effects the identity family does — aarch64
/// defines `nzcv`, x86-64 clobbers `rflags` — so `block0` chooses the
/// block-0 terminator the same way.
pub(super) fn staged_saturating_add_upper_bound_carrier_inputs(
    target: NativeTarget,
    carrier: SaturatingCarrier,
    literal_operand: u16,
    block0: BlockZeroTerminator,
) -> Inputs {
    staged_literal_binary_inputs(
        target,
        literal_operand,
        SelectedInstructionKind::SaturatingAdd { carrier },
        LiteralFoldPolicy::SATURATING_ADD_UPPER_BOUND_V1,
        carrier.maximum_bits(),
        block0,
    )
}

/// The same zero-literal fixture for `SaturatingSubtract` on the `U64`
/// carrier: `x -| 0` folds into a `CopyI64` of the operand-0 operand.
/// Unlike the saturating-add family the grammar is asymmetric —
/// `0 -| x` is `-x` clamped, not `x` — so `literal_operand` must be 1 for
/// an admitted fold; staging it at 0 produces the left-literal
/// arrangement the family refuses. The consumer retires the same
/// target-specific unit effects the saturating add does — aarch64
/// defines `nzcv`, x86-64 clobbers `rflags` — so `block0` chooses the
/// block-0 terminator the same way.
pub(super) fn staged_saturating_subtract_inputs(
    target: NativeTarget,
    literal_operand: u16,
    block0: BlockZeroTerminator,
) -> Inputs {
    staged_saturating_subtract_carrier_inputs(
        target,
        SaturatingCarrier::U64,
        literal_operand,
        block0,
    )
}

/// The same zero-literal fixture for `SaturatingSubtract` on `carrier`:
/// every unsigned carrier binds the three-operand unsigned row — two
/// `Use` operands and a `Def` result — while every signed carrier binds
/// the clamped row whose bound scratch `Def` at operand 3 the fold drops
/// under occurrence-free custody, so the staged consumer carries the
/// row's own operand count and the scratch register the consumer alone
/// defines.
pub(super) fn staged_saturating_subtract_carrier_inputs(
    target: NativeTarget,
    carrier: SaturatingCarrier,
    literal_operand: u16,
    block0: BlockZeroTerminator,
) -> Inputs {
    staged_literal_binary_inputs(
        target,
        literal_operand,
        SelectedInstructionKind::SaturatingSubtract { carrier },
        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
        0,
        block0,
    )
}

/// A `MaterializeI64` victim producing `Unsigned(0)` feeding operand 0 —
/// the minuend — of a `SaturatingSubtract` consumer on `carrier`, under
/// the zero-minuend policy: `0 -| x` is `0` for every `x` an unsigned
/// carrier admits, because `0 - x` underflows the carrier's lower bound
/// and saturates to it, so the fold rewrites the consumer into a
/// `MaterializeI64` of zero at the result register and drops the
/// operand-1 subtrahend `Use`. Signed carriers admit no operand-0 fold —
/// `0 -| x` there is `-x` clamped to the carrier's bounds, not a
/// constant — so staging `carrier` signed produces the arrangement the
/// family refuses. The surviving register `VirtualRegisterId(0)`
/// occupies the operand-1 `Use` the fold drops. The consumer retires the
/// same target-specific unit effects the identity family does — aarch64
/// defines `nzcv`, x86-64 clobbers `rflags` — so `block0` chooses the
/// block-0 terminator the same way.
pub(super) fn staged_saturating_subtract_zero_minuend_carrier_inputs(
    target: NativeTarget,
    carrier: SaturatingCarrier,
    literal_operand: u16,
    block0: BlockZeroTerminator,
) -> Inputs {
    staged_literal_binary_inputs(
        target,
        literal_operand,
        SelectedInstructionKind::SaturatingSubtract { carrier },
        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
        0,
        block0,
    )
}

/// A `MaterializeI64` victim producing `Unsigned(carrier.maximum_bits())`
/// feeding operand 1 — the subtrahend — of a `SaturatingSubtract`
/// consumer on `carrier`, under the upper-bound policy: `x -| MAX` is
/// `0` for every `x` an unsigned carrier admits, because `x - MAX`
/// underflows the carrier's lower bound and saturates to it for every
/// `x < MAX` and is exactly zero at `x == MAX`, so the fold rewrites the
/// consumer into a `MaterializeI64` of zero at the result register and
/// drops the operand-0 minuend `Use` the constant result never reads.
/// Signed carriers admit no maximum-subtrahend fold — `x -| MAX` there
/// is `x - MAX` clamped to the carrier's lower bound for every negative
/// `x`, not a constant — so staging `carrier` signed produces the
/// arrangement the family refuses. The grammar is asymmetric —
/// `MAX -| x` is `MAX - x`, not a constant — so `literal_operand` must
/// be 1 for an admitted fold; staging it at 0 produces the left-literal
/// arrangement the family refuses. Every unsigned carrier binds the
/// three-operand row — two `Use` operands and a `Def` result, no scratch
/// tail. The consumer retires the same target-specific unit effects the
/// sibling families do — aarch64 defines `nzcv`, x86-64 clobbers
/// `rflags` — so `block0` chooses the block-0 terminator the same way.
pub(super) fn staged_saturating_subtract_upper_bound_carrier_inputs(
    target: NativeTarget,
    carrier: SaturatingCarrier,
    literal_operand: u16,
    block0: BlockZeroTerminator,
) -> Inputs {
    staged_literal_binary_inputs(
        target,
        literal_operand,
        SelectedInstructionKind::SaturatingSubtract { carrier },
        LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1,
        carrier.maximum_bits(),
        block0,
    )
}

/// A `MaterializeI64` victim producing `Unsigned(1)` feeding operand 1 —
/// the divisor — of a `SaturatingDivide` consumer on the `U64` carrier:
/// `x /| 1` is `x`. The grammar is asymmetric — `1 /| x` is not `x` —
/// so `literal_operand` must be 1 for an admitted fold; staging it at 0
/// produces the left-literal arrangement the family refuses. The consumer
/// retires both effect surfaces this family carries: the encoded
/// architectural fault the divisor-one literal discharges and the
/// implicit unit definitions — aarch64's signed rows' `nzcv` write —
/// gated on whole-function deadness, so `block0` chooses the block-0
/// terminator the same way the saturating-subtract fixture does.
pub(super) fn staged_saturating_divide_inputs(
    target: NativeTarget,
    literal_operand: u16,
    block0: BlockZeroTerminator,
) -> Inputs {
    staged_saturating_divide_carrier_inputs(target, SaturatingCarrier::U64, literal_operand, block0)
}

/// The same divisor-one fixture for `SaturatingDivide` on `carrier`.
/// Unsigned carriers bind the `divide_u64` row — aarch64's bare
/// three-operand `udiv`, x86-64's pinned four-operand `div` whose
/// operand-3 `Use` reads the zeroed high-half dividend — while every
/// signed carrier binds the `saturating_divide_signed` row: x86-64's
/// pinned `idiv` keeps the same zeroed-rdx auxiliary `Use`, and
/// aarch64's clamped form continues past the `Def` result with the bound
/// scratch `Def` the fold drops under occurrence-free custody. The staged
/// consumer carries the row's own operand list, a zero-materializing
/// definition for every auxiliary `Use` operand, and a scratch register
/// the consumer alone defines for every `Def` operand past the result.
pub(super) fn staged_saturating_divide_carrier_inputs(
    target: NativeTarget,
    carrier: SaturatingCarrier,
    literal_operand: u16,
    block0: BlockZeroTerminator,
) -> Inputs {
    staged_saturating_divide_family_inputs(
        target,
        carrier,
        literal_operand,
        IntegerValue::Unsigned(1),
        LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
        block0,
    )
}

/// The same fixture for `SaturatingDivide` on `carrier` whose
/// `MaterializeI64` victim produces `Unsigned(0)` and feeds operand 0 —
/// the dividend: `0 /| x` is `0` inside the carrier's bounds under the
/// carried nonzero-divisor obligation. The surviving register
/// `VirtualRegisterId(0)` occupies the operand-1 divisor `Use` the fold
/// drops; the fold's fault discharge is the consumer's recorded
/// obligation, not the folded literal. The grammar is asymmetric —
/// `x /| 0` is the divide-by-zero case, not a constant — so
/// `literal_operand` must be 0 for an admitted fold; staging it at 1
/// produces the right-literal arrangement the family refuses.
pub(super) fn staged_saturating_divide_zero_dividend_carrier_inputs(
    target: NativeTarget,
    carrier: SaturatingCarrier,
    literal_operand: u16,
    block0: BlockZeroTerminator,
) -> Inputs {
    staged_saturating_divide_family_inputs(
        target,
        carrier,
        literal_operand,
        IntegerValue::Unsigned(0),
        LiteralFoldPolicy::SATURATING_DIVIDE_ZERO_V1,
        block0,
    )
}

/// Shared staging for the exact-literal binary families: a
/// `MaterializeI64` victim producing `Unsigned(literal)` feeds `kind`'s
/// `Use` operand at `literal_operand`, whose `Def` result is a scalar
/// register, under `policy`. The consumer's constraint row comes from the
/// semantic's own selected key: the bitwise consumers bind the
/// flag-clobbering subtract row — x86-64 `and`/`xor` destroy `rflags`,
/// the aarch64 forms touch no condition state — the wrapping-add
/// consumer binds the flag-transparent add row, which clobbers nothing,
/// and the `U64` saturating add binds its three-operand carry-select row,
/// which defines `nzcv` on aarch64 and clobbers `rflags` on x86-64.
/// `block0` picks the terminator record wrapping instruction 2: the
/// default conditional branch reads condition state on both targets,
/// while `Jump` keeps the function free of implicit condition-state uses.
fn staged_literal_binary_inputs(
    target: NativeTarget,
    literal_operand: u16,
    kind: SelectedInstructionKind,
    policy: LiteralFoldPolicy,
    immediate: u64,
    block0: BlockZeroTerminator,
) -> Inputs {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let machine = MachineId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    // The consumer row is the one the semantic's own effect declaration
    // binds: the flag-clobbering subtract row for the bitwise forms —
    // x86-64 `and`/`xor` destroy `rflags`, aarch64 `and`/`eor` touch no
    // condition state — the flag-transparent add row for the wrapping
    // add, which clobbers nothing on either target, and the `U64`
    // saturating-add row, which defines `nzcv` on aarch64 and clobbers
    // `rflags` on x86-64.
    let consumer_key = keys.for_semantic(machine_semantic_kind(kind)).unwrap();
    let consumer_row = environment.constraint(consumer_key).unwrap();
    let tail = environment
        .constraint(match block0 {
            BlockZeroTerminator::ConditionalBranch => keys.conditional_branch,
            BlockZeroTerminator::Jump => keys.jump,
        })
        .unwrap();
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
    // The operand grammar fixes positions 0 through 2 — the two `Use`
    // operands and the result `Def`; every `Def` operand past the result is
    // a scratch output the fold drops, staged as a register the consumer
    // alone defines.
    let scratch_defs = consumer_row.operands.len() - 3;
    let literal_id = SelectedInstructionId(0);
    let consumer_id = SelectedInstructionId(1);
    let literal = SelectedInstruction {
        id: literal_id,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(immediate)),
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
    // The victim sits at `literal_operand`; the surviving register takes the
    // other `Use` position, and each `Def` position keeps the
    // operand-indexed register convention — operand 2 binds
    // `VirtualRegisterId(2)`, scratch outputs bind registers 3 and up.
    let consumer = SelectedInstruction {
        id: consumer_id,
        kind,
        constraint: consumer_row.key,
        operands: consumer_row
            .operands
            .iter()
            .map(|operand| SelectedOperand {
                operand: operand.operand,
                virtual_register: if operand.operand == literal_operand {
                    VirtualRegisterId(1)
                } else if operand.access == RegisterOperandAccess::Use {
                    VirtualRegisterId(0)
                } else {
                    VirtualRegisterId(u32::from(operand.operand))
                },
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: consumer_row.implicit_uses.clone(),
        implicit_defs: consumer_row.implicit_defs.clone(),
        clobbers: consumer_row.clobbers.clone(),
        provenance: SelectedInstructionProvenance {
            operations: vec![OperationId::new(2).unwrap()],
            values: vec![ValueId::new(1).unwrap()],
            ..Default::default()
        },
    };
    let tail_instruction = SelectedInstruction {
        id: SelectedInstructionId(2),
        kind: match block0 {
            BlockZeroTerminator::ConditionalBranch => {
                SelectedInstructionKind::ConditionalBranchNonZero
            }
            BlockZeroTerminator::Jump => SelectedInstructionKind::Jump,
        },
        constraint: tail.key,
        operands: Vec::new(),
        implicit_uses: tail.implicit_uses.clone(),
        implicit_defs: tail.implicit_defs.clone(),
        clobbers: tail.clobbers.clone(),
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
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers,
            blocks: vec![
                SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(source_block),
                    instructions: vec![literal, consumer],
                    terminator: match block0 {
                        BlockZeroTerminator::ConditionalBranch => {
                            SelectedTerminator::ConditionalBranch {
                                instruction: tail_instruction,
                                when_nonzero: successor(1, 1),
                                when_zero: successor(1, 2),
                            }
                        }
                        BlockZeroTerminator::Jump => SelectedTerminator::Jump {
                            instruction: tail_instruction,
                            successor: successor(1, 1),
                        },
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
            policy,
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
            policy,
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
                1 - literal_operand,
                RegisterOperandAccess::Use,
            )],
            vec![fragment(0, consumer_point + 1)],
        ),
        live(
            VirtualRegisterId(1),
            vec![
                occurrence(literal_id.0, literal_id, 0, RegisterOperandAccess::Def),
                occurrence(
                    consumer_point,
                    consumer_id,
                    literal_operand,
                    RegisterOperandAccess::Use,
                ),
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
                            value: IntegerValue::Unsigned(u128::from(immediate)),
                            provenance: literal_provenance,
                            future_uses: vec![RecoveryFutureUse {
                                block: SelectedBlockId(0),
                                point: LiveRangePoint(consumer_point),
                                instruction: consumer_id,
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

/// Shared staging for the saturating-divide identity family: a
/// `MaterializeI64` victim producing `Unsigned(1)` feeds the divisor
/// `Use` at `literal_operand` of a `SaturatingDivide` consumer on
/// `carrier`, whose `Def` result is a scalar register, under the
/// saturating-divide-one policy. The consumer's constraint row comes from
/// the semantic's own selected key — `divide_u64` for the unsigned
/// carriers, `saturating_divide_signed` for the signed — so the staged
/// operand list is the row's own: positions 0 through 2 are the
/// `[left, divisor, result]` grammar, and every operand past the `Def`
/// result is the mixed tail the family alone admits. Each tail `Use` —
/// the zeroed high-half dividend an x86-64 `div`/`idiv` realization reads
/// — binds a register a `MaterializeI64(0)` emitted immediately before
/// the literal defines, the zero-provenance custody the grammar
/// requires; each tail `Def` — the bound scratch an aarch64 clamped
/// signed realization writes — binds a register the consumer alone
/// defines. `block0` picks the terminator record wrapping the
/// instruction after the consumer: the default conditional branch reads
/// condition state on both targets, which keeps the aarch64 signed
/// rows' `nzcv` definition live, while `Jump` keeps the function free of
/// implicit condition-state uses.
fn staged_saturating_divide_family_inputs(
    target: NativeTarget,
    carrier: SaturatingCarrier,
    literal_operand: u16,
    literal_immediate: IntegerValue,
    policy: LiteralFoldPolicy,
    block0: BlockZeroTerminator,
) -> Inputs {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let machine = MachineId::new(1).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let kind = SelectedInstructionKind::SaturatingDivide {
        carrier,
        obligation: ObligationId::new(7).unwrap(),
        accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
    };
    let consumer_key = keys.for_semantic(machine_semantic_kind(kind)).unwrap();
    let consumer_row = environment.constraint(consumer_key).unwrap();
    let tail = environment
        .constraint(match block0 {
            BlockZeroTerminator::ConditionalBranch => keys.conditional_branch,
            BlockZeroTerminator::Jump => keys.jump,
        })
        .unwrap();
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
    // Split the operand tail by access: a `Use` past the result is the
    // auxiliary input the fold drops under zero-provenance custody,
    // staged with a `MaterializeI64(0)` definition emitted before the
    // literal; a `Def` past the result is the bound scratch, staged as a
    // register the consumer alone defines.
    let auxiliaries = consumer_row
        .operands
        .iter()
        .skip(3)
        .filter(|operand| operand.access == RegisterOperandAccess::Use)
        .enumerate()
        .map(|(auxiliary, operand)| {
            let register = VirtualRegisterId(u32::from(operand.operand));
            (
                register,
                operand.operand,
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
    let literal_id = SelectedInstructionId(u32::try_from(auxiliaries.len()).unwrap());
    let consumer_id = SelectedInstructionId(literal_id.0 + 1);
    let literal = SelectedInstruction {
        id: literal_id,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: literal_immediate,
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
    // The victim sits at `literal_operand`; the surviving register takes
    // the other `Use` position, and every operand past the result keeps
    // the operand-indexed register convention — operand 2 binds
    // `VirtualRegisterId(2)`, the tail operands bind registers 3 and up.
    let consumer = SelectedInstruction {
        id: consumer_id,
        kind,
        constraint: consumer_row.key,
        operands: consumer_row
            .operands
            .iter()
            .map(|operand| {
                let index = u32::from(operand.operand);
                let virtual_register = if operand.access == RegisterOperandAccess::Use
                    && operand.operand == literal_operand
                {
                    VirtualRegisterId(1)
                } else if operand.access == RegisterOperandAccess::Use
                    && operand.operand == 1 - literal_operand
                {
                    VirtualRegisterId(0)
                } else {
                    VirtualRegisterId(index)
                };
                SelectedOperand {
                    operand: operand.operand,
                    virtual_register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                }
            })
            .collect(),
        implicit_uses: consumer_row.implicit_uses.clone(),
        implicit_defs: consumer_row.implicit_defs.clone(),
        clobbers: consumer_row.clobbers.clone(),
        provenance: SelectedInstructionProvenance {
            operations: vec![OperationId::new(2).unwrap()],
            values: vec![ValueId::new(1).unwrap()],
            obligations: vec![ObligationId::new(7).unwrap()],
            ..Default::default()
        },
    };
    let tail_instruction = SelectedInstruction {
        id: SelectedInstructionId(consumer_id.0 + 1),
        kind: match block0 {
            BlockZeroTerminator::ConditionalBranch => {
                SelectedInstructionKind::ConditionalBranchNonZero
            }
            BlockZeroTerminator::Jump => SelectedInstructionKind::Jump,
        },
        constraint: tail.key,
        operands: Vec::new(),
        implicit_uses: tail.implicit_uses.clone(),
        implicit_defs: tail.implicit_defs.clone(),
        clobbers: tail.clobbers.clone(),
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
    virtual_registers.extend(consumer_row.operands.iter().skip(3).map(|operand| {
        // A tail `Use`'s register is the scratch its zero-materializing
        // auxiliary instruction defines; a tail `Def`'s register is the
        // bound scratch the consumer alone defines.
        let (instruction, origin_operand) = match operand.access {
            RegisterOperandAccess::Use => {
                let auxiliary = auxiliaries
                    .iter()
                    .find(|(_, position, _)| *position == operand.operand)
                    .expect("every tail `Use` stages an auxiliary definition");
                (auxiliary.2.id, 0)
            }
            _ => (consumer_id, operand.operand),
        };
        VirtualRegister {
            id: VirtualRegisterId(u32::from(operand.operand)),
            scalar_type: scalar,
            class: operand.class,
            origin: VirtualRegisterOrigin::InstructionScratch {
                instruction,
                operand: origin_operand,
            },
            definition_site: None,
            entry_fixed_view: None,
        }
    }));
    let mut block_instructions = auxiliaries
        .iter()
        .map(|(_, _, instruction)| instruction.clone())
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
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers,
            blocks: vec![
                SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(source_block),
                    instructions: block_instructions,
                    terminator: match block0 {
                        BlockZeroTerminator::ConditionalBranch => {
                            SelectedTerminator::ConditionalBranch {
                                instruction: tail_instruction,
                                when_nonzero: successor(1, 1),
                                when_zero: successor(1, 2),
                            }
                        }
                        BlockZeroTerminator::Jump => SelectedTerminator::Jump {
                            instruction: tail_instruction,
                            successor: successor(1, 1),
                        },
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
            policy,
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
            policy,
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
                1 - literal_operand,
                RegisterOperandAccess::Use,
            )],
            vec![fragment(0, consumer_point + 1)],
        ),
        live(
            VirtualRegisterId(1),
            vec![
                occurrence(literal_id.0, literal_id, 0, RegisterOperandAccess::Def),
                occurrence(
                    consumer_point,
                    consumer_id,
                    literal_operand,
                    RegisterOperandAccess::Use,
                ),
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
    virtual_live_ranges.extend(consumer_row.operands.iter().skip(3).map(|operand| {
        match operand.access {
            RegisterOperandAccess::Use => {
                // The auxiliary `Use`'s register is defined by its own
                // zero materialization at that instruction's position and
                // read once at the consumer point.
                let auxiliary = auxiliaries
                    .iter()
                    .find(|(_, position, _)| *position == operand.operand)
                    .expect("every tail `Use` stages an auxiliary definition");
                live(
                    VirtualRegisterId(u32::from(operand.operand)),
                    vec![
                        occurrence(
                            auxiliary.2.id.0,
                            auxiliary.2.id,
                            0,
                            RegisterOperandAccess::Def,
                        ),
                        occurrence(
                            consumer_point,
                            consumer_id,
                            operand.operand,
                            RegisterOperandAccess::Use,
                        ),
                    ],
                    vec![fragment(auxiliary.2.id.0, consumer_point + 1)],
                )
            }
            _ => live(
                VirtualRegisterId(u32::from(operand.operand)),
                vec![occurrence(
                    consumer_point,
                    consumer_id,
                    operand.operand,
                    RegisterOperandAccess::Def,
                )],
                vec![fragment(consumer_point, consumer_point + 1)],
            ),
        }
    }));
    let tail_len = consumer_row.operands.len() - 3;
    let register_count = 3 + tail_len;
    // Four base occurrences — the surviving `Use`, the literal's `Def`
    // and `Use`, and the result `Def` — plus two per auxiliary `Use` and
    // one per scratch `Def`.
    let occurrence_count = 4 + auxiliaries.len() + tail_len;
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
                            value: literal_immediate,
                            provenance: literal_provenance,
                            future_uses: vec![RecoveryFutureUse {
                                block: SelectedBlockId(0),
                                point: LiveRangePoint(consumer_point),
                                instruction: consumer_id,
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
