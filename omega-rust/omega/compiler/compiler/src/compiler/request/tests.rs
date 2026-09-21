use super::{
    Arc, CompileOptions, CompileOutcomes, CompileRequest, CompileTargetOutcome, Diagnostic,
    OptimizationRollback, RequestedCompileProduct, TargetCompileConfiguration, TargetProfile,
};
use optimization_core::Optimization;

fn request(product: RequestedCompileProduct) -> CompileRequest {
    CompileRequest::new(CompileOptions {
        root_path: "missing.omg".into(),
        build_dir: None,
        target_name: None,
    })
    .with_requested_product(product)
}

#[test]
fn neutral_products_stay_neutral_and_native_resolves_host() {
    for product in [
        RequestedCompileProduct::Check,
        RequestedCompileProduct::TerminalArtifact,
    ] {
        let admitted = request(product).validate_for_execution().unwrap();
        assert_eq!(admitted.targets.len(), 1);
        assert_eq!(admitted.targets[0].profile, None);
    }
    match TargetProfile::host_if_supported() {
        Some(host) => {
            let admitted = request(RequestedCompileProduct::NativeArtifact)
                .validate_for_execution()
                .unwrap();
            assert_eq!(admitted.targets.len(), 1);
            assert_eq!(admitted.targets[0].profile, Some(host));
            assert_eq!(
                admitted.targets[0].options.target_name.as_deref(),
                Some(host.target_name())
            );
        }
        None => {
            // A host with no catalogued profile cannot supply native
            // production's implicit target; the request must report that
            // absence as a diagnostic rather than panic inside host().
            let diagnostics = request(RequestedCompileProduct::NativeArtifact)
                .validate_for_execution()
                .expect_err("an unprofiled host cannot resolve a native target");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("no catalogued Omega deployment profile")),
                "{diagnostics:?}"
            );
        }
    }
}

#[test]
fn rollback_is_checked_before_source_acquisition_for_every_configuration() {
    let rollback = OptimizationRollback::new([Optimization::ControlFlowCleanup]).unwrap();
    assert!(
        request(RequestedCompileProduct::Check)
            .with_optimization_rollback(rollback.clone())
            .validate_for_execution()
            .is_err()
    );
    assert!(
        request(RequestedCompileProduct::TerminalArtifact)
            .with_optimization_rollback(rollback.clone())
            .validate_for_execution()
            .is_ok()
    );
    // Native production names an exact target here: the targetless form
    // resolves the compiler host's catalogued profile, which hosts without
    // one (macOS x86-64) cannot supply; that leg is covered by
    // `neutral_products_stay_neutral_and_native_resolves_host`.
    assert!(
        CompileRequest::new(CompileOptions {
            root_path: "missing.omg".into(),
            build_dir: None,
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_optimization_rollback(rollback)
        .validate_for_execution()
        .is_ok()
    );
    let native_only =
        OptimizationRollback::new([Optimization::SelectedIncomingU12ExactAddImmediate]).unwrap();
    assert!(
        request(RequestedCompileProduct::TerminalArtifact)
            .with_optimization_rollback(native_only)
            .validate_for_execution()
            .is_err()
    );
}

#[test]
fn empty_duplicate_and_colliding_targets_reject_before_acquisition() {
    assert!(
        request(RequestedCompileProduct::Check)
            .with_target_configurations(Vec::new())
            .validate_for_execution()
            .is_err()
    );
    let target =
        TargetCompileConfiguration::new(TargetProfile::LinuxX64).with_build_dir("linux".into());
    let duplicate = request(RequestedCompileProduct::Check)
        .with_target_configurations(vec![target.clone(), target.with_build_dir("other".into())])
        .validate_for_execution()
        .unwrap_err();
    assert!(
        duplicate
            .iter()
            .any(|diagnostic| diagnostic.message.contains("duplicate compilation target"))
    );
    let collision = request(RequestedCompileProduct::Check)
        .with_target_configurations(vec![
            TargetCompileConfiguration::new(TargetProfile::LinuxX64),
            TargetCompileConfiguration::new(TargetProfile::WindowsX64),
        ])
        .validate_for_execution()
        .unwrap_err();
    assert!(
        collision
            .iter()
            .any(|diagnostic| diagnostic.message.contains("same build directory"))
    );
}

#[test]
fn aliased_build_directory_spellings_reject_before_acquisition() {
    // `./out`, `sub/../out` and the absolute cwd spelling all stage into the
    // same directory as `out`: the byte spellings differ, but two targets
    // would race writes into one staging root, so the physical identity
    // collides.
    for spelling in [
        "./shared".into(),
        "subdir/../shared".into(),
        std::env::current_dir().unwrap().join("shared"),
    ] {
        let diagnostics = request(RequestedCompileProduct::Check)
            .with_target_configurations(vec![
                TargetCompileConfiguration::new(TargetProfile::LinuxX64)
                    .with_build_dir("shared".into()),
                TargetCompileConfiguration::new(TargetProfile::WindowsX64).with_build_dir(spelling),
            ])
            .validate_for_execution()
            .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("same build directory")),
            "{diagnostics:?}"
        );
    }
    // Genuinely separate staging directories still admit.
    assert!(
        request(RequestedCompileProduct::Check)
            .with_target_configurations(vec![
                TargetCompileConfiguration::new(TargetProfile::LinuxX64)
                    .with_build_dir("shared".into()),
                TargetCompileConfiguration::new(TargetProfile::WindowsX64)
                    .with_build_dir("subdir/../other".into()),
            ])
            .validate_for_execution()
            .is_ok()
    );
}

#[test]
fn targets_are_canonical_and_share_one_input_owner() {
    let admitted = request(RequestedCompileProduct::Check)
        .with_target_configurations(vec![
            TargetCompileConfiguration::new(TargetProfile::WindowsX64)
                .with_build_dir("windows".into()),
            TargetCompileConfiguration::new(TargetProfile::LinuxX64).with_build_dir("linux".into()),
        ])
        .validate_for_execution()
        .unwrap();
    assert_eq!(admitted.targets[0].profile, Some(TargetProfile::LinuxX64));
    assert_eq!(admitted.targets[1].profile, Some(TargetProfile::WindowsX64));
    assert_eq!(Arc::strong_count(&admitted.shared), 1);
    assert!(
        admitted
            .targets
            .iter()
            .all(|target| target.options.root_path == admitted.shared.root_path)
    );
}

#[test]
fn target_aliases_normalize_without_native_inference_for_neutral_checks() {
    let admitted = CompileRequest::new(CompileOptions {
        root_path: "missing.omg".into(),
        build_dir: None,
        target_name: Some("windows_x64".into()),
    })
    .validate_for_execution()
    .unwrap();
    assert_eq!(
        admitted.targets[0].options.target_name.as_deref(),
        Some("windows_x86_64")
    );
    assert!(
        CompileRequest::new(CompileOptions {
            root_path: "missing.omg".into(),
            build_dir: None,
            target_name: Some("unknown".into())
        })
        .validate_for_execution()
        .is_err()
    );
}

#[test]
fn target_authority_is_not_union_merged() {
    let rollback = OptimizationRollback::new([Optimization::ControlFlowCleanup]).unwrap();
    let admitted = request(RequestedCompileProduct::TerminalArtifact)
        .with_target_configurations(vec![
            TargetCompileConfiguration::new(TargetProfile::LinuxX64)
                .with_build_dir("linux".into())
                .with_optimization_rollback(rollback.clone()),
            TargetCompileConfiguration::new(TargetProfile::WindowsX64)
                .with_build_dir("windows".into()),
        ])
        .validate_for_execution()
        .unwrap();
    assert_eq!(
        admitted.targets[0].configuration.optimization_rollback,
        rollback
    );
    assert_eq!(
        admitted.targets[1].configuration.optimization_rollback,
        OptimizationRollback::default()
    );
}

#[test]
fn single_extraction_never_hides_sibling_failure() {
    let outcomes = CompileOutcomes::new(vec![
        CompileTargetOutcome::new(
            Some(TargetProfile::LinuxX64),
            Err(vec![Diagnostic::error("first")]),
        ),
        CompileTargetOutcome::new(
            Some(TargetProfile::WindowsX64),
            Err(vec![Diagnostic::error("second")]),
        ),
    ])
    .expect("rejected children still produce a batch collection");
    assert!(
        outcomes.into_single_report().unwrap_err()[0]
            .message
            .contains("received 2")
    );
    let outcomes = CompileOutcomes::new(vec![CompileTargetOutcome::new(
        None,
        Err(vec![Diagnostic::error("source failed")]),
    )])
    .expect("a rejected child still produces a batch collection");
    assert_eq!(
        outcomes.into_single_report().unwrap_err()[0].message,
        "source failed"
    );
}
