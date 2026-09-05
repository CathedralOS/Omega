use super::*;
use crate::operations::{PrepareLocalProjectError, prepare_local_project};
use crate::resolution::graph::ResolvedSourceIdentity;

const ORIGINAL_BUILD: &[u8] =
    b"machine build(builder: &mut Build) {\n    builder.package(\"before-publication\");\n}\n";
const PROPOSED_BUILD: &[u8] =
    b"machine build(builder: &mut Build) {\n    builder.package(\"after-publication\");\n}\n";
const MAIN: &[u8] = b"pub const PUBLIC_LIMIT: u64 = 4;\n";

fn valid_project() -> Project {
    let project = Project::new(None);
    fs::write(project.root.join("build.omg"), ORIGINAL_BUILD).unwrap();
    fs::write(project.root.join("main.omg"), MAIN).unwrap();
    project
}

fn prepare_snapshot(project: &Project, build: &[u8], name: &str) -> ResolvedSourceIdentity {
    let prepared = prepare_local_project(&project.root.join("main.omg"))
        .expect("prepare a valid Omega package")
        .expect("build.omg selects package preparation");
    let (entry, closure) = prepared.into_review_parts();
    let root = closure.graph().root();
    let snapshot = closure.source_root(root).unwrap();
    assert_ne!(snapshot, project.root);
    assert_eq!(entry, snapshot.join("main.omg"));
    assert_eq!(fs::read(&entry).unwrap(), MAIN);
    assert_eq!(fs::read(snapshot.join("build.omg")).unwrap(), build);
    assert_eq!(root.name().as_str(), name);
    assert_eq!(closure.graph().packages().len(), 1);
    closure.graph().packages()[0].source().clone()
}

fn proposed_lock(project: &Project) -> String {
    use crate::lock::PackageLock;
    use crate::operations::review_package_change;
    use crate::review::resolve_package_policy_decisions;
    fs::write(project.root.join("build.omg"), PROPOSED_BUILD).unwrap();
    let (_, closure) = prepare_local_project(&project.root.join("main.omg"))
        .unwrap()
        .unwrap()
        .into_review_parts();
    let review = review_package_change(
        closure,
        target::TargetProfile::host(),
        None,
        &project.root.join("build/check"),
    )
    .unwrap();
    let choices = resolve_package_policy_decisions(
        review.changes(),
        review.changes().fingerprint().digest(),
        &[],
    )
    .unwrap();
    let lock = PackageLock::from_targets(vec![review.propose_lock_target(&choices).unwrap()])
        .unwrap()
        .canonical_text()
        .unwrap();
    fs::write(project.root.join("build.omg"), ORIGINAL_BUILD).unwrap();
    lock
}

#[test]
fn preparation_recovers_pending_publication_before_snapshotting() {
    for stop in [
        PublicationStep::IntentRecorded,
        PublicationStep::BuildReplaced,
        PublicationStep::LockReplaced,
    ] {
        let project = valid_project();
        let original = prepare_snapshot(&project, ORIGINAL_BUILD, "before-publication");
        let after_lock = proposed_lock(&project);
        let after_lock = after_lock.as_bytes();
        let mut transaction = project.open();
        let error = transaction
            .publish_with_checkpoint(ORIGINAL_BUILD, PROPOSED_BUILD, None, after_lock, |step| {
                if step == stop {
                    Err(PackagePublicationError::InvalidJournal(
                        "injected interruption",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert!(matches!(error, PackagePublicationError::Pending(_)));
        assert!(matches!(
            transaction.read_pair(),
            Err(PackagePublicationError::RecoveryRequired)
        ));
        project.assert_pair(
            if stop == PublicationStep::IntentRecorded {
                ORIGINAL_BUILD
            } else {
                PROPOSED_BUILD
            },
            if stop == PublicationStep::LockReplaced {
                Some(after_lock)
            } else {
                None
            },
        );
        drop(transaction);

        let recovered = prepare_snapshot(&project, PROPOSED_BUILD, "after-publication");
        assert_ne!(recovered, original);
        project.assert_pair(PROPOSED_BUILD, Some(after_lock));
        assert!(!project.journal().exists());
        assert_eq!(entries(&project.state()), ["transaction.lock"]);
        let repeated = prepare_snapshot(&project, PROPOSED_BUILD, "after-publication");
        assert_eq!(repeated, recovered);
        let mut transaction = project.open();
        for _ in 0..2 {
            assert!(!transaction.recover().unwrap());
            assert_eq!(
                transaction.read_pair().unwrap(),
                (PROPOSED_BUILD.to_vec(), Some(after_lock.to_vec()))
            );
        }
        assert_eq!(
            entries(&project.root),
            ["build", "build.omg", "main.omg", "omega.lock"]
        );
    }
}

#[test]
fn preparation_rejects_busy_transaction_and_succeeds_after_guard_release() {
    let project = valid_project();
    let transaction = project.open();
    assert!(project.state().is_dir());
    assert!(matches!(
        prepare_local_project(&project.root.join("main.omg")),
        Err(PrepareLocalProjectError::Publication(
            PackagePublicationError::Busy
        ))
    ));
    project.assert_pair(ORIGINAL_BUILD, None);
    assert_eq!(fs::read(project.root.join("main.omg")).unwrap(), MAIN);
    assert_eq!(entries(&project.state()), ["transaction.lock"]);
    drop(transaction);
    prepare_snapshot(&project, ORIGINAL_BUILD, "before-publication");
    project.assert_pair(ORIGINAL_BUILD, None);
    assert_eq!(entries(&project.state()), ["transaction.lock"]);
}

#[test]
fn preparation_without_transaction_state_does_not_create_control_files() {
    let project = valid_project();
    assert!(
        PackageFileTransaction::open_if_present(&project.root, PackagePublicationLimits::default())
            .unwrap()
            .is_none()
    );
    prepare_snapshot(&project, ORIGINAL_BUILD, "before-publication");
    assert!(!project.state().exists());
    assert!(entries(&project.root.join("build")).is_empty());
    assert_eq!(entries(&project.root), ["build", "build.omg", "main.omg"]);
    project.assert_pair(ORIGINAL_BUILD, None);
}

#[test]
fn missing_build_file_during_pending_publication_is_not_standalone_source() {
    let project = valid_project();
    let mut transaction = project.open();
    let error = transaction
        .publish_with_checkpoint(ORIGINAL_BUILD, PROPOSED_BUILD, None, AFTER_LOCK, |_| {
            Err(PackagePublicationError::InvalidJournal(
                "injected interruption",
            ))
        })
        .unwrap_err();
    assert!(matches!(error, PackagePublicationError::Pending(_)));
    drop(transaction);
    fs::remove_file(project.root.join("build.omg")).unwrap();
    assert!(matches!(
        prepare_local_project(&project.root.join("main.omg")),
        Err(PrepareLocalProjectError::Publication(
            PackagePublicationError::Pending(_)
        ))
    ));
    assert!(project.journal().exists());
    assert!(!project.root.join("build.omg").exists());
    assert!(!project.root.join("omega.lock").exists());
}
