use super::{
    PROJECT_SEQUENCE, bundled_standard_library_root, compile_source_native_evidence,
    diagnostic_messages, exact_optimization_vocabulary_build, fixture_package_identity, project,
    publish_source_native_evidence, replay_native_artifact_parts, validate_source_native_evidence,
};
use crate::macos_entry_acceptance;
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, OptimizationRollback,
    RequestedCompileProduct, compile_to_checked,
};
use optimization_core::{Optimization, OptimizationReportRequest};
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use std::sync::atomic::Ordering;

#[test]
fn absent_and_role_only_builds_select_no_optimizations() {
    let absent = project("absent", None);
    let checked = compile_to_checked(CheckedCompileRequest::new(&absent.join("main.omg"), None))
        .expect("absent build remains valid");
    assert!(checked.optimization_selections().is_empty());
    assert_eq!(
        checked.optimization_report_request(),
        OptimizationReportRequest::Suppressed
    );

    let role_only = project(
        "role-only",
        Some(
            "machine build(builder: &mut Build) {\n    builder.application(\"optimizer-role-only\");\n}\n",
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &role_only.join("main.omg"),
        None,
    ))
    .expect("role-only canonical build remains valid");
    assert!(checked.optimization_selections().is_empty());
    assert_eq!(
        checked.optimization_report_request(),
        OptimizationReportRequest::Suppressed
    );
}

#[test]
fn human_report_is_an_explicit_request_not_an_optimization_selection() {
    let root = project(
        "human-report",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-human-report");
    builder.optimizations.emit_report();
}
"#,
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
        .expect("an explicit report-only request should evaluate");
    assert!(checked.optimization_selections().is_empty());
    assert_eq!(
        checked.optimization_report_request(),
        OptimizationReportRequest::EmitHumanText
    );
}

#[test]
fn duplicate_human_report_requests_reject_during_build_evaluation() {
    let root = project(
        "duplicate-human-report",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-duplicate-human-report");
    builder.optimizations.emit_report();
    builder.optimizations.emit_report();
}
"#,
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
        .expect_err("duplicate human report requests must reject");
    assert!(
        diagnostic_messages(&diagnostics)
            .contains("optimization human report is requested more than once")
    );
}

#[test]
fn every_exact_enable_and_rollback_maps_to_itself_through_the_build_prelude() {
    for optimization in Optimization::ALL {
        let build = exact_optimization_vocabulary_build(optimization);
        let label = format!("selected-{}", optimization.build_counter_field());
        let root = project(&label, Some(&build));
        let checked = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
            .expect("the exact named selection should evaluate");
        assert_eq!(
            checked.optimization_selections().as_slice(),
            &[optimization]
        );
        assert_eq!(
            checked.optimization_report_request(),
            OptimizationReportRequest::EmitHumanText
        );
        assert_eq!(
            checked.optimization_selection_identity(),
            checked.optimization_selections().identity()
        );

        let rollback = OptimizationRollback::from_exact_names([optimization.build_case_name()])
            .expect("the exact build name is also the exact rollback name");
        let receipt = rollback
            .reconcile(checked.optimization_selections())
            .expect("one exact rollback request leaves custody");
        assert_eq!(receipt.build_selected().as_slice(), &[optimization]);
        assert_eq!(receipt.requested_disabled().as_slice(), &[optimization]);
        assert_eq!(receipt.actually_disabled().as_slice(), &[optimization]);
        assert!(receipt.effective().is_empty());
    }
}

#[test]
fn duplicate_enable_calls_reject_during_build_evaluation() {
    let root = project(
        "duplicate",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-duplicate");
    builder.optimizations.enable(Optimization::GlobalValueNumbering);
    builder.optimizations.enable(Optimization::GlobalValueNumbering);
}
"#,
        ),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
        .expect_err("duplicate optimization selections must reject");
    assert!(
        diagnostic_messages(&diagnostics)
            .contains("optimization `GlobalValueNumbering` is enabled more than once")
    );
}

#[test]
fn ordinary_authored_build_does_not_replace_selected_toolchain_build() {
    let root = project(
        "ordinary-build",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-ordinary-build");
}
"#,
        ),
    );
    std::fs::write(
        root.join("main.omg"),
        r#"data Build {
    freestanding: bool;
}
"#,
    )
    .expect("write ordinary legacy Build declaration");
    let checked = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
        .expect("ordinary authored Build must remain outside build-root vocabulary");
    assert!(checked.optimization_selections().is_empty());
}

#[test]
fn ordinary_authored_lookalike_selection_field_cannot_spoof_toolchain_build() {
    let root = project(
        "lookalike",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-lookalike");
}
"#,
        ),
    );
    std::fs::write(
        root.join("main.omg"),
        r#"data ProgramOptimizations {
}
data Build {
    freestanding: bool;
    optimizations: ProgramOptimizations;
}
"#,
    )
    .expect("write ordinary lookalike Build declaration");
    let checked = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
        .expect("ordinary lookalike vocabulary must not replace the toolchain Build");
    assert!(checked.optimization_selections().is_empty());
}

#[test]
fn return_only_identity_build_preserves_complete_native_evidence() {
    let standard_library = bundled_standard_library_root();
    let macos_entry = macos_entry_acceptance::candidate_macos_entry_binding(
        &standard_library,
        fixture_package_identity(2),
    )
    .expect("check and explicitly accept the real target entry contract");
    for target in [
        "windows_x86_64",
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
    ] {
        let report = compile_source_native_evidence(
            target,
            false,
            "data Main {} machine Main::main() {}\n",
            &macos_entry,
        );
        validate_source_native_evidence(&report);
        let physical = report
            .retained_native_artifact()
            .unwrap()
            .physical_evidence()
            .expect("complete empty physical evidence");
        assert!(physical.projection().operator_occurrences().is_empty());
        assert!(physical.projection().boundary_occurrences().is_empty());
        assert!(physical.children().is_empty());
        publish_source_native_evidence(report, target);
    }
}

#[test]
fn scalar_returning_source_calls_preserve_native_evidence_on_every_target() {
    let standard_library = bundled_standard_library_root();
    let macos_entry = macos_entry_acceptance::candidate_macos_entry_binding(
        &standard_library,
        fixture_package_identity(2),
    )
    .expect("check and explicitly accept the real target entry contract");
    for target in [
        "windows_x86_64",
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
    ] {
        for copy_propagation in [false, true] {
            let report = compile_source_native_evidence(
                target,
                copy_propagation,
                r#"
machine pick(left: u64, right: u64) -> u64
requires true
ensures result == right
{ transition { _ -> right } }
data Main {}
machine Main::main() {
    let first: u64 = pick(7u64, 9u64);
    let second: u64 = pick(first, first);
    let third: u64 = pick(first, second);
}
"#,
                &macos_entry,
            );
            validate_source_native_evidence(&report);
            let artifact = report.retained_native_artifact().unwrap();
            assert!(
                image_emission::derive_stack_demand(artifact.object(), artifact.object().entry())
                    .unwrap()
                    .ceiling_bytes()
                    > 0
            );
            publish_source_native_evidence(report, target);
        }
    }
}

#[test]
fn return_only_selected_lowering_build_rejoins_native_artifact_production() {
    let root = project(
        "fail-closed",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-fail-closed");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactAddImmediate);
}
"#,
        ),
    );
    let build_dir = root.join("build");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the exact return-only selected-lowering cohort should reach native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("selected-lowering compilation retains its native artifact");
    artifact
        .validate()
        .expect("selected-lowering native artifact should replay");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("empty D32 coverage is still exact physical evidence");
    assert!(physical.projection().operator_occurrences().is_empty());
    assert!(physical.projection().boundary_occurrences().is_empty());
    assert!(physical.children().is_empty());
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn partial_rollback_routes_the_remaining_psi_selection_to_preterminal() {
    let root = project(
        "partial-rollback-fail-closed",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-partial-rollback-fail-closed");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::ControlFlowCleanup);
    builder.optimizations.enable(Optimization::CopyPropagation);
}
"#,
        ),
    );
    let build_dir = root.join("build");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_optimization_rollback(
            OptimizationRollback::new([Optimization::CopyPropagation])
                .expect("the partial rollback request must be unique"),
        ),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the remaining Psi selection executes at its pre-Terminal owner");
    let receipt = report
        .optimization_rollback_receipt()
        .expect("a partial rollback reports the receipt of what actually ran");
    assert_eq!(
        receipt.actually_disabled().as_slice(),
        &[Optimization::CopyPropagation]
    );
    assert_eq!(
        receipt.effective().as_slice(),
        &[Optimization::ControlFlowCleanup]
    );
    let artifact = report
        .retained_native_artifact()
        .expect("the surviving selection still reaches native custody");
    let executed = artifact.psi_artifact().optimization().selections();
    assert_eq!(executed.as_slice().len(), 1);
    assert!(
        executed.contains(
            Optimization::ControlFlowCleanup
                .optimization()
                .expect("ControlFlowCleanup is a Psi selection")
        )
    );
    assert!(
        !executed.contains(
            Optimization::CopyPropagation
                .optimization()
                .expect("CopyPropagation is a Psi selection")
        )
    );
    artifact
        .validate()
        .expect("the retained native artifact replays");
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn return_only_exact_subtract_rejoins_native_artifact_production() {
    let root = project(
        "subtract-fail-closed",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-subtract-fail-closed");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactSubtractImmediate);
}
"#,
        ),
    );
    let build_dir = root.join("build");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the exact return-only subtract selection should reach native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("selected-lowering compilation retains its native artifact");
    artifact
        .validate()
        .expect("selected-lowering native artifact should replay");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    assert!(
        artifact
            .physical_evidence()
            .expect("empty D32 coverage remains exact")
            .children()
            .is_empty()
    );
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn return_only_compare_immediate_rejoins_native_artifact_production() {
    let root = project(
        "compare-rejoin",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer-compare-rejoin");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12CompareImmediate);
}
"#,
        ),
    );
    let build_dir = root.join("build");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the exact return-only compare selection should reach native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("selected-lowering compilation retains its native artifact");
    artifact
        .validate()
        .expect("selected-lowering native artifact should replay");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    assert!(
        artifact
            .physical_evidence()
            .expect("empty D32 coverage remains exact")
            .children()
            .is_empty()
    );
    assert!(!build_dir.join("omega-program").exists());
    assert!(!build_dir.join("omega-program.exe").exists());
}

#[test]
fn selected_lowering_boundary_occurrence_replays_one_exact_physical_child() {
    let root = std::env::temp_dir().join(format!(
        "omega-optimizer-opt-in-selected-lowering-physical-child-{}-{}",
        std::process::id(),
        PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create selected-lowering physical-child project");
    std::fs::write(
        root.join("main.omg"),
        r#"data CheckedMath {}

boundary operator CheckedMath::select_left(left: u64, right: u64) -> u64;

data CheckedMathProvider {}

machine CheckedMathProvider::select_left_impl(left: u64, right: u64) -> u64
satisfies CheckedMath::select_left
{
    transition { _ -> left }
}

data Main {}

machine Main::main(&mut self) {
    let result: u64 = CheckedMath::select_left(7u64, 9u64);
}
"#,
    )
    .expect("write selected-lowering physical-child main");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("optimizer-selected-lowering-physical-child");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12CompareImmediate);
}
"#,
    )
    .expect("write selected-lowering physical-child build");
    let root_identity = fixture_package_identity(41);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("physical-child package graph should validate");
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("one selected-lowering operation must carry a boundary occurrence to native custody");
    let artifact = report
        .retained_native_artifact()
        .expect("selected-lowering compilation retains its native artifact");
    artifact
        .validate()
        .expect("selected-lowering native artifact should replay independently");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("the surviving boundary occurrence retains nonempty physical evidence");
    let [occurrence] = physical.projection().operator_occurrences() else {
        panic!("the checked boundary operator must survive as exactly one operator occurrence")
    };
    assert!(physical.projection().boundary_occurrences().is_empty());
    let [child] = physical.children() else {
        panic!("the surviving occurrence must bind exactly one physical child")
    };
    assert_eq!(
        child.occurrence(),
        native_realization::NativePhysicalOccurrence::Operator(occurrence.identity())
    );
    assert_eq!(child.projection(), physical.projection().identity());
    assert!(matches!(
        child.parent(),
        native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
    ));
    assert!(child.machine_span().byte_count() > 0);
    assert!(child.object_span().byte_count() > 0);
    assert_eq!(
        child.relocation(),
        native_realization::PhysicalRelocationDisposition::ResolvedInternalCall
    );

    let parts = report
        .into_retained_native_artifact()
        .expect("owned native artifact")
        .into_parts();
    let mut missing = replay_native_artifact_parts(&parts);
    let evidence = missing
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    missing.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: evidence.projection,
                children: Vec::new(),
                identity: evidence.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(missing).is_err(),
        "a missing physical child must not replay"
    );

    let mut duplicate = replay_native_artifact_parts(&parts);
    let evidence = duplicate
        .physical_evidence
        .take()
        .expect("replay physical evidence")
        .into_parts();
    let [only_child] = evidence.children.as_slice() else {
        panic!("one physical child before duplication")
    };
    duplicate.physical_evidence = Some(
        native_realization::NativePhysicalEvidence::from_replayed_parts(
            native_realization::NativePhysicalEvidenceParts {
                projection: evidence.projection,
                children: vec![only_child.clone(), only_child.clone()],
                identity: evidence.identity,
            },
        ),
    );
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(duplicate).is_err(),
        "a duplicate physical child must not replay"
    );

    let assert_mutated_child_rejected =
        |mutate: &dyn Fn(&mut native_realization::NativePhysicalChildParts)| {
            let mut replay = replay_native_artifact_parts(&parts);
            let evidence = replay
                .physical_evidence
                .take()
                .expect("replay physical evidence")
                .into_parts();
            let [child] = evidence.children.as_slice() else {
                panic!("one physical child before mutation")
            };
            let mut child = child.clone().into_parts();
            mutate(&mut child);
            replay.physical_evidence = Some(
                native_realization::NativePhysicalEvidence::from_replayed_parts(
                    native_realization::NativePhysicalEvidenceParts {
                        projection: evidence.projection,
                        children: vec![
                            native_realization::NativePhysicalChild::from_replayed_parts(child),
                        ],
                        identity: evidence.identity,
                    },
                ),
            );
            assert!(
                native_realization::NativeArtifact::from_replayed_parts(replay).is_err(),
                "a mutated physical child must not replay"
            );
        };
    assert_mutated_child_rejected(&|child| {
        assert!(matches!(
            child.parent,
            native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
        ));
        child.occurrence = native_realization::NativePhysicalOccurrence::Boundary(
            optimization_core::OptimizedBoundaryOccurrenceIdentity::from_bytes(
                child.occurrence.identity(),
            ),
        );
    });
    assert_mutated_child_rejected(&|child| {
        child.machine_span = native_realization::NativeByteSpan::from_replayed_parts(
            child.machine_span.offset(),
            child.machine_span.byte_count() + 1,
        );
    });
    assert_mutated_child_rejected(&|child| {
        child.projection =
            optimization_core::NativeOptimizationProjectionIdentity::from_bytes([0x5A; 32]);
    });
}
