// Once an install has been accepted, recovery needs only its journal and the accepted files.
use super::transaction::PublicationStep;
use super::{
    JOURNAL_FILE, PROPOSAL_FILE, PackageFileTransaction, PackagePublicationError,
    PackagePublicationLimits, ReviewedProposal,
};
use crate::PackageCommandKind;
use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::path::Path;

const BEFORE: &[u8] = b"old Build";
const AFTER: &[u8] = b"reviewed Build";
const LOCK: &[u8] = b"reviewed lock";
const PROPOSAL: &[u8] = b"the reviewed proposal";

fn interrupt(project: &Path, stop: PublicationStep) {
    fs::write(project.join("build.omg"), BEFORE).unwrap();
    let mut workspace = PackageFileTransaction::open(project, Default::default()).unwrap();
    fs::write(
        project.join("build/package-manager").join(PROPOSAL_FILE),
        PROPOSAL,
    )
    .unwrap();
    let result = workspace.publish(
        ReviewedProposal {
            kind: PackageCommandKind::Install,
            digest: crate::digest_encoder::content_identity(PROPOSAL),
        },
        BEFORE,
        AFTER,
        None,
        LOCK,
        |step| {
            if step == stop {
                Err(PackagePublicationError::Busy)
            } else {
                Ok(())
            }
        },
    );
    assert!(matches!(result, Err(PackagePublicationError::Pending(_))));
}

#[test]
fn an_interrupted_publication_finishes_the_pair_and_retires_its_proposal() {
    for step in [
        PublicationStep::IntentRecorded,
        PublicationStep::BuildReplaced,
        PublicationStep::LockReplaced,
    ] {
        let directory = TemporaryDirectory::create("interrupted-install", false).unwrap();
        let project = directory.path();
        interrupt(project, step);
        let mut recovered = PackageFileTransaction::open(project, Default::default()).unwrap();
        assert_eq!(
            recovered.recover().unwrap(),
            Some(PackageCommandKind::Install)
        );
        assert_eq!(
            recovered.read_pair().unwrap(),
            (AFTER.to_vec(), Some(LOCK.to_vec()))
        );
        let state = project.join("build/package-manager");
        assert!(!state.join(PROPOSAL_FILE).exists());
        assert!(!state.join(JOURNAL_FILE).exists());
        assert_eq!(recovered.recover().unwrap(), None);
    }
}

#[test]
fn recovery_preserves_a_changed_proposal_and_an_unrelated_project_edit() {
    let directory = TemporaryDirectory::create("changed-package-publication", false).unwrap();
    let project = directory.path();
    interrupt(project, PublicationStep::BuildReplaced);
    let state = project.join("build/package-manager");
    let journal = fs::read(state.join(JOURNAL_FILE)).unwrap();
    let mut recovery =
        PackageFileTransaction::open(project, PackagePublicationLimits::default()).unwrap();
    for (name, path, original) in [
        (PROPOSAL_FILE, state.join(PROPOSAL_FILE), PROPOSAL),
        ("build.omg", project.join("build.omg"), AFTER),
    ] {
        fs::write(&path, b"external edit").unwrap();
        let error = recovery.recover().unwrap_err();
        assert!(matches!(error, PackagePublicationError::Pending(inner)
            if matches!(*inner, PackagePublicationError::ConcurrentEdit { file } if file == name)));
        assert_eq!(fs::read(&path).unwrap(), b"external edit");
        assert_eq!(fs::read(state.join(JOURNAL_FILE)).unwrap(), journal);
        assert!(!project.join("omega.lock").exists());
        fs::write(path, original).unwrap();
    }
    assert_eq!(
        recovery.recover().unwrap(),
        Some(PackageCommandKind::Install)
    );
}

#[test]
fn a_crash_after_removing_the_proposal_still_finishes_the_committed_install() {
    let directory = TemporaryDirectory::create("removed-package-proposal", false).unwrap();
    let project = directory.path();
    interrupt(project, PublicationStep::LockReplaced);
    fs::remove_file(project.join("build/package-manager").join(PROPOSAL_FILE)).unwrap();
    let mut recovered = PackageFileTransaction::open(project, Default::default()).unwrap();
    assert_eq!(
        recovered.recover().unwrap(),
        Some(PackageCommandKind::Install)
    );
    assert_eq!(
        recovered.read_pair().unwrap(),
        (AFTER.to_vec(), Some(LOCK.to_vec()))
    );
}

#[test]
fn a_journal_without_a_review_identity_cannot_publish() {
    let directory = TemporaryDirectory::create("legacy-package-publication", false).unwrap();
    let project = directory.path();
    fs::write(project.join("build.omg"), BEFORE).unwrap();
    let mut recovered = PackageFileTransaction::open(project, Default::default()).unwrap();
    let state = project.join("build/package-manager");
    fs::write(state.join(PROPOSAL_FILE), b"separate pending review").unwrap();
    let journal = b"omega-package-transaction 1\nbefore-build 9\nold Build\nafter-build 14\nreviewed Build\nbefore-lock absent\nafter-lock 13\nreviewed lock\n";
    fs::write(state.join(JOURNAL_FILE), journal).unwrap();
    let error = recovered.recover().unwrap_err();
    assert!(
        matches!(error, PackagePublicationError::Pending(ref reason)
            if matches!(**reason, PackagePublicationError::InvalidJournal("invalid header"))),
        "{error}"
    );
    assert_eq!(fs::read(project.join("build.omg")).unwrap(), BEFORE);
    assert!(!project.join("omega.lock").exists());
    assert_eq!(fs::read(state.join(JOURNAL_FILE)).unwrap(), journal);
    assert_eq!(
        fs::read(state.join(PROPOSAL_FILE)).unwrap(),
        b"separate pending review"
    );
}
