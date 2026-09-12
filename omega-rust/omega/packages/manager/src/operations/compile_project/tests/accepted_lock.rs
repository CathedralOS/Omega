use super::*;
use crate::lock::PackageLock;
use crate::operations::{prepare_local_project_for_target, review_package_change};
use crate::review::{
    FreshPackageRootPolicyError, PackagePolicyDecision, PackagePolicyDecisionSubject,
    resolve_package_policy_decisions,
};

fn accept_project(project: &TemporaryProject, target: target::TargetProfile) -> PackageLock {
    let prepared = prepare_local_project_for_target(&project.entry(), target)
        .expect("prepare initial review")
        .expect("application project");
    let (_, closure, _) = prepared.into_review_parts();
    let review = review_package_change(closure, target, None, &project.workspace.join("review"))
        .expect("check the ordinary install/update candidate");
    let choices = review
        .changes()
        .packages()
        .iter()
        .flat_map(|package| package.rows())
        .filter(|row| row.requires_decision())
        .map(|row| PackagePolicyDecision {
            subject: PackagePolicyDecisionSubject::Row(row.fingerprint().digest()),
            disposition: ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        })
        .collect::<Vec<_>>();
    let resolution = resolve_package_policy_decisions(
        review.changes(),
        review.changes().fingerprint().digest(),
        &choices,
    )
    .expect("explicitly accept this test application's declared assumptions");
    let lock = PackageLock::from_targets(vec![
        review
            .propose_lock_target(&resolution)
            .expect("accepted target"),
    ])
    .expect("one accepted project target");
    std::fs::write(
        project.source.join("omega.lock"),
        lock.canonical_text().expect("readable accepted policy"),
    )
    .expect("record the test project's acceptance");
    lock
}

#[test]
fn accepted_project_policy_needs_no_second_native_approval() {
    let project = TemporaryProject::new();
    let target = target::TargetProfile::LinuxX64;
    let accepted = accept_project(&project, target);
    let lock_before = std::fs::read(project.source.join("omega.lock")).unwrap();
    let prepared = prepare_local_project_for_target(&project.entry(), target)
        .expect("prepare accepted project")
        .expect("application project");
    let (_, closure, _) = prepared.into_review_parts();
    let unchanged = review_package_change(
        closure,
        target,
        accepted.target(target),
        &project.workspace.join("unchanged-review"),
    )
    .expect("recheck against ordinary accepted policy");
    assert!(!unchanged.changes().requires_decision());
    let prepared = prepare_local_project_for_target(&project.entry(), target)
        .expect("prepare native candidate")
        .expect("application project");
    let report = compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(
            prepared,
            project.workspace.join("accepted-native"),
            target,
        )
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .expect("accepted package policy must not need a second native approval file");
    assert_eq!(
        report.output_kind(),
        compiler::CompileOutputKind::RetainedNativeArtifact
    );
    report
        .retained_native_artifact()
        .unwrap()
        .validate()
        .unwrap();
    assert_eq!(
        std::fs::read(project.source.join("omega.lock")).unwrap(),
        lock_before
    );
}

#[test]
fn native_comparison_observes_generated_candidate_without_reopening_authored_source() {
    let project = TemporaryProject::new();
    let producer = project.source.join("producer");
    std::fs::create_dir_all(producer.join("inputs")).unwrap();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|path| path.join("tests/fixtures/packages").is_dir())
        .unwrap()
        .join("tests/fixtures/packages/generated-table");
    for name in ["build.omg", "main.omg", "inputs/table.txt"] {
        std::fs::copy(fixtures.join(name), producer.join(name)).unwrap();
    }
    std::fs::write(
        project.source.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.application("observed-candidate");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.depend(Source::Path { location: "producer" });
}
"#,
    )
    .unwrap();
    std::fs::write(project.entry(), "use generated_table::main;\ndata Main {}\nmachine observed_value() -> u64 { table_size() }\nmachine Main::main(&mut self) { let value: u64 = observed_value(); }\n").unwrap();
    let target = target::TargetProfile::LinuxX64;
    accept_project(&project, target);
    let prepared = prepare_local_project_for_target(&project.entry(), target)
        .unwrap()
        .unwrap();
    let mut observations = 0;
    let (report, ()) = compile_prepared_local_project_for_native_with_observation(
        PreparedLocalProjectNativeRequest::new(
            prepared,
            project.workspace.join("observed"),
            target,
        )
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
        |checked| {
            observations += 1;
            let generated = checked_interpreter::interpret_entry(checked, "observed_value", &[]);
            assert!(generated.error.is_none(), "{:?}", generated.error);
            assert_eq!(generated.exit_code, 3);
            let entry = checked.selected_program_entry_machine().unwrap();
            let outcome = checked_interpreter::interpret_entry(checked, entry, &[]);
            assert!(outcome.error.is_none(), "{:?}", outcome.error);
            assert_eq!(outcome.exit_code, 0);
            // Realization consumes the checked snapshot, not another live build.
            std::fs::write(
                project.source.join("build.omg"),
                "invalid after observation",
            )
            .unwrap();
        },
    )
    .unwrap();
    assert_eq!(observations, 1);
    report
        .retained_native_artifact()
        .unwrap()
        .validate()
        .unwrap();
}

fn native_project(
    project: &TemporaryProject,
) -> Result<CompileReport, CompilePreparedLocalProjectNativeError> {
    let target = target::TargetProfile::LinuxX64;
    let prepared = prepare_local_project_for_target(&project.entry(), target)
        .expect("prepare current source with accepted dependency pins")
        .expect("application project");
    compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(prepared, project.workspace.join("native"), target)
            .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
}

#[test]
fn absent_acceptance_requires_ordinary_update_without_creating_a_lock() {
    for with_assumption in [true, false] {
        let project = TemporaryProject::new();
        if !with_assumption {
            std::fs::write(
                project.entry(),
                "data Main {}\nmachine Main::main(&mut self) {}\n",
            )
            .unwrap();
        }
        let error =
            native_project(&project).expect_err("native build cannot invent project acceptance");
        assert!(matches!(
            error,
            CompilePreparedLocalProjectNativeError::Evidence(
                AcceptedOrdinaryEvidenceError::RootPolicy(
                    FreshPackageRootPolicyError::ReviewRequired(_)
                )
            )
        ));
        assert!(error.to_string().contains("run omega update"));
        assert!(!project.source.join("omega.lock").exists());
    }
}

#[test]
fn changed_or_removed_assumptions_require_review_without_rewriting_acceptance() {
    for remove_assumption in [false, true] {
        let project = TemporaryProject::new();
        accept_project(&project, target::TargetProfile::LinuxX64);
        let lock_before = std::fs::read(project.source.join("omega.lock")).unwrap();
        let source = std::fs::read_to_string(project.entry()).unwrap();
        let changed = if remove_assumption {
            "data Main {}\nmachine Main::main(&mut self) {}\n".to_owned()
        } else {
            source.replace("ensures accepted(result);", "ensures accepted(42);")
        };
        std::fs::write(project.entry(), changed).unwrap();
        let error = native_project(&project)
            .expect_err("changed complete assumption contract requires review");
        assert!(matches!(error,
            CompilePreparedLocalProjectNativeError::Evidence(
                AcceptedOrdinaryEvidenceError::RootPolicy(FreshPackageRootPolicyError::ReviewRequired(ref changes))
            ) if changes.requires_decision()
        ));
        assert_eq!(
            std::fs::read(project.source.join("omega.lock")).unwrap(),
            lock_before
        );
    }
}

#[test]
fn equal_policy_source_edit_remains_visible_and_resolution_update_preserves_assumptions() {
    let project = TemporaryProject::new();
    let target = target::TargetProfile::LinuxX64;
    let accepted = accept_project(&project, target);
    let lock_before = std::fs::read(project.source.join("omega.lock")).unwrap();
    let source = std::fs::read_to_string(project.entry()).unwrap();
    std::fs::write(
        project.entry(),
        format!("{source}\n// Source-only edit, no policy change.\n"),
    )
    .unwrap();
    let prepared = prepare_local_project_for_target(&project.entry(), target)
        .unwrap()
        .unwrap();
    let (_, closure, retained) = prepared.into_review_parts();
    assert_eq!(retained.as_ref(), accepted.target(target));
    let review = review_package_change(
        closure,
        target,
        retained.as_ref(),
        &project.workspace.join("update"),
    )
    .unwrap();
    assert!(!review.changes().requires_decision());
    assert!(review.changes().source_subject_changed());
    assert!(review.changes().audit_recommended());
    assert!(
        review
            .changes()
            .packages()
            .iter()
            .any(|package| package.source_changed())
    );
    let decisions = resolve_package_policy_decisions(
        review.changes(),
        review.changes().fingerprint().digest(),
        &[],
    )
    .unwrap();
    let proposed = review.propose_lock_target(&decisions).unwrap();
    assert_eq!(
        proposed.baselines(),
        accepted.target(target).unwrap().baselines()
    );
    assert_ne!(
        proposed.source().fingerprint(),
        accepted.target(target).unwrap().source().fingerprint()
    );
    native_project(&project).expect("equal accepted policy survives a local source edit");
    assert_eq!(
        std::fs::read(project.source.join("omega.lock")).unwrap(),
        lock_before
    );
    let updated = PackageLock::from_targets(vec![proposed]).unwrap();
    std::fs::write(
        project.source.join("omega.lock"),
        updated.canonical_text().unwrap(),
    )
    .unwrap();
    native_project(&project).expect("resolution-only update retains accepted assumptions");
}
