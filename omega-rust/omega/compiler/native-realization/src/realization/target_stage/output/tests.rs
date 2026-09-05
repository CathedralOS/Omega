use super::*;
use crate::tests::fixtures::checked_source::checked;
use optimization_core::{Optimization, OptimizationSelections};
use proof_admission::AdmissionProfile;
use target::NativeTarget;

fn optimized_target(
    target: NativeTarget,
    selections: OptimizationSelections,
) -> ValidatedOptimizedTargetOperations {
    let checked = checked("data Main {} machine Main::launch() {}");
    let artifact = terminal_production::produce_terminal_artifact(&checked, "Main::launch")
        .expect("publish independent Terminal fixture");
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
    )
    .expect("verified abstract input");
    let abstract_program = crate::optimize_verified_abstract_input(
        input,
        crate::compiler_baseline_request_v1(&selections),
    )
    .expect("complete abstract optimization");
    abstract_operations_to_target_operations::lower_validated_abstract_to_target_operations(
        abstract_program,
        target,
        &[],
        None,
        &[],
        &[],
    )
    .expect("independently validated target lowering")
}

#[test]
fn empty_and_selected_target_results_share_the_original_current_program() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for selections in [
            OptimizationSelections::default(),
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap(),
        ] {
            let evidence = optimized_target(target, selections);
            let original = evidence.shared_program();
            assert!(std::ptr::eq(&original.plan, evidence.target_operations()));
            let stage = NativeTargetStageResult::ordinary(evidence);
            assert!(Arc::ptr_eq(&stage.program, &original));
            let (program, evidence) = stage.into_parts().expect("exact current/evidence join");
            assert!(Arc::ptr_eq(&program, &original));
            drop(evidence);
            drop(original);
            assert_eq!(Arc::strong_count(&program), 1);
            assert_eq!(program.plan.target, target);
            assert_eq!(program.plan.functions.len(), 1);
            assert!(program.native_callback_arguments.is_empty());
        }
    }
}

#[test]
fn changed_target_contents_reject_even_with_unchanged_root_ids() {
    let evidence = optimized_target(NativeTarget::linux_x64(), OptimizationSelections::default());
    let original = evidence.shared_program();
    let mut stage = NativeTargetStageResult::ordinary(evidence);
    Arc::make_mut(&mut stage.program).plan.functions.clear();
    assert_eq!(stage.program.plan.psi, original.plan.psi);
    assert_eq!(stage.program.plan.entry, original.plan.entry);
    assert_eq!(stage.program.plan.target, original.plan.target);
    assert_eq!(
        original.plan.functions.len(),
        1,
        "replay input stays unchanged"
    );
    assert!(matches!(
        stage.into_parts(),
        Err("current target program differs from its retained translation evidence")
    ));
}

#[test]
fn substituted_target_profile_rejects_without_rewriting_retained_evidence() {
    let evidence = optimized_target(NativeTarget::linux_x64(), OptimizationSelections::default());
    let original = evidence.shared_program();
    let mut stage = NativeTargetStageResult::ordinary(evidence);
    Arc::make_mut(&mut stage.program).plan.target = NativeTarget::linux_arm64();
    assert_eq!(original.plan.target, NativeTarget::linux_x64());
    assert!(matches!(
        stage.into_parts(),
        Err("current target program differs from its retained translation evidence")
    ));
}

#[test]
fn ordinary_and_ranked_outputs_own_the_same_representation() {
    let checked = checked(
        r#"
            data Token { value: i32; }
            data Root {}
            machine Root::countdown(token: Token, remaining: u32)
            terminates by remaining -> Nat::Descending;
            {
                transition remaining > 0 {
                    true -> countdown(token, remaining - 1)
                    _ -> done(token)
                }
                state done(token: Token) {}
            }
        "#,
    );
    let artifact = terminal_production::produce_terminal_artifact(&checked, "Root::countdown")
        .expect("publish ranked Terminal fixture");
    let native =
        terminal_psi_to_abstract_operations::lower_artifact_sections_for_native_realization(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
        )
        .expect("ranked native admission");
    let terminal_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(
        ranked,
    ) = native
    else {
        panic!("fixture must retain ranked authority");
    };
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let ranked_plan =
            abstract_operations_to_target_operations::lower_ranked_to_target_operations(
                &ranked, target,
            )
            .unwrap();
        let ranked_stage = NativeTargetStageResult::ranked(ranked_plan);
        let ordinary = optimized_target(target, OptimizationSelections::default());
        let ordinary_stage = NativeTargetStageResult::ordinary(ordinary);
        let mut programs = Vec::new();
        for (stage, is_ranked) in [(ordinary_stage, false), (ranked_stage, true)] {
            let (program, evidence) = stage.into_parts().expect("current target output");
            assert_eq!(
                matches!(evidence, NativeTargetStageEvidence::Ranked),
                is_ranked
            );
            programs.push(program);
        }
        assert_eq!(programs.len(), 2);
        for program in programs {
            assert_eq!(program.plan.target, target);
            assert_eq!(Arc::strong_count(&program), 1);
            assert!(program.native_callback_arguments.is_empty());
            let assigned = target_operations_to_assigned_target_operations::assign_registers_with_native_callbacks(
                &program,
            )
            .expect("assignment consumes current representation for both roles");
            let previous =
                target_operations_to_assigned_target_operations::assign_registers(&program.plan)
                    .expect("prior callback-free assignment");
            assert_eq!(assigned.plan, previous);
            assert_eq!(
                machine_emission::emit_machine_code_with_native_callbacks(&assigned).unwrap(),
                machine_emission::emit_machine_code(&previous).unwrap(),
                "shared assignment and emission preserve complete machine bytes and evidence",
            );
        }
    }
}
