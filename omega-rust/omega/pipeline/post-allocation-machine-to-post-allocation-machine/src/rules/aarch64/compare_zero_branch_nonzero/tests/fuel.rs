use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    Aarch64CbnzFusionAttemptOutcome, Aarch64CbnzFusionError, Aarch64CbnzFusionPlan,
    Aarch64CbnzFusionWorkAxis, Aarch64CbnzInstructionDisposition, aarch64_cbnz_fusion_identity,
};

#[test]
fn fuel_bearing_compare_fuses_and_independent_replay_preserves_its_selected_root() {
    let fixture = super::fixture::fixture();
    let compare = &fixture.selected.functions[0].blocks[0].instructions[0];
    let expected_fuel = compare.provenance.fuel.clone();
    assert!(!expected_fuel.is_empty());

    let plan =
        super::super::compute::compute_from_inputs(fixture.inputs(), super::fixture::budget())
            .unwrap();
    let validated = super::super::validate::validate_from_inputs(fixture.inputs(), plan.clone())
        .expect("independent replay accepts exact fuel-bearing fusion");

    assert_eq!(validated.plan(), &plan);
    assert_eq!(plan.selected, fixture.selected_identity);
    assert_eq!(plan.actions.len(), 1);
    assert_eq!(plan.usage.rule_evaluations, 2);
    assert_eq!(plan.usage.candidates, 1);
    assert_eq!(plan.usage.validation_steps, 1);
    assert_eq!(plan.usage.commits, 1);
    assert_eq!(plan.usage.iterations, 2);
    assert_eq!(
        plan.attempts
            .iter()
            .map(|attempt| attempt.outcome)
            .collect::<Vec<_>>(),
        [
            Aarch64CbnzFusionAttemptOutcome::SelectedForFusion,
            Aarch64CbnzFusionAttemptOutcome::AlreadyFused,
        ]
    );
    assert_eq!(
        fixture.selected.functions[0].blocks[0].instructions[0]
            .provenance
            .fuel,
        expected_fuel,
        "fusion disposition must not rewrite logical compare settlements"
    );
}

#[test]
fn two_fuel_bearing_pairs_pin_work_and_every_first_over_budget_axis() {
    let fixture = super::fixture::two_pair_fixture();
    let exact = OptimizationWorkBudget::new(5, 2, 2, 2, 3).unwrap();
    let plan = super::super::compute::compute_from_inputs(fixture.inputs(), exact).unwrap();
    super::super::validate::validate_from_inputs(fixture.inputs(), plan.clone()).unwrap();
    assert_eq!(plan.actions.len(), 2);
    assert_eq!(plan.usage.rule_evaluations, 5);
    assert_eq!(plan.usage.candidates, 2);
    assert_eq!(plan.usage.validation_steps, 2);
    assert_eq!(plan.usage.commits, 2);
    assert_eq!(plan.usage.iterations, 3);

    for (budget, axis) in [
        (
            OptimizationWorkBudget::new(4, 2, 2, 2, 3).unwrap(),
            Aarch64CbnzFusionWorkAxis::RuleEvaluations,
        ),
        (
            OptimizationWorkBudget::new(5, 1, 2, 2, 3).unwrap(),
            Aarch64CbnzFusionWorkAxis::Candidates,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 1, 2, 3).unwrap(),
            Aarch64CbnzFusionWorkAxis::ValidationSteps,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 2, 1, 3).unwrap(),
            Aarch64CbnzFusionWorkAxis::Commits,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 2, 2, 2).unwrap(),
            Aarch64CbnzFusionWorkAxis::Iterations,
        ),
    ] {
        assert_eq!(
            super::super::compute::compute_from_inputs(fixture.inputs(), budget),
            Err(Aarch64CbnzFusionError::BudgetExceeded(axis))
        );
    }
}

#[test]
fn fuel_bearing_fusion_replay_rejects_action_and_authenticated_root_corruption() {
    let fixture = super::fixture::fixture();
    let plan =
        super::super::compute::compute_from_inputs(fixture.inputs(), super::fixture::budget())
            .unwrap();

    let mut corrupted = plan.clone();
    corrupted.actions[0].compare.0 += 1;
    assert_eq!(
        super::super::validate::validate_from_inputs(fixture.inputs(), corrupted),
        Err(Aarch64CbnzFusionError::ArtifactMismatch)
    );

    let mut wrong_selected_root = fixture.selected_identity.bytes();
    wrong_selected_root[0] ^= 0x80;
    let mut corrupted = plan;
    corrupted.selected =
        selected_instructions::SelectedInstructionPlanIdentity::from_bytes(wrong_selected_root);
    assert_eq!(
        super::super::validate::validate_from_inputs(fixture.inputs(), corrupted),
        Err(Aarch64CbnzFusionError::ArtifactMismatch)
    );
}

#[test]
fn legacy_fuel_refusal_decodes_but_is_stale_under_current_replay() {
    let fixture = super::fixture::fixture();
    let mut legacy =
        super::super::compute::compute_from_inputs(fixture.inputs(), super::fixture::budget())
            .unwrap();
    legacy.attempts.truncate(1);
    legacy.attempts[0].outcome = Aarch64CbnzFusionAttemptOutcome::CompareCarriesFuel;
    legacy.actions.clear();
    for function in &mut legacy.functions {
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                instruction.disposition = Aarch64CbnzInstructionDisposition::RetainedV1;
            }
        }
    }
    legacy.usage = OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates: 0,
        validation_steps: 0,
        commits: 0,
        iterations: 1,
    };
    legacy.output_revision = super::super::identity::revision_identity(
        legacy.source,
        legacy.selected,
        legacy.liveness,
        legacy.target,
        legacy.physical_register_model,
        &legacy.functions,
    );
    legacy.identity = aarch64_cbnz_fusion_identity(&legacy);

    let decoded = Aarch64CbnzFusionPlan::decode(&legacy.encode()).unwrap();
    assert_eq!(decoded, legacy, "legacy refusal tag remains decodable");
    assert_eq!(
        super::super::validate::validate_from_inputs(fixture.inputs(), decoded),
        Err(Aarch64CbnzFusionError::ArtifactMismatch),
        "current replay must reject the obsolete fuel refusal"
    );
}
