use std::os::unix::fs::{PermissionsExt, symlink};

use super::*;

#[test]
fn publication_and_forward_recovery_preserve_executable_permissions() {
    for stop in [
        None,
        Some(PublicationStep::IntentRecorded),
        Some(PublicationStep::BuildReplaced),
        Some(PublicationStep::LockReplaced),
    ] {
        let project = Project::new(Some(BEFORE_LOCK));
        for (file, mode) in [("build.omg", 0o751), ("omega.lock", 0o705)] {
            fs::set_permissions(project.root.join(file), fs::Permissions::from_mode(mode)).unwrap();
        }
        let mut transaction = project.open();
        if let Some(step) = stop {
            interrupt(&mut transaction, Some(BEFORE_LOCK), step);
            drop(transaction);
            recover_twice(&project);
        } else {
            transaction
                .publish(BEFORE_BUILD, AFTER_BUILD, Some(BEFORE_LOCK), AFTER_LOCK)
                .unwrap();
        }
        project.assert_pair(AFTER_BUILD, Some(AFTER_LOCK));
        for (file, mode) in [("build.omg", 0o751), ("omega.lock", 0o705)] {
            assert_eq!(
                fs::metadata(project.root.join(file))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                mode
            );
        }
        project.assert_layout(true, false);
    }
}

#[test]
fn symlinked_build_and_state_directories_reject_without_touching_targets() {
    for relative in ["build", "build/package-manager"] {
        for dangling in [false, true] {
            let project = Project::new(Some(BEFORE_LOCK));
            let outside = Project::new(None);
            let target = outside
                .root
                .join(if dangling { "missing" } else { "build" });
            let destination = project.root.join(relative);
            if relative == "build" {
                fs::remove_dir(&destination).unwrap();
            }
            symlink(&target, &destination).unwrap();
            assert!(
                PackageFileTransaction::open(&project.root, PackagePublicationLimits::default())
                    .is_err()
            );
            assert_eq!(fs::read_link(&destination).unwrap(), target);
            assert!(entries(&outside.root.join("build")).is_empty());
            assert!(!outside.root.join("missing").exists());
            project.assert_pair(BEFORE_BUILD, Some(BEFORE_LOCK));
            outside.assert_pair(BEFORE_BUILD, None);
        }
    }
}

#[test]
fn symlinked_transaction_lock_rejects_without_touching_target() {
    for dangling in [false, true] {
        let project = Project::new(None);
        let outside = Project::new(None);
        fs::create_dir(project.state()).unwrap();
        let target = outside.root.join("lock-target");
        if !dangling {
            fs::write(&target, THIRD_PARTY).unwrap();
        }
        let destination = project.state().join("transaction.lock");
        symlink(&target, &destination).unwrap();
        assert!(
            PackageFileTransaction::open(&project.root, PackagePublicationLimits::default())
                .is_err()
        );
        assert_eq!(fs::read_link(&destination).unwrap(), target);
        assert_eq!(
            read_optional(&target).as_deref(),
            if dangling { None } else { Some(THIRD_PARTY) }
        );
        assert_eq!(entries(&project.state()), ["transaction.lock"]);
        project.assert_pair(BEFORE_BUILD, None);
    }
}

#[test]
fn symlinked_pending_journal_blocks_publication_and_recovery() {
    for dangling in [false, true] {
        let project = Project::new(Some(BEFORE_LOCK));
        let outside = Project::new(None);
        let mut transaction = project.open();
        interrupt(
            &mut transaction,
            Some(BEFORE_LOCK),
            PublicationStep::IntentRecorded,
        );
        drop(transaction);
        let target = outside.root.join("journal-target");
        let original = fs::read(project.journal()).unwrap();
        if !dangling {
            fs::write(&target, &original).unwrap();
        }
        fs::remove_file(project.journal()).unwrap();
        symlink(&target, project.journal()).unwrap();
        let mut transaction = project.open();
        assert!(transaction.has_pending().is_err());
        assert!(
            transaction
                .publish(BEFORE_BUILD, AFTER_BUILD, Some(BEFORE_LOCK), AFTER_LOCK)
                .is_err()
        );
        assert!(transaction.recover().is_err());
        assert_eq!(fs::read_link(project.journal()).unwrap(), target);
        assert_eq!(
            read_optional(&target).as_deref(),
            if dangling {
                None
            } else {
                Some(original.as_slice())
            }
        );
        project.assert_pair(BEFORE_BUILD, Some(BEFORE_LOCK));
        project.assert_layout(true, true);
    }
}

#[test]
fn symlinked_destinations_reject_before_journaling_even_when_bytes_match() {
    for (destination, before) in [("build.omg", BEFORE_BUILD), ("omega.lock", BEFORE_LOCK)] {
        for dangling in [false, true] {
            let project = Project::new(Some(BEFORE_LOCK));
            let outside = Project::new(None);
            let mut transaction = project.open();
            let target = outside.root.join("destination-target");
            if !dangling {
                fs::write(&target, before).unwrap();
            }
            fs::remove_file(project.root.join(destination)).unwrap();
            symlink(&target, project.root.join(destination)).unwrap();
            // A dangling lock must not be mistaken for the absent initial lock.
            let before_lock = if destination == "omega.lock" && dangling {
                None
            } else {
                Some(BEFORE_LOCK)
            };
            assert!(
                transaction
                    .publish(BEFORE_BUILD, AFTER_BUILD, before_lock, AFTER_LOCK)
                    .is_err()
            );
            assert!(!transaction.has_pending().unwrap());
            assert_eq!(
                fs::read_link(project.root.join(destination)).unwrap(),
                target
            );
            assert_eq!(
                read_optional(&target).as_deref(),
                if dangling { None } else { Some(before) }
            );
            let (other, bytes) = if destination == "build.omg" {
                ("omega.lock", BEFORE_LOCK)
            } else {
                ("build.omg", BEFORE_BUILD)
            };
            assert_eq!(fs::read(project.root.join(other)).unwrap(), bytes);
            project.assert_layout(true, false);
        }
    }
}

#[test]
fn recovery_rejects_symlinked_destinations_before_changing_the_pair() {
    for step in [
        PublicationStep::IntentRecorded,
        PublicationStep::BuildReplaced,
    ] {
        for destination in ["build.omg", "omega.lock"] {
            for dangling in [false, true] {
                let project = Project::new(Some(BEFORE_LOCK));
                let outside = Project::new(None);
                let mut transaction = project.open();
                interrupt(&mut transaction, Some(BEFORE_LOCK), step);
                drop(transaction);
                let build = if step == PublicationStep::IntentRecorded {
                    BEFORE_BUILD
                } else {
                    AFTER_BUILD
                };
                let before = if destination == "build.omg" {
                    build
                } else {
                    BEFORE_LOCK
                };
                let target = outside.root.join("recovery-target");
                if !dangling {
                    fs::write(&target, before).unwrap();
                }
                fs::remove_file(project.root.join(destination)).unwrap();
                symlink(&target, project.root.join(destination)).unwrap();
                let journal = fs::read(project.journal()).unwrap();
                for _ in 0..2 {
                    let mut transaction = project.open();
                    assert!(transaction.recover().is_err());
                    assert!(transaction.has_pending().unwrap());
                    assert_eq!(fs::read(project.journal()).unwrap(), journal);
                    assert_eq!(
                        fs::read_link(project.root.join(destination)).unwrap(),
                        target
                    );
                    assert_eq!(
                        read_optional(&target).as_deref(),
                        if dangling { None } else { Some(before) }
                    );
                    let (other, bytes) = if destination == "build.omg" {
                        ("omega.lock", BEFORE_LOCK)
                    } else {
                        ("build.omg", build)
                    };
                    assert_eq!(fs::read(project.root.join(other)).unwrap(), bytes);
                    project.assert_layout(true, true);
                }
            }
        }
    }
}
