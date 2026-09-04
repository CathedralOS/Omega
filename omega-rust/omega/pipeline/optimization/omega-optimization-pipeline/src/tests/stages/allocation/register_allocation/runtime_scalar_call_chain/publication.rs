use crate::tests::*;

use super::fixture::staged_homes;

fn staged_fixed_frame_text(
    target: NativeTarget,
) -> (
    StagedOptimizedFixedFrameTextSection,
    FunctionFragmentFrameApplicationIdentity,
) {
    let budget =
        OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000).unwrap();
    let realization =
        stage_fixed_frame_function_relative_realization(staged_homes(target), budget).unwrap();
    let frame = realization.frame().clone();
    let protocol = realization.protocol().clone();
    let fragments = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::FixedFrame(Box::new(realization)),
    )
    .unwrap();
    let applied = stage_function_fragment_frame_application(fragments, frame, protocol).unwrap();
    assert_eq!(applied.receipt().framed_function_count(), 1);
    let application = applied.receipt().identity();
    (
        stage_optimized_fixed_frame_text_section(applied).unwrap(),
        application,
    )
}

#[test]
fn nonzero_fixed_frames_reach_object_artifacts_on_both_isas() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = scalar_call_unit_artifact();
        let (text, application) = staged_fixed_frame_text(target);
        let text_manifest = text.manifest().record().identity;
        assert_eq!(text.text_section().resolved_internal_machine_calls.len(), 3);
        assert_eq!(
            text.manifest().record().source_custody,
            FunctionFragmentTextSectionSourceCustody::FixedFrameApplicationV1 { application }
        );

        let object = stage_optimized_relocation_free_object_container(text).unwrap();
        assert_eq!(
            validate_optimized_relocation_free_object_container(&object).unwrap(),
            object.custody()
        );
        assert_eq!(object.object().relocation_record_count, 0);
        assert_eq!(
            object.manifest().record().source_text_section_manifest,
            text_manifest
        );
        let StagedOptimizedObjectTextSectionSource::FixedFrame(fixed) = object.source() else {
            panic!("fixed-frame text custody must survive object construction")
        };
        assert_eq!(
            fixed.manifest().record().source_custody,
            FunctionFragmentTextSectionSourceCustody::FixedFrameApplicationV1 { application }
        );
        assert_eq!(
            fixed.source().source().manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::CanonicalFixedFrameBodyV1
        );

        let artifact = stage_validated_optimized_object_artifact(
            canonical_artifact(&semantic, &proof),
            object,
        )
        .unwrap();
        assert_eq!(
            validate_optimized_object_artifact(&artifact).unwrap(),
            artifact.custody()
        );
        assert_eq!(artifact.artifact().text_section_manifest, text_manifest);
        let StagedOptimizedObjectTextSectionSource::FixedFrame(fixed) = artifact.source().source()
        else {
            panic!("fixed-frame text custody must survive object artifact publication")
        };
        assert_eq!(
            fixed.manifest().record().source_custody,
            FunctionFragmentTextSectionSourceCustody::FixedFrameApplicationV1 { application }
        );
    }
}

#[test]
fn object_entrance_rejects_corrupt_fixed_frame_custody() {
    let (mut text, _) = staged_fixed_frame_text(NativeTarget::linux_x64());
    text.corrupt_custody_frame_application_for_test();
    assert!(matches!(
        stage_optimized_relocation_free_object_container(text),
        Err(RelocationFreeObjectContainerError::Source(
            RelocationFreeTextSectionPlacementError::ReceiptMismatch
        ))
    ));
}
