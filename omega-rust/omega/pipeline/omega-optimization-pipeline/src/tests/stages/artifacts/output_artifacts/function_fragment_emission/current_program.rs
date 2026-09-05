//! Current artifacts survive producer history; replay still binds every input.

use std::sync::Arc;

use crate::tests::*;

pub(super) fn source(
    target: NativeTarget,
    selections: OptimizationSelections,
) -> StagedOptimizedFunctionFragmentEmissionSource {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        crate::OptimizationPipelineRequest::new(selections, selected_lowering_budget()),
    )
    .unwrap();
    stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
        .unwrap()
        .into_function_fragment_emission_source()
}

#[test]
fn emission_retains_original_current_artifacts_without_the_producer_history() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for selections in [
            OptimizationSelections::default(),
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap(),
        ] {
            let source = source(target, selections);
            let replay = source.replay();
            let retained = source.program().clone();
            assert!(Arc::ptr_eq(
                &retained.selected,
                &replay.shared_selected_plan()
            ));
            assert!(Arc::ptr_eq(
                &retained.homes,
                &replay.register_homes().shared_plan()
            ));
            assert!(Arc::ptr_eq(
                &retained.machine,
                &replay.machine().machine().shared_plan()
            ));
            assert!(Arc::ptr_eq(
                &retained.effects,
                &replay.machine().effects().shared_plan()
            ));
            assert!(Arc::ptr_eq(
                &retained.layout,
                &replay.resolved_layout().shared_program()
            ));
            assert!(std::ptr::eq(
                source.encoding().rows().as_ptr(),
                replay.encoding().rows().as_ptr()
            ));
            assert!(std::ptr::eq(
                source.exit_contract().contract(),
                replay.exit_contract().contract()
            ));
            if let Some(protocol) = source.frame_protocol() {
                assert!(std::ptr::eq(
                    protocol.plan(),
                    replay.fixed_frame_realization().unwrap().protocol().plan()
                ));
            }
            let emitted = stage_optimized_function_fragment_emission(source).unwrap();
            assert_eq!(emitted.source().program(), &retained);
            let original_machine = retained.machine.identity;
            drop(emitted);
            assert_eq!(retained.machine.identity, original_machine);
            assert_eq!(
                retained.layout.identity,
                retained.layout.recomputed_identity()
            );
            assert_eq!(retained.layout.machine, retained.machine.identity);
            assert_eq!(retained.machine.selected, retained.layout.selected);
        }
    }
}

#[test]
fn emission_rejects_individually_canonical_substituted_current_artifacts() {
    let other = source(
        NativeTarget::linux_arm64(),
        OptimizationSelections::default(),
    );
    let other = other.program().clone();
    for component in 0..5 {
        let mut candidate = source(NativeTarget::linux_x64(), OptimizationSelections::default());
        let program = candidate.program_mut();
        match component {
            0 => program.selected = Arc::clone(&other.selected),
            1 => program.homes = Arc::clone(&other.homes),
            2 => program.effects = Arc::clone(&other.effects),
            3 => program.machine = Arc::clone(&other.machine),
            4 => program.layout = Arc::clone(&other.layout),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                stage_optimized_function_fragment_emission(candidate),
                Err(FunctionFragmentEmissionError::RootMismatch)
            ),
            "component {component}"
        );
    }
}

#[test]
fn emission_rejects_reauthenticated_layout_without_mutating_replay_data() {
    let mut candidate = source(NativeTarget::linux_x64(), OptimizationSelections::default());
    let original = candidate.program().clone();
    let layout = Arc::make_mut(&mut candidate.program_mut().layout);
    layout.policy = omega_machine_code::SelectedFunctionLayoutPolicy::SingleEntryBlockV1;
    layout.identity = layout.recomputed_identity();
    assert_ne!(layout.identity, original.layout.identity);
    assert_eq!(
        candidate.replay().resolved_layout().program(),
        original.layout.as_ref()
    );
    assert!(matches!(
        stage_optimized_function_fragment_emission(candidate),
        Err(FunctionFragmentEmissionError::RootMismatch)
    ));
}
