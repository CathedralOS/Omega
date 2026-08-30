use omega_compiler::compile_to_checked;
use omega_optimization_pipeline::OptimizationReportRequest;

use super::support::{
    HOSTED_NATIVE_TARGETS, compile_native_with_checked_receipt, compile_retained_native,
    golden_for_target, native_canary, retained_native_snapshot,
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

#[test]
fn checked_receipt_retains_exact_native_join_for_every_target() {
    for target in HOSTED_NATIVE_TARGETS {
        let compilation = compile_native_with_checked_receipt(target);
        assert_eq!(compilation.target_profile().target_name(), target);
        assert_eq!(
            compilation.checked().source_file_count(),
            compilation.report().source_file_count
        );
        assert_eq!(
            compilation.native_target(),
            compilation
                .report()
                .retained_native_artifact()
                .expect("the paired native report must retain its artifact")
                .target()
        );
        let artifact = compilation
            .into_report()
            .into_retained_native_artifact()
            .expect("the paired native report must transfer its retained artifact");
        artifact
            .validate()
            .expect("the retained checked-receipt artifact must replay");
    }
}
