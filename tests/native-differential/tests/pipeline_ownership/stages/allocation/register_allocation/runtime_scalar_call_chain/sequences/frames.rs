//! Optimization and frame evidence must describe the same calling program.

use super::{Sequence, sequence_artifact};
use crate::tests::*;

fn realization(
    target: NativeTarget,
    sequence: Sequence,
) -> StagedPostAllocationMachineFunctionRelativeRealization {
    let (semantic, proof) = sequence_artifact(sequence);
    let optimization = match target.architecture {
        target::Architecture::X86_64 => {
            Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1
        }
        target::Architecture::Aarch64 => {
            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
        }
    };
    let selections = OptimizationSelections::new([optimization]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
        .unwrap()
        .into_post_allocation_machine_for_test()
        .expect("physical materialization retains its exact replay inputs")
}

#[test]
fn optimized_calls_bind_frame_and_rewrite_evidence_together() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut staged = realization(target, Sequence::InterleavedCallees);
        let frame = staged.frame().expect("calls require a canonical frame");
        let expected_frame = FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 {
            layout: frame.layout().receipt().identity(),
            protocol: frame.protocol().receipt().identity(),
        };
        let record = staged.manifest().record().clone();
        assert_eq!(record.frame, expected_frame);
        let optimization = staged.optimization().custody().unwrap();
        assert_eq!(
            record.post_allocation_machine_optimization,
            Some(optimization)
        );
        assert!(
            optimization.selected_bytes() < optimization.baseline_bytes(),
            "{target:?}: selected {} bytes against baseline {}",
            optimization.selected_bytes(),
            optimization.baseline_bytes(),
        );
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&record.encode()).unwrap(),
            record,
        );
        assert_eq!(
            validate_post_allocation_machine_function_relative_realization_custody(&staged)
                .unwrap(),
            *staged.custody(),
        );

        // Valid outer hashes do not authorize dropping either side of the join.
        staged.manifest_mut().record_mut().frame = FunctionRelativeFrameDisposition::Unavailable;
        let changed = staged.manifest_mut().record_mut();
        changed.identity = changed.recomputed_identity();
        assert!(
            validate_post_allocation_machine_function_relative_realization_custody(&staged)
                .is_err()
        );
        *staged.manifest_mut().record_mut() = record.clone();
        staged
            .manifest_mut()
            .record_mut()
            .post_allocation_machine_optimization = None;
        let changed = staged.manifest_mut().record_mut();
        changed.identity = changed.recomputed_identity();
        assert!(
            validate_post_allocation_machine_function_relative_realization_custody(&staged)
                .is_err()
        );
        *staged.manifest_mut().record_mut() = record;

        let original = staged.frame_mut_for_test().take();
        assert!(
            validate_post_allocation_machine_function_relative_realization_custody(&staged)
                .is_err()
        );
        let mut other = realization(target, Sequence::Single);
        *staged.frame_mut_for_test() = other.frame_mut_for_test().take();
        assert!(
            validate_post_allocation_machine_function_relative_realization_custody(&staged)
                .is_err()
        );
        *staged.frame_mut_for_test() = original;
        assert_eq!(
            validate_post_allocation_machine_function_relative_realization_custody(&staged)
                .unwrap(),
            *staged.custody(),
        );
    }
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
