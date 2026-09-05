use omega_optimization_core::{OptimizationWorkBudget, PostAllocationOptimizationManifestIdentity};
use omega_register_model::{
    PhysicalRegisterModel, PreservationConvention, RegisterClass, RegisterClassId,
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    RegisterOperandAccess, RegisterUnit, RegisterUnitId, RegisterUnitKind, RegisterView,
    RegisterViewId, RegisterWriteSemantics, TargetRegisterEnvironmentIdentity,
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
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{BlockId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::{movn_recipe, zero_seed_word_count};
use crate::{
    Aarch64MovnMaterializationAttemptOutcome, Aarch64MovnMaterializationError,
    Aarch64MovnMaterializationWorkAxis, Aarch64MovnPatch, MachineAlternativeChoiceRule,
    PhysicalOperandFootprint, PostAllocationMachineBlock, PostAllocationMachineFunction,
    PostAllocationMachineIdentity, PostAllocationMachineInstruction, PostAllocationMachinePlan,
    PreAllocationMachineEffectIdentity,
};

#[test]
fn movn_recipe_uses_lowest_eligible_seed_and_ascending_patches() {
    let recipe = movn_recipe(0xffff_1234_ffff_abcd);
    assert_eq!(recipe.seed_halfword, 0);
    assert_eq!(recipe.seed_immediate, !0xabcd);
    assert_eq!(
        recipe.patches,
        vec![Aarch64MovnPatch {
            halfword: 2,
            immediate: 0x1234,
        }]
    );
    assert_eq!(recipe.word_count(), Some(2));
}

#[test]
fn strict_word_count_policy_only_prefers_real_shrinks() {
    assert_eq!(zero_seed_word_count(0), 1);
    assert_eq!(movn_recipe(0).word_count(), Some(4));
    assert_eq!(zero_seed_word_count(u64::MAX), 4);
    assert_eq!(movn_recipe(u64::MAX).word_count(), Some(1));
    assert_eq!(zero_seed_word_count(0xffff_0000_0000_0001), 2);
    assert_eq!(movn_recipe(0xffff_0000_0000_0001).word_count(), Some(3));
}

fn physical() -> ValidatedPhysicalRegisterModel {
    validate_physical_register_model(PhysicalRegisterModel {
        architecture: Architecture::Aarch64,
        units: vec![RegisterUnit {
            id: RegisterUnitId(0),
            name: "x0.storage".into(),
            bits: 64,
            kind: RegisterUnitKind::IntegerLane,
        }],
        views: vec![RegisterView {
            id: RegisterViewId(0),
            name: "x0".into(),
            class: RegisterClassId(0),
            units: vec![RegisterUnitId(0)],
            write_units: vec![RegisterUnitId(0)],
            bits: 64,
            write_semantics: RegisterWriteSemantics::ExactView,
            allocatable: true,
        }],
        classes: vec![RegisterClass {
            id: RegisterClassId(0),
            name: "gpr64".into(),
            views: vec![RegisterViewId(0)],
        }],
        conventions: vec![PreservationConvention {
            name: "test".into(),
            argument_views: vec![RegisterViewId(0)],
            result_views: vec![RegisterViewId(0)],
            caller_saved: vec![RegisterUnitId(0)],
            callee_saved: vec![],
            fixed: vec![],
            stack_alignment: 16,
            red_zone_bytes: 0,
        }],
        reservations: vec![],
    })
    .unwrap()
}

fn constraint() -> RegisterConstraintKey {
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: 0,
    }
}

fn selected_instruction(id: u32, value: u64) -> SelectedInstruction {
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
            class: RegisterClassId(0),
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

fn machine_instruction(id: u32) -> PostAllocationMachineInstruction {
    PostAllocationMachineInstruction {
        instruction: SelectedInstructionId(id),
        alternative: MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::MaterializeI64,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(16),
            },
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![0]),
        },
        operands: vec![PhysicalOperandFootprint {
            operand: 0,
            virtual_register: VirtualRegisterId(id),
            class: RegisterClassId(0),
            view: RegisterViewId(0),
            access: RegisterOperandAccess::Def,
            storage_units: vec![RegisterUnitId(0)],
            read_units: vec![],
            write_units: vec![RegisterUnitId(0)],
            write_semantics: Some(RegisterWriteSemantics::ExactView),
        }],
        implicit_unit_uses: vec![],
        implicit_unit_defs: vec![],
        implicit_unit_clobbers: vec![],
        unit_uses: vec![],
        unit_defs: vec![RegisterUnitId(0)],
        unit_clobbers: vec![],
    }
}

fn fixture() -> (
    SelectedInstructionPlan,
    SelectedInstructionPlanIdentity,
    PostAllocationMachinePlan,
    PostAllocationMachineIdentity,
    ValidatedPhysicalRegisterModel,
) {
    let machine = MachineId::new(1).unwrap();
    let block = SelectedBlockId(0);
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
        target: NativeTarget::linux_arm64(),
        entry: machine,
        functions: vec![SelectedFunction {
            machine,
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            entry_block: block,
            virtual_registers: vec![],
            blocks: vec![SelectedBlock {
                id: block,
                source_block: BlockId::new(1).unwrap(),
                instructions: vec![
                    selected_instruction(1, u64::MAX),
                    selected_instruction(2, 0xffff_1234_ffff_abcd),
                    selected_instruction(3, 7),
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
    let physical = physical();
    let source_identity = PostAllocationMachineIdentity::from_bytes([3; 32]);
    let return_machine = PostAllocationMachineInstruction {
        instruction: SelectedInstructionId(4),
        alternative: MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::ReturnUnit,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(4),
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
        effects: PreAllocationMachineEffectIdentity::from_bytes([4; 32]),
        ranges: omega_selected_instructions_to_register_homes::LiveRangeIdentity::from_bytes(
            [5; 32],
        ),
        legality:
            omega_selected_instructions_to_register_homes::AllocationLegalityIdentity::from_bytes(
                [6; 32],
            ),
        homes: omega_selected_instructions_to_register_homes::RegisterHomeIdentity::from_bytes(
            [7; 32],
        ),
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes([8; 32]),
        target: NativeTarget::linux_arm64(),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([9; 32]),
        physical_register_model: physical.identity(),
        register_constraints: RegisterConstraintCatalogIdentity::from_bytes([10; 32]),
        machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([11; 32]),
        choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        functions: vec![PostAllocationMachineFunction {
            machine,
            blocks: vec![PostAllocationMachineBlock {
                block,
                instructions: vec![
                    machine_instruction(1),
                    machine_instruction(2),
                    machine_instruction(3),
                    return_machine,
                ],
            }],
        }],
        structural_unit_functions: vec![],
    };
    (
        selected,
        selected_identity,
        source,
        source_identity,
        physical,
    )
}

#[test]
fn compute_and_independent_replay_bind_every_action_and_retention() {
    let (selected, selected_identity, source, source_identity, physical) = fixture();
    let budget = OptimizationWorkBudget::new(20, 20, 20, 20, 20).unwrap();
    let computed = super::compute_from_parts(
        &selected,
        selected_identity,
        &source,
        source_identity,
        &physical,
        budget,
    )
    .unwrap();
    let replayed = crate::rules::aarch64::materialize_i64_movn::validate::replay_from_parts(
        &selected,
        selected_identity,
        &source,
        source_identity,
        &physical,
        budget,
    )
    .unwrap();
    assert_eq!(computed, replayed);
    assert_eq!(computed.actions.len(), 2);
    assert_eq!(computed.attempts.len(), 6);
    assert_eq!(computed.usage.iterations, 3);
    assert_eq!(computed.usage.rule_evaluations, 6);
    assert_eq!(computed.actions[0].literal_bits, u64::MAX);
    assert_eq!(computed.actions[0].baseline_word_count, 4);
    assert_eq!(computed.actions[0].recipe.word_count(), Some(1));
    assert_eq!(computed.actions[1].recipe.word_count(), Some(2));
    assert_eq!(
        computed.attempts.last().unwrap().outcome,
        Aarch64MovnMaterializationAttemptOutcome::BaselineNotLonger
    );

    let mut corrupted = computed.clone();
    corrupted.actions[0].literal_bits ^= 1;
    assert_ne!(corrupted, replayed);
}

#[test]
fn compute_charges_the_exact_bounded_scan() {
    let (selected, selected_identity, source, source_identity, physical) = fixture();
    let exact_budget = OptimizationWorkBudget::new(6, 2, 2, 2, 3).unwrap();
    let computed = super::compute_from_parts(
        &selected,
        selected_identity,
        &source,
        source_identity,
        &physical,
        exact_budget,
    )
    .unwrap();
    assert_eq!(computed.usage.rule_evaluations, 6);
    assert_eq!(computed.usage.candidates, 2);
    assert_eq!(computed.usage.validation_steps, 2);
    assert_eq!(computed.usage.commits, 2);
    assert_eq!(computed.usage.iterations, 3);

    for (budget, axis) in [
        (
            OptimizationWorkBudget::new(5, 2, 2, 2, 3).unwrap(),
            Aarch64MovnMaterializationWorkAxis::RuleEvaluations,
        ),
        (
            OptimizationWorkBudget::new(6, 1, 2, 2, 3).unwrap(),
            Aarch64MovnMaterializationWorkAxis::Candidates,
        ),
        (
            OptimizationWorkBudget::new(6, 2, 1, 2, 3).unwrap(),
            Aarch64MovnMaterializationWorkAxis::ValidationSteps,
        ),
        (
            OptimizationWorkBudget::new(6, 2, 2, 1, 3).unwrap(),
            Aarch64MovnMaterializationWorkAxis::Commits,
        ),
        (
            OptimizationWorkBudget::new(6, 2, 2, 2, 2).unwrap(),
            Aarch64MovnMaterializationWorkAxis::Iterations,
        ),
    ] {
        assert_eq!(
            super::compute_from_parts(
                &selected,
                selected_identity,
                &source,
                source_identity,
                &physical,
                budget,
            ),
            Err(Aarch64MovnMaterializationError::BudgetExceeded(axis)),
            "a one-below {axis:?} budget must fail on its typed axis",
        );
    }
}
