use omega_isa_x86_64::{
    encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization, x86_64_physical_register_model,
};
use omega_optimization_core::{OptimizationWorkBudget, PostAllocationOptimizationManifestIdentity};
use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    RegisterOperandAccess, TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    validate_physical_register_model,
};
use omega_selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineEffectCatalogIdentity, MachineEncodedEffects,
    MachineLatencyKnowledge, MachineSizeKnowledge, SelectedBlock, SelectedBlockId,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    SelectedOperand, SelectedTerminator, VirtualRegisterId,
};
use omega_target::NativeTarget;
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{BlockId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::{
    X86_MOV_R64_IMM32_SIGN_EXTENDED_BASELINE_BYTE_COUNT,
    X86_MOV_R64_IMM32_SIGN_EXTENDED_EXTENDED_REGISTER_BYTE_COUNT,
    X86_MOV_R64_IMM32_SIGN_EXTENDED_LOW_REGISTER_BYTE_COUNT,
    X86MovR64Imm32SignExtendedInstructionDisposition,
    X86MovR64Imm32SignExtendedMaterializationAttemptOutcome,
    X86MovR64Imm32SignExtendedMaterializationError,
    X86MovR64Imm32SignExtendedMaterializationWorkAxis,
};
use crate::{
    MachineAlternativeChoiceRule, PhysicalOperandFootprint, PostAllocationMachineBlock,
    PostAllocationMachineFunction, PostAllocationMachineIdentity, PostAllocationMachineInstruction,
    PostAllocationMachinePlan, PreAllocationMachineEffectIdentity,
};

struct Fixture {
    selected: SelectedInstructionPlan,
    selected_identity: SelectedInstructionPlanIdentity,
    source: PostAllocationMachinePlan,
    source_identity: PostAllocationMachineIdentity,
    physical: ValidatedPhysicalRegisterModel,
}

fn constraint() -> RegisterConstraintKey {
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: 0,
    }
}

fn selected_materialization(
    id: u32,
    value: u64,
    physical: &ValidatedPhysicalRegisterModel,
) -> SelectedInstruction {
    let rax = physical.model().view_named("rax").unwrap();
    SelectedInstruction {
        id: SelectedInstructionId(id),
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(value.into()),
        },
        constraint: constraint(),
        operands: vec![SelectedOperand {
            operand: 0,
            virtual_register: VirtualRegisterId(id),
            access: RegisterOperandAccess::Def,
            class: rax.class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }],
        implicit_uses: vec![],
        implicit_defs: vec![],
        clobbers: vec![],
        provenance: SelectedInstructionProvenance::default(),
    }
}

fn machine_materialization(
    id: u32,
    view_name: &str,
    physical: &ValidatedPhysicalRegisterModel,
) -> PostAllocationMachineInstruction {
    let destination = physical.model().view_named(view_name).unwrap();
    PostAllocationMachineInstruction {
        instruction: SelectedInstructionId(id),
        alternative: MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::MaterializeI64,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(10),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![0]),
        },
        operands: vec![PhysicalOperandFootprint {
            operand: 0,
            virtual_register: VirtualRegisterId(id),
            class: destination.class,
            view: destination.id,
            access: RegisterOperandAccess::Def,
            storage_units: destination.units.clone(),
            read_units: vec![],
            write_units: destination.write_units.clone(),
            write_semantics: Some(destination.write_semantics),
        }],
        implicit_unit_uses: vec![],
        implicit_unit_defs: vec![],
        implicit_unit_clobbers: vec![],
        unit_uses: vec![],
        unit_defs: destination.write_units.clone(),
        unit_clobbers: vec![],
    }
}

fn fixture() -> Fixture {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let machine = MachineId::new(1).unwrap();
    let block = SelectedBlockId(0);
    let source_block = BlockId::new(1).unwrap();
    let return_instruction = SelectedInstruction {
        id: SelectedInstructionId(5),
        kind: SelectedInstructionKind::ReturnUnit,
        constraint: constraint(),
        operands: vec![],
        implicit_uses: vec![],
        implicit_defs: vec![],
        clobbers: vec![],
        provenance: SelectedInstructionProvenance::default(),
    };
    let selected = SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target: NativeTarget::linux_x64(),
        entry: machine,
        functions: vec![SelectedFunction {
            machine,
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            entry_block: block,
            virtual_registers: vec![],
            blocks: vec![SelectedBlock {
                id: block,
                source_block,
                instructions: vec![
                    selected_materialization(1, 0, &physical),
                    selected_materialization(2, i32::MAX as u64, &physical),
                    selected_materialization(3, 0x8000_0000, &physical),
                    selected_materialization(4, u64::MAX, &physical),
                ],
                terminator: SelectedTerminator::Return {
                    instruction: return_instruction,
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        }],
        structural_unit_functions: vec![],
    };
    let selected_identity = SelectedInstructionPlanIdentity::from_bytes([2; 32]);
    let source_identity = PostAllocationMachineIdentity::from_bytes([4; 32]);
    let return_machine = PostAllocationMachineInstruction {
        instruction: SelectedInstructionId(5),
        alternative: MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::ReturnUnit,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(1),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![]),
        },
        operands: vec![],
        implicit_unit_uses: vec![],
        implicit_unit_defs: vec![],
        implicit_unit_clobbers: vec![],
        unit_uses: vec![],
        unit_defs: vec![],
        unit_clobbers: vec![],
    };
    let source = PostAllocationMachinePlan {
        identity: source_identity,
        selected: selected_identity,
        effects: PreAllocationMachineEffectIdentity::from_bytes([5; 32]),
        ranges: omega_regalloc::LiveRangeIdentity::from_bytes([6; 32]),
        legality: omega_regalloc::AllocationLegalityIdentity::from_bytes([7; 32]),
        homes: omega_regalloc::RegisterHomeIdentity::from_bytes([8; 32]),
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes([9; 32]),
        target: selected.target,
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([10; 32]),
        physical_register_model: physical.identity(),
        register_constraints: RegisterConstraintCatalogIdentity::from_bytes([11; 32]),
        machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([12; 32]),
        choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        functions: vec![PostAllocationMachineFunction {
            machine,
            blocks: vec![PostAllocationMachineBlock {
                block,
                instructions: vec![
                    machine_materialization(1, "rax", &physical),
                    machine_materialization(2, "rax", &physical),
                    machine_materialization(3, "rax", &physical),
                    machine_materialization(4, "r8", &physical),
                    return_machine,
                ],
            }],
        }],
        structural_unit_functions: vec![],
    };
    Fixture {
        selected,
        selected_identity,
        source,
        source_identity,
        physical,
    }
}

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(20, 20, 20, 20, 20).unwrap()
}

fn compute(fixture: &Fixture) -> super::X86MovR64Imm32SignExtendedMaterializationPlan {
    super::compute::compute_from_parts(
        &fixture.selected,
        fixture.selected_identity,
        &fixture.source,
        fixture.source_identity,
        &fixture.physical,
        budget(),
    )
    .unwrap()
}

#[test]
fn selects_the_exact_sign_extended_i32_domain_for_low_and_extended_registers() {
    let fixture = fixture();
    let computed = compute(&fixture);
    let replayed = super::validate::replay_from_parts(
        &fixture.selected,
        fixture.selected_identity,
        &fixture.source,
        fixture.source_identity,
        &fixture.physical,
        budget(),
    )
    .unwrap();

    assert_eq!(computed, replayed);
    assert_eq!(
        computed
            .actions
            .iter()
            .map(|action| action.instruction)
            .collect::<Vec<_>>(),
        [
            SelectedInstructionId(1),
            SelectedInstructionId(2),
            SelectedInstructionId(4),
        ]
    );
    assert_eq!(
        computed
            .actions
            .iter()
            .map(|action| action.literal_bits)
            .collect::<Vec<_>>(),
        [0, i32::MAX as u64, u64::MAX]
    );
    assert_eq!(
        computed
            .actions
            .iter()
            .map(|action| action.selected_byte_count)
            .collect::<Vec<_>>(),
        [
            X86_MOV_R64_IMM32_SIGN_EXTENDED_LOW_REGISTER_BYTE_COUNT,
            X86_MOV_R64_IMM32_SIGN_EXTENDED_LOW_REGISTER_BYTE_COUNT,
            X86_MOV_R64_IMM32_SIGN_EXTENDED_EXTENDED_REGISTER_BYTE_COUNT,
        ]
    );

    let rax = fixture.physical.model().view_named("rax").unwrap();
    let low_action = &computed.actions[0];
    assert_eq!(
        low_action.baseline_byte_count,
        X86_MOV_R64_IMM32_SIGN_EXTENDED_BASELINE_BYTE_COUNT
    );
    assert_eq!(low_action.destination.destination_view, rax.id);
    assert_eq!(low_action.destination.encoded_view, rax.id);
    assert_eq!(
        low_action.destination.destination_write_units,
        rax.write_units
    );
    assert_eq!(low_action.destination.encoded_storage_units, rax.units);
    assert_eq!(low_action.destination.encoded_write_units, rax.write_units);
    assert_eq!(
        low_action.destination.encoded_write_semantics,
        omega_register_model::RegisterWriteSemantics::ExactView
    );

    for action in &computed.actions {
        let encoded = encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
            &fixture.physical,
            action.destination.destination_view,
            IntegerValue::Unsigned(action.literal_bits.into()),
        )
        .unwrap();
        assert_eq!(
            encoded.bytes().len(),
            usize::from(action.selected_byte_count)
        );
        assert_eq!(encoded.destination(), action.destination.destination_view);
        assert_eq!(
            encoded.encoded_write_view(),
            action.destination.encoded_view
        );
        assert_eq!(
            encoded.footprint().encoded_write_view_units,
            action.destination.encoded_storage_units
        );
        assert_eq!(
            encoded.footprint().encoded_write_units,
            action.destination.encoded_write_units
        );
        assert_eq!(
            encoded.footprint().encoded_write_semantics,
            action.destination.encoded_write_semantics
        );
        assert!(!encoded.footprint().writes_rflags);
    }

    assert_eq!(computed.usage.iterations, 4);
    assert_eq!(computed.usage.rule_evaluations, 11);
    assert_eq!(computed.usage.candidates, 3);
    assert_eq!(computed.usage.validation_steps, 3);
    assert_eq!(computed.usage.commits, 3);
    assert_eq!(computed.attempts.len(), 11);
    assert_eq!(
        computed
            .attempts
            .iter()
            .filter(|attempt| attempt.instruction == SelectedInstructionId(3))
            .map(|attempt| attempt.outcome)
            .collect::<Vec<_>>(),
        [
            X86MovR64Imm32SignExtendedMaterializationAttemptOutcome::IntegerOutsideSignExtendedI32,
            X86MovR64Imm32SignExtendedMaterializationAttemptOutcome::IntegerOutsideSignExtendedI32,
        ]
    );
    for (index, expected_selected) in [true, true, false, true].into_iter().enumerate() {
        assert_eq!(
            matches!(
                computed.functions[0].blocks[0].instructions[index].disposition,
                X86MovR64Imm32SignExtendedInstructionDisposition::MovR64Imm32SignExtendedMaterializationV1 { .. }
            ),
            expected_selected
        );
    }
    assert_eq!(
        computed.functions[0].blocks[0].instructions[2].disposition,
        X86MovR64Imm32SignExtendedInstructionDisposition::RetainedV1
    );
}

#[test]
fn negative_signed_value_is_inside_the_sign_extended_i32_partition() {
    let mut fixture = fixture();
    fixture.selected.functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Signed(-1),
        };
    let computed = compute(&fixture);

    assert!(
        computed
            .actions
            .iter()
            .any(|action| action.instruction == SelectedInstructionId(1))
    );
    assert_eq!(
        computed.attempts[0].outcome,
        X86MovR64Imm32SignExtendedMaterializationAttemptOutcome::SelectedForRewrite
    );
}

#[test]
fn non_r64_baseline_destination_is_rejected_before_selection() {
    let mut fixture = fixture();
    let eax = fixture.physical.model().view_named("eax").unwrap();
    fixture.selected.functions[0].blocks[0].instructions[0].operands[0].class = eax.class;
    let operand = &mut fixture.source.functions[0].blocks[0].instructions[0].operands[0];
    operand.view = eax.id;
    operand.class = eax.class;
    operand.storage_units = eax.units.clone();
    operand.write_units = eax.write_units.clone();
    operand.write_semantics = Some(eax.write_semantics);
    fixture.source.functions[0].blocks[0].instructions[0].unit_defs = eax.write_units.clone();

    assert_eq!(
        super::compute::compute_from_parts(
            &fixture.selected,
            fixture.selected_identity,
            &fixture.source,
            fixture.source_identity,
            &fixture.physical,
            budget(),
        ),
        Err(
            X86MovR64Imm32SignExtendedMaterializationError::InvalidPhysicalDestination(
                SelectedInstructionId(1)
            )
        )
    );
}

#[test]
fn independent_replay_exposes_action_corruption() {
    let fixture = fixture();
    let replayed = super::validate::replay_from_parts(
        &fixture.selected,
        fixture.selected_identity,
        &fixture.source,
        fixture.source_identity,
        &fixture.physical,
        budget(),
    )
    .unwrap();
    let mut corrupted = replayed.clone();
    corrupted.actions[0].destination.encoded_view =
        fixture.physical.model().view_named("ecx").unwrap().id;
    assert_eq!(
        super::validate::validate_from_parts(
            &fixture.selected,
            fixture.selected_identity,
            &fixture.source,
            fixture.source_identity,
            &fixture.physical,
            &corrupted,
        ),
        Err(X86MovR64Imm32SignExtendedMaterializationError::ArtifactMismatch)
    );
}

#[test]
fn every_work_axis_exhausts_at_its_exact_boundary() {
    let fixture = fixture();
    for (budget, axis) in [
        (
            OptimizationWorkBudget::new(10, 20, 20, 20, 20).unwrap(),
            X86MovR64Imm32SignExtendedMaterializationWorkAxis::RuleEvaluations,
        ),
        (
            OptimizationWorkBudget::new(20, 2, 20, 20, 20).unwrap(),
            X86MovR64Imm32SignExtendedMaterializationWorkAxis::Candidates,
        ),
        (
            OptimizationWorkBudget::new(20, 20, 2, 20, 20).unwrap(),
            X86MovR64Imm32SignExtendedMaterializationWorkAxis::ValidationSteps,
        ),
        (
            OptimizationWorkBudget::new(20, 20, 20, 2, 20).unwrap(),
            X86MovR64Imm32SignExtendedMaterializationWorkAxis::Commits,
        ),
        (
            OptimizationWorkBudget::new(20, 20, 20, 20, 3).unwrap(),
            X86MovR64Imm32SignExtendedMaterializationWorkAxis::Iterations,
        ),
    ] {
        assert_eq!(
            super::compute::compute_from_parts(
                &fixture.selected,
                fixture.selected_identity,
                &fixture.source,
                fixture.source_identity,
                &fixture.physical,
                budget,
            ),
            Err(X86MovR64Imm32SignExtendedMaterializationError::BudgetExceeded(axis)),
            "the first unit beyond the exact {axis:?} budget must fail closed",
        );
    }
}
