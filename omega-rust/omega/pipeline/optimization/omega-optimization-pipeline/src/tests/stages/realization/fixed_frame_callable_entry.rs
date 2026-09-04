use crate::tests::*;

fn staged_fixed_frame_callable(
    target: NativeTarget,
) -> (
    StagedValidatedOptimizedOrdinaryCallableEntry,
    FunctionFragmentFrameApplicationIdentity,
) {
    let (semantic, proof) = conditional_u64_not_equal_zero_parameter_artifact();
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    let selected = stage_optimized_instruction_selection(target).unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    let budget =
        OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000).unwrap();
    let realization = stage_fixed_frame_function_relative_realization(homes, budget).unwrap();
    let fragments = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::FixedFrame(Box::new(realization)),
    )
    .unwrap();
    let applied = stage_function_fragment_frame_application(fragments).unwrap();
    let application = applied.receipt().identity();
    let text = stage_optimized_fixed_frame_text_section(applied).unwrap();
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    let artifact =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .unwrap();
    (
        stage_validated_optimized_ordinary_callable_entry(artifact).unwrap(),
        application,
    )
}

#[test]
fn fixed_frame_source_reaches_ordinary_callable_on_both_isas() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (callable, application) = staged_fixed_frame_callable(target);
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&callable).unwrap(),
            callable.custody()
        );
        let StagedOptimizedObjectTextSectionSource::FixedFrame(fixed) =
            callable.source().source().source()
        else {
            panic!("ordinary callable must retain fixed-frame text custody")
        };
        assert_eq!(
            fixed.manifest().record().source_custody,
            FunctionFragmentTextSectionSourceCustody::FixedFrameApplicationV1 { application }
        );
        assert_eq!(
            fixed.source().source().manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::CanonicalFixedFrameBodyV1
        );
        let report = optimization_pipeline_report_from_ordinary_callable_entry(&callable);
        assert_eq!(
            report.function_fragment().unwrap().source_kind,
            FunctionFragmentEmissionSourceKind::CanonicalFixedFrameBodyV1
        );
        assert_eq!(
            report.ordinary_callable_entry().unwrap().entry,
            callable.entry().identity
        );
    }
}
