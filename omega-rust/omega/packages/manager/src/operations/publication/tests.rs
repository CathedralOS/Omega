use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::transaction::PublicationStep;
use super::{PackageFileTransaction, PackagePublicationError, PackagePublicationLimits};

const BEFORE_BUILD: &[u8] = b"original build\n\0";
const AFTER_BUILD: &[u8] = b"published build\n\xff";
const BEFORE_LOCK: &[u8] = b"original lock\n\0";
const AFTER_LOCK: &[u8] = b"published lock\n\xfe";
const THIRD_PARTY: &[u8] = b"independent edit\n";
const CHILD_ROOT: &str = "OMEGA_MANAGER_PUBLICATION_TEST_CHILD_ROOT";
const CHILD_ACTION: &str = "OMEGA_MANAGER_PUBLICATION_TEST_CHILD_ACTION";
const INTERRUPTED_EXIT: i32 = 73;
const PROBED_EXIT: i32 = 74;
const STEPS: [PublicationStep; 3] = [
    PublicationStep::IntentRecorded,
    PublicationStep::BuildReplaced,
    PublicationStep::LockReplaced,
];

struct Project {
    root: PathBuf,
}

impl Project {
    fn new(before_lock: Option<&[u8]>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let root = std::env::temp_dir().join(format!(
            "omega-manager-publication-{}-{}-{}",
            std::process::id(),
            timestamp.as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let project = Self {
            root: root.canonicalize().unwrap(),
        };
        fs::create_dir(project.root.join("build")).unwrap();
        fs::write(project.root.join("build.omg"), BEFORE_BUILD).unwrap();
        if let Some(bytes) = before_lock {
            fs::write(project.root.join("omega.lock"), bytes).unwrap();
        }
        project
    }

    fn open(&self) -> PackageFileTransaction {
        PackageFileTransaction::open(&self.root, PackagePublicationLimits::default()).unwrap()
    }

    fn state(&self) -> PathBuf {
        self.root.join("build/package-manager")
    }

    fn journal(&self) -> PathBuf {
        self.state().join("pending")
    }

    fn assert_pair(&self, build: &[u8], lock: Option<&[u8]>) {
        assert_eq!(fs::read(self.root.join("build.omg")).unwrap(), build);
        assert_eq!(
            read_optional(&self.root.join("omega.lock")).as_deref(),
            lock
        );
    }

    fn assert_layout(&self, lock_present: bool, pending: bool) {
        let mut expected = vec!["build", "build.omg"];
        if lock_present {
            expected.push("omega.lock");
        }
        assert_eq!(entries(&self.root), expected);
        assert_eq!(entries(&self.root.join("build")), ["package-manager"]);
        let expected_state = if pending {
            vec!["pending", "transaction.lock"]
        } else {
            vec!["transaction.lock"]
        };
        assert_eq!(entries(&self.state()), expected_state);
        assert!(
            fs::symlink_metadata(self.state().join("transaction.lock"))
                .unwrap()
                .is_file()
        );
    }

    fn child(&self, action: &str) -> std::process::Output {
        // libtest names omit the crate prefix included by module_path!().
        let (_, module) = module_path!().split_once("::").unwrap();
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!("{module}::publication_subprocess"),
                "--nocapture",
            ])
            .env(CHILD_ROOT, &self.root)
            .env(CHILD_ACTION, action)
            .output()
            .unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn read_optional(path: &Path) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("{}: {error}", path.display()),
    }
}

fn entries(path: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

fn assert_pending_edit(error: PackagePublicationError, expected: &str) {
    let PackagePublicationError::Pending(cause) = error else {
        panic!("expected pending concurrent edit, got {error:?}");
    };
    assert!(matches!(
        *cause,
        PackagePublicationError::ConcurrentEdit { file } if file == expected
    ));
}

fn interrupt(
    transaction: &mut PackageFileTransaction,
    before_lock: Option<&[u8]>,
    stop: PublicationStep,
) {
    let mut visited = Vec::new();
    let error = transaction
        .publish_with_checkpoint(BEFORE_BUILD, AFTER_BUILD, before_lock, AFTER_LOCK, |step| {
            visited.push(step);
            if step == stop {
                Err(PackagePublicationError::InvalidJournal(
                    "injected interruption",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    let PackagePublicationError::Pending(cause) = error else {
        panic!("checkpoint failure lost durable intent: {error:?}");
    };
    assert!(matches!(
        *cause,
        PackagePublicationError::InvalidJournal("injected interruption")
    ));
    let count = STEPS.iter().position(|step| *step == stop).unwrap() + 1;
    assert_eq!(visited, STEPS[..count]);
}

fn assert_interrupted_pair(project: &Project, before_lock: Option<&[u8]>, step: PublicationStep) {
    let build = if step == PublicationStep::IntentRecorded {
        BEFORE_BUILD
    } else {
        AFTER_BUILD
    };
    let lock = if step == PublicationStep::LockReplaced {
        Some(AFTER_LOCK)
    } else {
        before_lock
    };
    project.assert_pair(build, lock);
    project.assert_layout(lock.is_some(), true);
}

fn recover_twice(project: &Project) {
    let mut transaction = project.open();
    assert!(transaction.has_pending().unwrap());
    assert!(matches!(
        transaction.read_pair(),
        Err(PackagePublicationError::RecoveryRequired)
    ));
    assert!(transaction.recover().unwrap());
    assert!(!transaction.has_pending().unwrap());
    let expected = (AFTER_BUILD.to_vec(), Some(AFTER_LOCK.to_vec()));
    assert_eq!(transaction.read_pair().unwrap(), expected);
    project.assert_pair(AFTER_BUILD, Some(AFTER_LOCK));
    project.assert_layout(true, false);
    assert!(!transaction.recover().unwrap());
    assert_eq!(transaction.read_pair().unwrap(), expected);
    drop(transaction);
    let mut reopened = project.open();
    assert!(!reopened.has_pending().unwrap());
    assert!(!reopened.recover().unwrap());
    assert_eq!(reopened.read_pair().unwrap(), expected);
    project.assert_pair(AFTER_BUILD, Some(AFTER_LOCK));
    project.assert_layout(true, false);
}

#[test]
fn initial_publication_and_update_publish_exact_pairs_without_stage_residue() {
    for before_lock in [None, Some(b"".as_slice()), Some(BEFORE_LOCK)] {
        let project = Project::new(before_lock);
        let mut transaction = project.open();
        assert_eq!(transaction.project_root(), project.root);
        assert!(!transaction.has_pending().unwrap());
        assert!(!transaction.recover().unwrap());
        project.assert_pair(BEFORE_BUILD, before_lock);
        assert_eq!(
            transaction.read_pair().unwrap(),
            (BEFORE_BUILD.to_vec(), before_lock.map(<[u8]>::to_vec))
        );
        transaction
            .publish(BEFORE_BUILD, AFTER_BUILD, before_lock, AFTER_LOCK)
            .unwrap();
        project.assert_pair(AFTER_BUILD, Some(AFTER_LOCK));
        project.assert_layout(true, false);
        assert!(!transaction.has_pending().unwrap());
        transaction
            .publish(AFTER_BUILD, BEFORE_BUILD, Some(AFTER_LOCK), BEFORE_LOCK)
            .unwrap();
        project.assert_pair(BEFORE_BUILD, Some(BEFORE_LOCK));
        project.assert_layout(true, false);
        drop(transaction);
        assert!(!project.open().recover().unwrap());
    }
}

#[test]
fn stale_either_file_rejects_the_whole_pair_before_intent() {
    for before_lock in [None, Some(b"".as_slice()), Some(BEFORE_LOCK)] {
        for changed in ["build.omg", "omega.lock"] {
            let project = Project::new(before_lock);
            let mut transaction = project.open();
            fs::write(project.root.join(changed), THIRD_PARTY).unwrap();
            let error = transaction
                .publish_with_checkpoint(BEFORE_BUILD, AFTER_BUILD, before_lock, AFTER_LOCK, |_| {
                    panic!("stale pair reached durable intent")
                })
                .unwrap_err();
            assert!(matches!(
                error,
                PackagePublicationError::ConcurrentEdit { file } if file == changed
            ));
            project.assert_pair(
                if changed == "build.omg" {
                    THIRD_PARTY
                } else {
                    BEFORE_BUILD
                },
                if changed == "omega.lock" {
                    Some(THIRD_PARTY)
                } else {
                    before_lock
                },
            );
            assert!(!transaction.has_pending().unwrap());
            project.assert_layout(changed == "omega.lock" || before_lock.is_some(), false);
        }
    }
}

#[test]
fn absent_and_empty_old_lock_are_not_interchangeable() {
    for (actual, expected) in [(None, Some(b"".as_slice())), (Some(b"".as_slice()), None)] {
        let project = Project::new(actual);
        let mut transaction = project.open();
        assert!(matches!(
            transaction.publish(BEFORE_BUILD, AFTER_BUILD, expected, AFTER_LOCK),
            Err(PackagePublicationError::ConcurrentEdit { file: "omega.lock" })
        ));
        project.assert_pair(BEFORE_BUILD, actual);
        project.assert_layout(actual.is_some(), false);
    }
}

#[test]
fn checkpoint_errors_reopen_and_complete_forward_at_every_step() {
    for before_lock in [None, Some(b"".as_slice()), Some(BEFORE_LOCK)] {
        for step in STEPS {
            let project = Project::new(before_lock);
            let mut transaction = project.open();
            interrupt(&mut transaction, before_lock, step);
            assert!(transaction.has_pending().unwrap());
            assert!(matches!(
                transaction.read_pair(),
                Err(PackagePublicationError::RecoveryRequired)
            ));
            assert_interrupted_pair(&project, before_lock, step);
            let journal = fs::read(project.journal()).unwrap();
            assert!(matches!(
                transaction.publish(BEFORE_BUILD, THIRD_PARTY, before_lock, THIRD_PARTY),
                Err(PackagePublicationError::RecoveryRequired)
            ));
            assert_eq!(fs::read(project.journal()).unwrap(), journal);
            assert_interrupted_pair(&project, before_lock, step);
            drop(transaction);
            recover_twice(&project);
        }
    }
}

#[test]
fn process_exit_at_every_checkpoint_releases_lock_and_recovers_forward() {
    for before_lock in [None, Some(BEFORE_LOCK)] {
        for step in STEPS {
            let project = Project::new(before_lock);
            let output = project.child(&format!("{step:?}"));
            assert_eq!(output.status.code(), Some(INTERRUPTED_EXIT), "{output:?}");
            assert_interrupted_pair(&project, before_lock, step);
            recover_twice(&project);
        }
    }
}

#[test]
fn third_party_edit_after_first_write_blocks_recovery_without_further_changes() {
    for before_lock in [None, Some(BEFORE_LOCK)] {
        for changed in ["build.omg", "omega.lock"] {
            let project = Project::new(before_lock);
            let mut transaction = project.open();
            interrupt(
                &mut transaction,
                before_lock,
                PublicationStep::BuildReplaced,
            );
            drop(transaction);
            fs::write(project.root.join(changed), THIRD_PARTY).unwrap();
            let journal = fs::read(project.journal()).unwrap();
            for _ in 0..2 {
                let mut transaction = project.open();
                assert_pending_edit(transaction.recover().unwrap_err(), changed);
                assert!(transaction.has_pending().unwrap());
                project.assert_pair(
                    if changed == "build.omg" {
                        THIRD_PARTY
                    } else {
                        AFTER_BUILD
                    },
                    if changed == "omega.lock" {
                        Some(THIRD_PARTY)
                    } else {
                        before_lock
                    },
                );
                assert_eq!(fs::read(project.journal()).unwrap(), journal);
                project.assert_layout(changed == "omega.lock" || before_lock.is_some(), true);
            }
        }
    }
}

#[test]
fn recovery_checks_stale_lock_before_writing_unchanged_build() {
    let project = Project::new(Some(BEFORE_LOCK));
    let mut transaction = project.open();
    interrupt(
        &mut transaction,
        Some(BEFORE_LOCK),
        PublicationStep::IntentRecorded,
    );
    drop(transaction);
    fs::write(project.root.join("omega.lock"), THIRD_PARTY).unwrap();
    let journal = fs::read(project.journal()).unwrap();
    assert_pending_edit(project.open().recover().unwrap_err(), "omega.lock");
    project.assert_pair(BEFORE_BUILD, Some(THIRD_PARTY));
    assert_eq!(fs::read(project.journal()).unwrap(), journal);
    project.assert_layout(true, true);
}

#[test]
fn corrupt_journal_is_retained_without_changing_either_file() {
    for step in STEPS {
        for corruption in [
            b"".as_slice(),
            b"invalid journal\n",
            b"omega-package-transaction 1\n",
        ] {
            let project = Project::new(Some(BEFORE_LOCK));
            let mut transaction = project.open();
            interrupt(&mut transaction, Some(BEFORE_LOCK), step);
            drop(transaction);
            fs::write(project.journal(), corruption).unwrap();
            for _ in 0..2 {
                let mut transaction = project.open();
                assert!(transaction.has_pending().unwrap());
                let error = transaction.recover().unwrap_err();
                let PackagePublicationError::Pending(cause) = error else {
                    panic!("corrupt journal lost pending status: {error:?}");
                };
                assert!(matches!(*cause, PackagePublicationError::InvalidJournal(_)));
                assert_eq!(fs::read(project.journal()).unwrap(), corruption);
                assert_interrupted_pair(&project, Some(BEFORE_LOCK), step);
            }
        }
    }
}

#[test]
fn transaction_contention_releases_on_drop_and_keeps_lock_file_on_reopen() {
    let project = Project::new(None);
    for _ in 0..2 {
        let transaction = project.open();
        assert!(matches!(
            PackageFileTransaction::open(&project.root, PackagePublicationLimits::default()),
            Err(PackagePublicationError::Busy)
        ));
        let output = project.child("busy");
        assert_eq!(output.status.code(), Some(PROBED_EXIT), "{output:?}");
        project.assert_layout(false, false);
        drop(transaction);
        project.assert_layout(false, false);
        let output = project.child("open");
        assert_eq!(output.status.code(), Some(PROBED_EXIT), "{output:?}");
        project.assert_pair(BEFORE_BUILD, None);
        project.assert_layout(false, false);
    }
}

// Only the spawned test process reads these variables. The parent never changes
// its environment, so unrelated tests and parallel fixtures cannot enter here.
#[test]
fn publication_subprocess() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    let action = std::env::var(CHILD_ACTION).unwrap();
    let opened = PackageFileTransaction::open(&root, PackagePublicationLimits::default());
    if action == "busy" {
        assert!(matches!(opened, Err(PackagePublicationError::Busy)));
        std::process::exit(PROBED_EXIT);
    }
    let mut transaction = opened.unwrap();
    if action == "open" {
        assert!(!transaction.has_pending().unwrap());
        drop(transaction);
        std::process::exit(PROBED_EXIT);
    }
    let stop = STEPS
        .into_iter()
        .find(|step| format!("{step:?}") == action)
        .unwrap();
    let before_lock = read_optional(&root.join("omega.lock"));
    transaction
        .publish_with_checkpoint(
            BEFORE_BUILD,
            AFTER_BUILD,
            before_lock.as_deref(),
            AFTER_LOCK,
            |step| {
                if step == stop {
                    // Bypass guard and staging destructors to exercise durable intent.
                    std::process::exit(INTERRUPTED_EXIT);
                }
                Ok(())
            },
        )
        .unwrap();
    panic!("publication did not visit {stop:?}");
}

mod preparation;

#[cfg(unix)]
mod unix;

#[cfg(not(unix))]
#[test]
fn unix_permissions_and_symlink_cases_require_unix() {
    eprintln!("SKIP: Unix executable permissions and symlink cases require a Unix host");
}
