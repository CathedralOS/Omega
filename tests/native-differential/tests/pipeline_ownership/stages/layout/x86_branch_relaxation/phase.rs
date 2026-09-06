//! Identity and selected layout phases retain data, not unchecked admission.

use crate::tests::*;
use optimization_core::OptimizationExecutionPhase;

#[test]
fn layout_phase_replays_exact_selection_current_and_evidence() {
    let homes = super::fixture::physical_homes();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .unwrap();
    let baseline =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .unwrap();
    let empty = OptimizationSelections::new([])
        .unwrap()
        .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    let enabled = OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
        .unwrap()
        .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    let execute = |selections| {
        execute_resolved_layout_optimization(
            selected,
            &machine,
            physical,
            &encoding,
            None,
            &baseline,
            selections,
            selected_lowering_budget(),
        )
        .unwrap()
    };
    let replay = |selections, artifact| {
        validate_resolved_layout_optimization(
            selected, &machine, physical, &encoding, None, &baseline, selections, artifact,
        )
    };
    let identity = execute(&empty);
    let relaxed = execute(&enabled);
    assert_eq!(replay(&empty, &identity), Ok(()));
    assert_eq!(replay(&enabled, &relaxed), Ok(()));
    assert!(identity.relaxation().is_none());
    assert_eq!(identity.layout(), baseline.program());
    assert!(std::sync::Arc::ptr_eq(
        &identity.shared_layout(),
        &baseline.shared_program()
    ));
    assert!(!relaxed.relaxation().unwrap().actions().is_empty());
    assert_ne!(identity.layout(), relaxed.layout());
    assert_eq!(
        replay(&empty, &relaxed),
        Err(ResolvedLayoutOptimizationError::SelectionMismatch)
    );
    assert_eq!(
        replay(&enabled, &identity),
        Err(ResolvedLayoutOptimizationError::SelectionMismatch)
    );

    let mut wrong_current = relaxed.clone();
    wrong_current.substitute_shared_layout_for_test(identity.shared_layout());
    assert_eq!(
        replay(&enabled, &wrong_current),
        Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch)
    );
    let mut wrong_identity = identity.clone();
    wrong_identity.substitute_shared_layout_for_test(relaxed.shared_layout());
    assert_eq!(
        replay(&empty, &wrong_identity),
        Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch)
    );
    let mut wrong_evidence = relaxed.clone();
    wrong_evidence
        .relaxation_mut_for_test()
        .unwrap()
        .corrupt_first_action_bytes_and_reauthenticate_for_test();
    assert_eq!(
        replay(&enabled, &wrong_evidence),
        Err(ResolvedLayoutOptimizationError::Relaxation(
            OptimizedX86BranchRelaxationError::ArtifactMismatch,
        ))
    );
    let alternate = super::fixture::alternate_direct_realization();
    let mut substituted_evidence = relaxed.clone();
    *substituted_evidence.relaxation_mut_for_test().unwrap() = alternate.relaxation().clone();
    assert!(replay(&enabled, &substituted_evidence).is_err());

    let retained_identity = identity.shared_layout();
    let retained_relaxed = relaxed.shared_layout();
    let expected_identity = identity.layout().clone();
    let expected_relaxed = relaxed.layout().clone();
    drop(identity);
    drop(relaxed);
    drop(wrong_current);
    drop(wrong_identity);
    drop(wrong_evidence);
    drop(substituted_evidence);
    drop(alternate);
    drop(baseline);
    drop(encoding);
    drop(machine);
    drop(homes);
    assert_eq!(retained_identity.as_ref(), &expected_identity);
    assert_eq!(retained_relaxed.as_ref(), &expected_relaxed);
    assert_eq!(
        retained_identity.identity(),
        retained_identity.recomputed_identity()
    );
    assert_eq!(
        retained_relaxed.identity(),
        retained_relaxed.recomputed_identity()
    );
}
