use super::*;
use crate::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLockTarget,
};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, resolve_locked_package_source_closure_with_storage,
};
use crate::review::compile_resolved_package_reviews;
use std::fs;

mod fixture;
mod git;
mod offline;
use fixture::Project;

#[test]
fn local_root_main_edits_preserve_locked_dependencies_and_lock_bytes() {
    for role in ["application", "package"] {
        let project = Project::new(role);
        let lock = project.lock();
        let before = fs::read(project.root().join("omega.lock")).unwrap();
        let edited = "pub machine value() -> u64 { 19 }\n";
        fs::write(project.root().join("main.omg"), edited).unwrap();
        let prepared = project.prepare().unwrap().unwrap();
        assert_eq!(fs::read_to_string(&prepared.entry_path).unwrap(), edited);
        let accepted = lock.target(TargetProfile::host()).unwrap().source();
        let fresh = &prepared.source_closure;
        assert_ne!(
            fresh.custody(fresh.graph().root()).unwrap().resolution(),
            accepted.root().selected().resolution()
        );
        for package in accepted
            .packages()
            .iter()
            .filter(|package| package.key() != accepted.root().selected().key())
        {
            assert_eq!(
                fresh.custody(package.key()).unwrap().resolution(),
                package.resolution()
            );
        }
        // The explicit immutable-root API still rejects precisely this edit.
        assert!(
            resolve_locked_package_source_closure_with_storage(
                accepted,
                fresh.source_requests().root().request(),
                GitExactRevisionAcquisition::AllowFetch,
                &project.storage(),
                LocalSourceLimits::default(),
                PackageSourceClosureLimits::default(),
                CanonicalSourceClosureSubjectLimits::default(),
            )
            .is_err()
        );
        assert_eq!(fs::read(project.root().join("omega.lock")).unwrap(), before);
    }
}

#[test]
fn dependency_content_and_root_declaration_changes_require_update() {
    for change in [
        "dependency-content",
        "dependency-declaration",
        "root-request",
        "root-name",
        "root-role",
        "new-request",
    ] {
        let project = Project::new("package");
        project.lock();
        let before = fs::read(project.root().join("omega.lock")).unwrap();
        match change {
            "dependency-content" => fs::write(
                project.0.join("dependency/main.omg"),
                "pub machine value() -> u64 { 99 }\n",
            )
            .unwrap(),
            "dependency-declaration" => fs::write(
                project.0.join("dependency/build.omg"),
                "machine build(builder: &mut Build) { builder.package(\"renamed\"); }\n",
            )
            .unwrap(),
            _ => {
                let path = project.root().join("build.omg");
                let before = fs::read_to_string(&path).unwrap();
                let after = match change {
                    "root-request" => before.replace("\"dependency\", Source", "\"renamed\", Source"),
                    "root-name" => before.replace("\"root\"", "\"renamed\""),
                    "root-role" => before.replace("builder.package", "builder.application"),
                    "new-request" => before.replace("\n}", "\n builder.depend_as(\"missing\", Source::Path { location: \"../missing\" });\n}"),
                    _ => unreachable!(),
                };
                assert_ne!(before, after);
                fs::write(path, after).unwrap();
            }
        }
        let error = project.prepare().err().expect("locked drift rejects");
        assert!(
            error.to_string().contains("omega update"),
            "{change}: {error}"
        );
        assert_eq!(fs::read(project.root().join("omega.lock")).unwrap(), before);
    }
}

#[test]
fn missing_target_rejects_before_storage_or_missing_local_source_acquisition() {
    let project = Project::new("package");
    project.lock();
    fs::remove_dir_all(project.0.join("dependency")).unwrap();
    let other = TargetProfile::ALL
        .into_iter()
        .find(|target| *target != TargetProfile::host())
        .unwrap();
    let error = prepare_with_storage(&project.root().join("main.omg"), other, |_| {
        panic!("missing target must reject before storage and source acquisition")
    })
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("no accepted section for exact target")
    );
    assert!(error.to_string().contains("omega update"));
}

#[test]
fn unlocked_project_keeps_legacy_preparation_without_coordination_state() {
    let project = Project::new("package");
    assert!(project.prepare().unwrap().is_some());
    assert!(!project.root().join("build/package-manager").exists());
    assert!(!project.root().join("omega.lock").exists());
}

#[test]
fn lock_in_fresh_checkout_creates_coordination_state() {
    let project = Project::new("package");
    project.lock();
    assert!(!project.root().join("build/package-manager").exists());
    assert!(project.prepare().unwrap().is_some());
    assert!(
        project
            .root()
            .join("build/package-manager/transaction.lock")
            .is_file()
    );
}

#[test]
fn incompatible_lock_rejects_before_acquisition() {
    let project = Project::new("package");
    for bytes in [b"omega-lock 999\n".as_slice(), &[0xff]] {
        fs::write(project.root().join("omega.lock"), bytes).unwrap();
        let error = prepare_with_storage(
            &project.root().join("main.omg"),
            TargetProfile::host(),
            |_| panic!("incompatible lock must reject before acquisition"),
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("omega update"));
        assert_eq!(fs::read(project.root().join("omega.lock")).unwrap(), bytes);
    }
}

#[test]
fn recovery_precedes_baseline_loading_and_deleted_declaration_standalone_gate() {
    for (deleted, offline) in [(false, false), (true, false), (false, true), (true, true)] {
        let project = Project::new("package");
        let prepare = || {
            prepare_with_options_and_storage(
                &project.root().join("main.omg"),
                LocalProjectPreparationOptions {
                    target: TargetProfile::host(),
                    offline,
                },
                |_| Ok(project.storage()),
            )
        };
        project.lock();
        let after_build = fs::read_to_string(project.root().join("build.omg")).unwrap();
        let after_lock = fs::read_to_string(project.root().join("omega.lock")).unwrap();
        let before_build = "invalid declaration before interrupted publication\n";
        fs::write(project.root().join("build.omg"), before_build).unwrap();
        fs::remove_file(project.root().join("omega.lock")).unwrap();
        let transaction =
            PackageFileTransaction::open(&project.root(), PackagePublicationLimits::default())
                .unwrap();
        let journal = format!(
            "omega-package-transaction 1\nbefore-build {}\n{before_build}\nafter-build {}\n{after_build}\nbefore-lock absent\nafter-lock {}\n{after_lock}\n",
            before_build.len(),
            after_build.len(),
            after_lock.len()
        );
        fs::write(
            project.root().join("build/package-manager/pending"),
            &journal,
        )
        .unwrap();
        drop(transaction);
        if deleted {
            fs::remove_file(project.root().join("build.omg")).unwrap();
            assert!(matches!(
                prepare(),
                Err(PrepareLocalProjectError::Publication(_))
            ));
            assert_eq!(
                fs::read_to_string(project.root().join("build/package-manager/pending")).unwrap(),
                journal
            );
            assert!(!project.root().join("build.omg").exists());
        } else {
            assert!(prepare().unwrap().is_some());
            assert_eq!(
                fs::read_to_string(project.root().join("build.omg")).unwrap(),
                after_build
            );
            assert_eq!(
                fs::read_to_string(project.root().join("omega.lock")).unwrap(),
                after_lock
            );
            assert!(
                !project
                    .root()
                    .join("build/package-manager/pending")
                    .exists()
            );
        }
    }
}

#[test]
fn historical_root_spelling_uses_physical_caller_without_relaxing_strict_api() {
    let project = Project::new("package");
    let request_path = project.root().join(".");
    let closure = project.resolve(&request_path);
    let subject = CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(TargetProfile::host()),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    let request = PackageRootSourceRequest::ExternalLocal {
        requested_root: project.root().canonicalize().unwrap(),
        source_context: ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
    };
    let storage = project.storage();
    let strict = resolve_locked_package_source_closure_with_storage(
        &subject,
        &request,
        GitExactRevisionAcquisition::AllowFetch,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        CanonicalSourceClosureSubjectLimits::default(),
    );
    assert!(matches!(
        strict,
        Err(crate::resolution::graph::ResolveLockedPackageClosureError::RootRequestMismatch)
    ));
    assert!(
        resolve_locked_local_project_closure_with_storage(
            &subject,
            &request,
            GitExactRevisionAcquisition::AllowFetch,
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            CanonicalSourceClosureSubjectLimits::default()
        )
        .is_ok()
    );
    let wrong_request = PackageRootSourceRequest::ExternalLocal {
        requested_root: project.0.join("dependency"),
        source_context: ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
    };
    assert!(
        resolve_locked_local_project_closure_with_storage(
            &subject,
            &wrong_request,
            GitExactRevisionAcquisition::AllowFetch,
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            CanonicalSourceClosureSubjectLimits::default()
        )
        .is_err()
    );
}
