use omega_compiler::compile_to_checked;
use omega_optimization_pipeline::OptimizationReportRequest;

use super::support::{
    HOSTED_NATIVE_TARGETS, compile_retained_native, golden_for_target, native_canary,
    retained_native_snapshot,
};

#[test]
fn retained_native_bytes_and_metadata_match_every_target_golden() {
    for target in HOSTED_NATIVE_TARGETS {
        let checked = compile_to_checked(&native_canary().join("main.omg"), Some(target))
            .expect("the retained-artifact canary must pass source admission");
        assert!(checked.optimization_selections().is_empty(), "{target}");
        assert_eq!(
            checked.optimization_report_request(),
            OptimizationReportRequest::Suppressed,
            "{target}"
        );

        let first = compile_retained_native(target);
        let second = compile_retained_native(target);
        assert_eq!(first.semantic_bytes(), second.semantic_bytes(), "{target}");
        assert_eq!(first.proof_bytes(), second.proof_bytes(), "{target}");
        assert_eq!(
            first.object().text_bytes(),
            second.object().text_bytes(),
            "{target}"
        );
        assert_eq!(
            first.image().output().bytes,
            second.image().output().bytes,
            "{target}"
        );
        assert_eq!(
            first.image().output().final_text_bytes,
            second.image().output().final_text_bytes,
            "{target}"
        );

        let first_snapshot = retained_native_snapshot(target, &first);
        let second_snapshot = retained_native_snapshot(target, &second);
        assert_eq!(first_snapshot, second_snapshot, "{target}");
        assert_eq!(first_snapshot, golden_for_target(target), "{target}");
    }
}
