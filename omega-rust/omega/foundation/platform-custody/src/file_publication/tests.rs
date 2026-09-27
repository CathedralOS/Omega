use super::{install_staged_files, publish_exact_file_bytes, stage_exact_bytes};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct PublicationDirectory(PathBuf);

impl PublicationDirectory {
    fn new() -> Self {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "omega-file-publication-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create publication test directory");
        Self(path)
    }
}

impl Drop for PublicationDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn stages_without_replacing_then_installs_exact_bytes_and_executable_mode() {
    let directory = PublicationDirectory::new();
    let executable = directory.0.join("program");
    let companion = directory.0.join("companion");
    std::fs::write(&executable, b"old program").expect("write previous program");
    std::fs::write(&companion, b"old companion").expect("write previous companion");
    let staged = [
        stage_exact_bytes(&executable, b"new program", true).expect("stage executable"),
        stage_exact_bytes(&companion, b"new companion", false).expect("stage companion"),
    ];
    assert_eq!(std::fs::read(&executable).unwrap(), b"old program");
    assert_eq!(std::fs::read(&companion).unwrap(), b"old companion");

    assert!(stage_exact_bytes(&directory.0.join("absent/file"), b"bytes", false).is_err());
    assert_eq!(std::fs::read(&executable).unwrap(), b"old program");
    assert_eq!(std::fs::read(&companion).unwrap(), b"old companion");

    install_staged_files(&staged).expect("install staged set");
    assert_eq!(std::fs::read(&executable).unwrap(), b"new program");
    assert_eq!(std::fs::read(&companion).unwrap(), b"new companion");
    assert!(staged.iter().all(|file| !file.staged.exists()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&executable).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
    publish_exact_file_bytes(&companion, b"single output").expect("replace a single output");
    assert_eq!(std::fs::read(&companion).unwrap(), b"single output");
}

#[test]
fn installation_failure_removes_installed_prefix_but_preserves_untouched_destinations() {
    let directory = PublicationDirectory::new();
    let first = directory.0.join("first");
    let obstructed = directory.0.join("directory");
    let last = directory.0.join("last");
    std::fs::write(&first, b"old first").unwrap();
    std::fs::create_dir(&obstructed).unwrap();
    std::fs::write(&last, b"old last").unwrap();
    let staged = [
        stage_exact_bytes(&first, b"new first", false).unwrap(),
        stage_exact_bytes(&obstructed, b"cannot replace directory", false).unwrap(),
        stage_exact_bytes(&last, b"new last", false).unwrap(),
    ];

    assert!(install_staged_files(&staged).is_err());
    assert!(!first.exists(), "installed files are removed, not restored");
    assert!(obstructed.is_dir());
    assert_eq!(std::fs::read(&last).unwrap(), b"old last");
}

#[test]
fn replay_rejects_changed_staging_bytes_and_removes_the_installed_file() {
    let directory = PublicationDirectory::new();
    let first = directory.0.join("first");
    let last = directory.0.join("last");
    std::fs::write(&last, b"old last").unwrap();
    let staged = [
        stage_exact_bytes(&first, b"expected bytes", false).unwrap(),
        stage_exact_bytes(&last, b"new last", false).unwrap(),
    ];
    std::fs::write(&staged[0].staged, b"changed bytes").unwrap();

    let error = install_staged_files(&staged).expect_err("reject changed bytes");
    assert!(error.contains("failed exact replay"), "{error}");
    assert!(!first.exists());
    assert_eq!(std::fs::read(&last).unwrap(), b"old last");
}
