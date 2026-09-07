//! Optimization and frame evidence must describe the same calling program.

use super::{Sequence, sequence_artifact};
use crate::tests::*;

fn realization(
    target: NativeTarget,
    sequence: Sequence,
) -> StagedFixedFrameFunctionRelativeRealization {
    let (semantic, proof) = sequence_artifact(sequence);
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
        .unwrap()
        .into_fixed_frame_for_test()
}

#[test]
fn optimized_call_publication_rejects_detached_frame_application() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = realization(target, Sequence::InterleavedCallees);
        let emitted = stage_optimized_function_fragment_emission(staged.into()).unwrap();
        let expected_frames = super::super::publication::planned_frame_count(&emitted);
        let framed = stage_function_fragment_frame_application(emitted).unwrap();
        assert_eq!(framed.receipt().framed_function_count(), expected_frames);
        let mut text = stage_optimized_fixed_frame_text_section(framed).unwrap();
        assert_eq!(text.text_section().resolved_internal_machine_calls.len(), 4);
        text.corrupt_custody_frame_application_for_test();
        assert!(stage_optimized_relocation_free_object_container(text).is_err());
    }
}
