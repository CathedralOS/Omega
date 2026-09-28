use super::*;
use crate::{
    BatchChildCommitment, BatchChildOutcome, CompileOutcomes, CompileReport, CompileTargetOutcome,
};
use optimization_core::Optimization;

#[test]
fn check_only_report_cannot_claim_a_native_proof_product() {
    let error = CompileReport::check_only("check.omg".into(), 1)
        .unwrap()
        .with_pcc_context(
            PccRequests {
                psi: false,
                native: true,
            },
            proof_admission::AdmissionProfile::default(),
        )
        .unwrap_err();
    assert_eq!(
        error,
        "a check-only stop cannot satisfy an optional proof-product request"
    );
}

#[test]
fn direct_report_attachment_respects_check_and_object_product_limits() {
    for kind in [
        crate::CompileOutputKind::CheckOnly,
        crate::CompileOutputKind::ObjectContainer,
    ] {
        for requests in [
            PccRequests::default(),
            PccRequests {
                psi: true,
                native: false,
            },
            PccRequests {
                psi: false,
                native: true,
            },
            PccRequests {
                psi: true,
                native: true,
            },
        ] {
            let report = CompileReport::checked(
                "input.omg".into(),
                1,
                kind == crate::CompileOutputKind::ObjectContainer,
                kind,
                None,
            )
            .unwrap();
            assert_eq!(
                report
                    .with_pcc_context(requests, Default::default())
                    .is_ok(),
                !requests.any()
            );
        }
    }
}

fn request() -> CompileRequest {
    CompileRequest::new(CompileOptions {
        root_path: PathBuf::from("unread-source/main.omg"),
        target_name: None,
        build_dir: None,
    })
}

#[test]
fn configurations_validate_before_source_access_and_preserve_canonical_order() {
    let configurations = [
        TargetProfile::WindowsX64,
        TargetProfile::LinuxX64,
        TargetProfile::MacosArm64,
    ]
    .into_iter()
    .map(|target| {
        TargetCompileConfiguration::new(target)
            .with_build_dir(PathBuf::from("build").join(target.target_name()))
    })
    .collect();
    let (_, targets) = request()
        .with_target_configurations(configurations)
        .validate_for_execution()
        .expect("configuration needs no source")
        .into_parts();
    assert_eq!(
        targets
            .iter()
            .map(ValidatedTargetCompilation::profile)
            .collect::<Vec<_>>(),
        vec![
            Some(TargetProfile::LinuxX64),
            Some(TargetProfile::MacosArm64),
            Some(TargetProfile::WindowsX64)
        ]
    );
    assert!(
        request()
            .with_target_configurations(Vec::new())
            .validate_for_execution()
            .is_err()
    );
    let duplicates = vec![
        TargetCompileConfiguration::new(TargetProfile::LinuxX64),
        TargetCompileConfiguration::new(TargetProfile::LinuxX64),
    ];
    let errors = request()
        .with_target_configurations(duplicates)
        .validate_for_execution()
        .unwrap_err();
    assert_eq!(errors.len(), 2);
    assert!(errors[0].message.contains("duplicate compilation target"));
    assert!(errors[1].message.contains("same build directory"));
}

#[test]
fn lexical_staging_aliases_reject_across_distinct_targets() {
    let configurations = vec![
        TargetCompileConfiguration::new(TargetProfile::LinuxX64)
            .with_build_dir("build/unused/../shared".into()),
        TargetCompileConfiguration::new(TargetProfile::MacosArm64)
            .with_build_dir("build/shared".into()),
    ];
    let errors = request()
        .with_target_configurations(configurations)
        .validate_for_execution()
        .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("same build directory"));
}

#[test]
fn rollback_admission_tracks_the_requested_product_stages() {
    let native =
        OptimizationRollback::new([Optimization::X86SelectXorZeroI64MaterializationV1]).unwrap();
    let checked = OptimizationRollback::new([Optimization::CheckedTreeProductPruning]).unwrap();
    assert!(
        RequestedCompileProduct::Check
            .validate_optimization_rollback(&checked)
            .is_err()
    );
    assert!(
        RequestedCompileProduct::TerminalArtifact
            .validate_optimization_rollback(&native)
            .is_err()
    );
    RequestedCompileProduct::TerminalArtifact
        .validate_optimization_rollback(&checked)
        .unwrap();
    RequestedCompileProduct::NativeArtifact
        .validate_optimization_rollback(&native)
        .unwrap();
    assert!(
        request()
            .with_optimization_rollback(native)
            .validate_for_execution()
            .unwrap_err()[0]
            .message
            .contains("names stages not executed by Check")
    );
}

#[test]
fn evaluated_build_requirements_preserve_each_product_fence() {
    let empty = OptimizationRollback::default();
    for product in [
        RequestedCompileProduct::Check,
        RequestedCompileProduct::TerminalArtifact,
        RequestedCompileProduct::NativeArtifact,
    ] {
        for artifact_only in [false, true] {
            for psi in [false, true] {
                for native in [false, true] {
                    let requests = PccRequests { psi, native };
                    let allowed = match product {
                        RequestedCompileProduct::Check => !psi && !native,
                        _ if artifact_only => !psi && !native,
                        RequestedCompileProduct::TerminalArtifact => !native,
                        RequestedCompileProduct::NativeArtifact => true,
                    };
                    assert_eq!(
                        product
                            .admit_build_requirements(artifact_only, requests, &empty)
                            .is_ok(),
                        allowed,
                        "{product:?}, artifact_only={artifact_only}, {requests:?}"
                    );
                }
            }
        }
    }
    let rollback = OptimizationRollback::new([Optimization::ControlFlowCleanup]).unwrap();
    let errors = RequestedCompileProduct::NativeArtifact
        .admit_build_requirements(
            true,
            PccRequests {
                psi: true,
                native: true,
            },
            &rollback,
        )
        .unwrap_err();
    assert_eq!(
        errors[0].message,
        "artifact-only builds execute no product optimization stages to roll back"
    );
}

#[test]
fn batch_outcomes_retain_failure_identity_and_do_not_hide_a_sibling() {
    let outcomes = CompileOutcomes::new(vec![
        CompileTargetOutcome::new(
            Some(TargetProfile::LinuxX64),
            Ok(CompileReport::check_only("first.omg".into(), 1).unwrap()),
        ),
        CompileTargetOutcome::new(
            Some(TargetProfile::MacosArm64),
            Err(vec![Diagnostic::error("second target rejected")]),
        ),
    ])
    .unwrap();
    assert!(outcomes.batch_manifest().validate());
    assert_eq!(outcomes.outcomes().len(), 2);
    assert!(outcomes.outcomes()[0].succeeded());
    assert_eq!(
        outcomes.outcomes()[1].diagnostics().unwrap()[0].message,
        "second target rejected"
    );
    assert!(matches!(
        outcomes.batch_manifest().rows()[0].outcome(),
        BatchChildOutcome::Succeeded {
            commitment: BatchChildCommitment::Checked
        }
    ));
    assert!(matches!(
        outcomes.batch_manifest().rows()[1].outcome(),
        BatchChildOutcome::Rejected { .. }
    ));
    assert!(outcomes.into_single_report().is_err());
    assert!(CompileOutcomes::new(Vec::new()).is_err());
    assert!(CompileOutcomes::new(vec![CompileTargetOutcome::new(None, Err(Vec::new()))]).is_err());
}
