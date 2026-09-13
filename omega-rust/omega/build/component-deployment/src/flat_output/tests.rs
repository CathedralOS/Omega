use super::{validate_published_executable, write_atomic_executable};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omega-component-output-{}-{timestamp}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("fresh scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn output_publication_replays_bytes_and_detects_later_drift() {
    let scratch = ScratchDirectory::new();
    let path = scratch.0.join("program.bin");
    let bytes = [0x90, 0xc3];
    write_atomic_executable(&path, &bytes).expect("publish exact bytes");
    validate_published_executable(&path, &bytes).expect("replay published bytes and mode");
    assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 1);
    std::fs::write(&path, [0xc3]).expect("tamper with published file");
    assert!(
        validate_published_executable(&path, &bytes)
            .unwrap_err()
            .contains("bytes differ")
    );
}

#[test]
fn failed_rename_preserves_destination_and_removes_staged_file() {
    let scratch = ScratchDirectory::new();
    let destination = scratch.0.join("program.bin");
    std::fs::create_dir(&destination).expect("directory blocks file publication");
    let sentinel = destination.join("retained");
    std::fs::write(&sentinel, b"original").unwrap();
    let error = write_atomic_executable(&destination, &[0xc3]).unwrap_err();
    assert!(error.contains("failed to install terminal output"));
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn output_replay_rejects_executable_mode_drift() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = ScratchDirectory::new();
    let path = scratch.0.join("program.bin");
    write_atomic_executable(&path, &[0xc3]).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        validate_published_executable(&path, &[0xc3])
            .unwrap_err()
            .contains("mode 0755")
    );
}
