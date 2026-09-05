use omega_isa_x86_64::{
    encode_x86_64_xor_zero_i64_materialization, x86_64_physical_register_model,
};
use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, PostAllocationOptimizationManifestIdentity,
};
use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    RegisterOperandAccess, RegisterUnitId, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, validate_physical_register_model,
};
use omega_selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineEffectCatalogIdentity, MachineEncodedEffects,
    MachineLatencyKnowledge, MachineSizeKnowledge, SelectedBlock, SelectedBlockId,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    SelectedOperand, SelectedTerminator, VirtualRegisterId,
};
use omega_selected_instructions_to_register_homes::{
    BlockLiveness, FunctionLiveness, InstructionLiveness, LivenessIdentity, LivenessPlan,
    LivenessPosition,
};
use omega_target::NativeTarget;
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{BlockId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::{
    X86_MOVABS_I64_BYTE_COUNT, X86_XOR_R64_SELF_BYTE_COUNT, X86XorZeroInstructionDisposition,
    X86XorZeroMaterializationAttemptOutcome, X86XorZeroMaterializationError,
    X86XorZeroMaterializationWorkAxis,
};
use crate::{
    MachineAlternativeChoiceRule, PhysicalOperandFootprint, PostAllocationMachineBlock,
    PostAllocationMachineFunction, PostAllocationMachineIdentity, PostAllocationMachineInstruction,
    PostAllocationMachinePlan, PreAllocationMachineEffectIdentity,
};

struct Fixture {
    selected: SelectedInstructionPlan,
    selected_identity: SelectedInstructionPlanIdentity,
    liveness: LivenessPlan,
    liveness_identity: LivenessIdentity,
    source: PostAllocationMachinePlan,
    source_identity: PostAllocationMachineIdentity,
    physical: ValidatedPhysicalRegisterModel,
    rflags: RegisterUnitId,
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
    physical: &ValidatedPhysicalRegisterModel,
) -> PostAllocationMachineInstruction {
    let rax = physical.model().view_named("rax").unwrap();
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
            class: rax.class,
            view: rax.id,
            access: RegisterOperandAccess::Def,
            storage_units: rax.units.clone(),
            read_units: vec![],
            write_units: rax.write_units.clone(),
            write_semantics: Some(rax.write_semantics),
        }],
        implicit_unit_uses: vec![],
        implicit_unit_defs: vec![],
        implicit_unit_clobbers: vec![],
        unit_uses: vec![],
        unit_defs: rax.write_units.clone(),
        unit_clobbers: vec![],
    }
}

fn instruction_liveness(id: u32, unit_live_out: Vec<RegisterUnitId>) -> InstructionLiveness {
    InstructionLiveness {
        position: LivenessPosition(id - 1),
        instruction: SelectedInstructionId(id),
        virtual_uses: vec![],
        virtual_defs: vec![],
        virtual_live_in: vec![],
        virtual_live_out: vec![],
        unit_uses: vec![],
        unit_defs: vec![],
        unit_clobbers: vec![],
        unit_live_in: unit_live_out.clone(),
        unit_live_out,
    }
}

fn fixture() -> Fixture {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rflags = physical.model().view_named("rflags").unwrap().units[0];
    let machine = MachineId::new(1).unwrap();
    let block = SelectedBlockId(0);
    let source_block = BlockId::new(1).unwrap();
    let return_instruction = SelectedInstruction {
        id: SelectedInstructionId(4),
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
                    selected_materialization(2, 0, &physical),
                    selected_materialization(3, 7, &physical),
                ],
                terminator: SelectedTerminator::Return {
                    instruction: return_instruction,
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        }],
        structural_unit_functions: vec![],
        projected_structural_call_returns: vec![],
    };
    let selected_identity = SelectedInstructionPlanIdentity::from_bytes([2; 32]);
    let liveness = LivenessPlan {
        selected: selected_identity,
        optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"xor-zero-test"),
        fuel_schedule: selected.fuel_schedule,
        target: selected.target,
        functions: vec![FunctionLiveness {
            machine,
            entry_definitions: vec![],
            operand_positions: vec![],
            blocks: vec![BlockLiveness {
                block,
                source_block,
                virtual_live_in: vec![],
                virtual_live_out: vec![],
                unit_live_in: vec![],
                unit_live_out: vec![],
                instructions: vec![
                    instruction_liveness(1, vec![]),
                    instruction_liveness(2, vec![rflags]),
                    instruction_liveness(3, vec![]),
                    instruction_liveness(4, vec![]),
                ],
                successors: vec![],
            }],
        }],
        structural_unit_functions: vec![],
    };
    let liveness_identity = LivenessIdentity::from_bytes([3; 32]);
    let source_identity = PostAllocationMachineIdentity::from_bytes([4; 32]);
    let return_machine = PostAllocationMachineInstruction {
        instruction: SelectedInstructionId(4),
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
        ranges: omega_selected_instructions_to_register_homes::LiveRangeIdentity::from_bytes(
            [6; 32],
        ),
        legality:
            omega_selected_instructions_to_register_homes::AllocationLegalityIdentity::from_bytes(
                [7; 32],
            ),
        homes: omega_selected_instructions_to_register_homes::RegisterHomeIdentity::from_bytes(
            [8; 32],
        ),
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
                    machine_materialization(1, &physical),
                    machine_materialization(2, &physical),
                    machine_materialization(3, &physical),
                    return_machine,
                ],
            }],
        }],
        structural_unit_functions: vec![],
    };
    Fixture {
        selected,
        selected_identity,
        liveness,
        liveness_identity,
        source,
        source_identity,
        physical,
        rflags,
    }
}

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(20, 20, 20, 20, 20).unwrap()
}

fn compute(fixture: &Fixture) -> super::X86XorZeroMaterializationPlan {
    super::compute::compute_from_parts(
        &fixture.selected,
        fixture.selected_identity,
        &fixture.liveness,
        fixture.liveness_identity,
        &fixture.source,
        fixture.source_identity,
        &fixture.physical,
        budget(),
    )
    .unwrap()
}

#[test]
fn selects_only_zero_with_every_rflags_unit_dead_and_replay_agrees() {
    let fixture = fixture();
    let computed = compute(&fixture);
    let replayed = super::validate::replay_from_parts(
        &fixture.selected,
        fixture.selected_identity,
        &fixture.liveness,
        fixture.liveness_identity,
        &fixture.source,
        fixture.source_identity,
        &fixture.physical,
        budget(),
    )
    .unwrap();

    assert_eq!(computed, replayed);
    assert_eq!(computed.actions.len(), 1);
    assert_eq!(computed.actions[0].instruction, SelectedInstructionId(1));
    assert_eq!(computed.actions[0].rflags_units, [fixture.rflags]);
    assert_eq!(
        computed.actions[0].baseline_byte_count,
        X86_MOVABS_I64_BYTE_COUNT
    );
    assert_eq!(
        computed.actions[0].selected_byte_count,
        X86_XOR_R64_SELF_BYTE_COUNT
    );
    let encoded = encode_x86_64_xor_zero_i64_materialization(
        &fixture.physical,
        computed.actions[0].destination.view,
    )
    .unwrap();
    assert_eq!(
        encoded.bytes().len(),
        usize::from(X86_XOR_R64_SELF_BYTE_COUNT)
    );
    assert_eq!(
        encoded.footprint().encoded.implicit_unit_clobbers,
        computed.actions[0].rflags_units
    );
    assert_eq!(computed.usage.iterations, 2);
    assert_eq!(computed.usage.rule_evaluations, 4);
    assert_eq!(computed.usage.candidates, 1);
    assert_eq!(computed.usage.validation_steps, 1);
    assert_eq!(computed.usage.commits, 1);
    assert_eq!(computed.attempts.len(), 4);
    assert_eq!(
        computed.attempts[2].outcome,
        X86XorZeroMaterializationAttemptOutcome::RflagsLiveOut
    );
    assert_eq!(
        computed.attempts[3].outcome,
        X86XorZeroMaterializationAttemptOutcome::NonZeroLiteral
    );
    assert!(matches!(
        computed.functions[0].blocks[0].instructions[0].disposition,
        X86XorZeroInstructionDisposition::XorZeroMaterializationV1 { .. }
    ));
    assert_eq!(
        computed.functions[0].blocks[0].instructions[1].disposition,
        X86XorZeroInstructionDisposition::RetainedV1
    );
}

#[test]
fn liveness_corruption_changes_selection_and_is_bound_into_identity() {
    let fixture = fixture();
    let baseline = compute(&fixture);
    let mut corrupted = fixture;
    corrupted.liveness.functions[0].blocks[0].instructions[0].unit_live_out =
        vec![corrupted.rflags];
    let with_live_flags = compute(&corrupted);

    assert!(with_live_flags.actions.is_empty());
    assert_ne!(baseline, with_live_flags);
    assert_ne!(baseline.identity, with_live_flags.identity);
    assert_eq!(
        with_live_flags.attempts[0].outcome,
        X86XorZeroMaterializationAttemptOutcome::RflagsLiveOut
    );
}

#[test]
fn independent_replay_exposes_action_corruption() {
    let fixture = fixture();
    let replayed = super::validate::replay_from_parts(
        &fixture.selected,
        fixture.selected_identity,
        &fixture.liveness,
        fixture.liveness_identity,
        &fixture.source,
        fixture.source_identity,
        &fixture.physical,
        budget(),
    )
    .unwrap();
    let mut corrupted = replayed.clone();
    corrupted.actions[0].selected_byte_count = 4;
    assert_eq!(
        super::validate::validate_from_parts(
            &fixture.selected,
            fixture.selected_identity,
            &fixture.liveness,
            fixture.liveness_identity,
            &fixture.source,
            fixture.source_identity,
            &fixture.physical,
            &corrupted,
        ),
        Err(X86XorZeroMaterializationError::ArtifactMismatch)
    );
}

#[test]
fn every_work_axis_exhausts_at_its_exact_boundary() {
    let mut fixture = fixture();
    fixture.liveness.functions[0].blocks[0].instructions[1]
        .unit_live_out
        .clear();
    let successful = compute(&fixture);
    assert_eq!(successful.usage.iterations, 3);
    assert_eq!(successful.usage.rule_evaluations, 6);
    assert_eq!(successful.usage.candidates, 2);
    assert_eq!(successful.usage.validation_steps, 2);
    assert_eq!(successful.usage.commits, 2);

    for (budget, axis) in [
        (
            OptimizationWorkBudget::new(5, 20, 20, 20, 20).unwrap(),
            X86XorZeroMaterializationWorkAxis::RuleEvaluations,
        ),
        (
            OptimizationWorkBudget::new(20, 1, 20, 20, 20).unwrap(),
            X86XorZeroMaterializationWorkAxis::Candidates,
        ),
        (
            OptimizationWorkBudget::new(20, 20, 1, 20, 20).unwrap(),
            X86XorZeroMaterializationWorkAxis::ValidationSteps,
        ),
        (
            OptimizationWorkBudget::new(20, 20, 20, 1, 20).unwrap(),
            X86XorZeroMaterializationWorkAxis::Commits,
        ),
        (
            OptimizationWorkBudget::new(20, 20, 20, 20, 2).unwrap(),
            X86XorZeroMaterializationWorkAxis::Iterations,
        ),
    ] {
        assert_eq!(
            super::compute::compute_from_parts(
                &fixture.selected,
                fixture.selected_identity,
                &fixture.liveness,
                fixture.liveness_identity,
                &fixture.source,
                fixture.source_identity,
                &fixture.physical,
                budget,
            ),
            Err(X86XorZeroMaterializationError::BudgetExceeded(axis)),
            "the first unit beyond the exact {axis:?} budget must fail closed",
        );
    }
}
