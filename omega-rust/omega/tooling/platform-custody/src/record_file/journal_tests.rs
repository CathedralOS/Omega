use super::*;
use cap_std::ambient_authority;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const LIMITS: RecordFileLimits = RecordFileLimits { maximum_bytes: 64 };
const RECORD: &str = "build.omg";
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock follows Unix epoch")
            .as_nanos();
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "omega-record-journal-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn root(&self) -> RecordFileRoot {
        let directory = CapabilityDirectory::open_ambient_dir(&self.0, ambient_authority())
            .expect("open test directory");
        RecordFileRoot::from_directory(directory, self.0.clone()).expect("retain directory")
    }

    fn write(&self, bytes: &[u8]) {
        fs::write(self.0.join(RECORD), bytes).expect("write test record");
    }

    fn assert_bytes(&self, bytes: &[u8]) {
        assert_eq!(
            fs::read(self.0.join(RECORD)).expect("read test record"),
            bytes
        );
    }

    fn assert_no_stage(&self) {
        assert!(fs::read_dir(&self.0).expect("list directory").all(|entry| {
            !entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".omega-record-stage-")
        }));
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test directory");
    }
}

#[test]
fn optional_read_distinguishes_missing_existing_and_invalid_records() {
    let directory = TestDirectory::new();
    let root = directory.root();
    assert!(
        root.read_optional(Path::new(RECORD), LIMITS)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        root.read(Path::new(RECORD), LIMITS),
        Err(RecordFileError::Io { .. })
    ));
    for invalid in ["", ".", "..", "../build.omg", "nested/build.omg"] {
        assert!(matches!(
            root.read_optional(Path::new(invalid), LIMITS),
            Err(RecordFileError::InvalidDestination { .. })
        ));
    }
    assert!(matches!(
        root.read_optional(&directory.0.join(RECORD), LIMITS),
        Err(RecordFileError::InvalidDestination { .. })
    ));
    assert!(matches!(
        root.read_optional(Path::new("bad\0name"), LIMITS),
        Err(RecordFileError::Io { .. })
    ));
    fs::create_dir(directory.0.join("subdirectory")).unwrap();
    assert!(matches!(
        root.read_optional(Path::new("subdirectory"), LIMITS),
        Err(RecordFileError::NotRegularFile { .. })
    ));
    directory.write(b"first");
    assert_eq!(
        root.read_optional(Path::new(RECORD), LIMITS)
            .unwrap()
            .unwrap()
            .bytes(),
        b"first"
    );
    assert!(matches!(
        root.read_optional(Path::new(RECORD), RecordFileLimits { maximum_bytes: 4 }),
        Err(RecordFileError::ByteLimitExceeded { .. })
    ));
}

#[cfg(unix)]
#[test]
fn optional_read_rejects_live_and_dangling_symlinks() {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new();
    let root = directory.root();
    symlink("target", directory.0.join(RECORD)).unwrap();
    assert!(matches!(
        root.read_optional(Path::new(RECORD), LIMITS),
        Err(RecordFileError::NotRegularFile { .. })
    ));
    fs::write(directory.0.join("target"), b"first").unwrap();
    assert!(matches!(
        root.read_optional(Path::new(RECORD), LIMITS),
        Err(RecordFileError::NotRegularFile { .. })
    ));
}

#[test]
fn retained_replace_rejects_prior_in_place_edits_and_cleans_stage() {
    let directory = TestDirectory::new();
    directory.write(b"first");
    let root = directory.root();
    let read = root.read(Path::new(RECORD), LIMITS).unwrap();
    directory.write(b"other");
    assert!(matches!(
        read.replace(b"proposed", LIMITS),
        Err(RecordFileError::ContentsChanged { .. })
    ));
    directory.assert_bytes(b"other");
    directory.assert_no_stage();
}

#[test]
fn retained_mutations_reject_replaced_pathname_even_with_identical_bytes() {
    let directory = TestDirectory::new();
    directory.write(b"first");
    let root = directory.root();
    let replacement = root.read(Path::new(RECORD), LIMITS).unwrap();
    let removal = root.read(Path::new(RECORD), LIMITS).unwrap();
    fs::rename(directory.0.join(RECORD), directory.0.join("retained")).unwrap();
    directory.write(b"first");
    assert!(matches!(
        replacement.replace(b"proposed", LIMITS),
        Err(RecordFileError::NotRegularFile { .. })
    ));
    assert!(matches!(
        removal.remove(LIMITS),
        Err(RecordFileError::NotRegularFile { .. })
    ));
    directory.assert_bytes(b"first");
    directory.assert_no_stage();
}

#[cfg(unix)]
#[test]
fn retained_mutations_reject_pathname_changed_to_symlink() {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new();
    directory.write(b"first");
    let root = directory.root();
    let replacement = root.read(Path::new(RECORD), LIMITS).unwrap();
    let removal = root.read(Path::new(RECORD), LIMITS).unwrap();
    fs::rename(directory.0.join(RECORD), directory.0.join("retained")).unwrap();
    symlink("retained", directory.0.join(RECORD)).unwrap();
    assert!(matches!(
        replacement.replace(b"proposed", LIMITS),
        Err(RecordFileError::NotRegularFile { .. })
    ));
    assert!(matches!(
        removal.remove(LIMITS),
        Err(RecordFileError::NotRegularFile { .. })
    ));
    directory.assert_bytes(b"first");
    directory.assert_no_stage();
}

#[test]
fn retained_replace_publishes_exact_bytes_and_cleans_stage() {
    let directory = TestDirectory::new();
    directory.write(b"first");
    let root = directory.root();
    root.read(Path::new(RECORD), LIMITS)
        .unwrap()
        .replace(b"proposed", LIMITS)
        .unwrap();
    directory.assert_bytes(b"proposed");
    directory.assert_no_stage();
}

#[cfg(unix)]
#[test]
fn retained_replace_preserves_executable_permissions_and_mutable_replace_stays_private() {
    use std::os::unix::fs::PermissionsExt;

    let directory = TestDirectory::new();
    directory.write(b"first");
    let path = directory.0.join(RECORD);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o751)).unwrap();
    let root = directory.root();
    root.read(Path::new(RECORD), LIMITS)
        .unwrap()
        .replace(b"proposed", LIMITS)
        .unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
        0o751
    );
    directory.assert_bytes(b"proposed");
    root.replace_existing(Path::new(RECORD), b"mutable", LIMITS)
        .unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    directory.assert_no_stage();
}

#[test]
fn retained_mutations_enforce_limits_before_publication() {
    let directory = TestDirectory::new();
    directory.write(b"first");
    let root = directory.root();
    let small = RecordFileLimits { maximum_bytes: 4 };
    assert!(matches!(
        root.read(Path::new(RECORD), LIMITS)
            .unwrap()
            .replace(b"oversized", small),
        Err(RecordFileError::ByteLimitExceeded { .. })
    ));
    assert!(matches!(
        root.read(Path::new(RECORD), LIMITS)
            .unwrap()
            .replace(b"new", small),
        Err(RecordFileError::ByteLimitExceeded { .. })
    ));
    assert!(matches!(
        root.read(Path::new(RECORD), LIMITS).unwrap().remove(small),
        Err(RecordFileError::ByteLimitExceeded { .. })
    ));
    directory.assert_bytes(b"first");
    directory.assert_no_stage();
}

#[test]
fn retained_remove_rejects_prior_edits_then_removes_current_record() {
    let directory = TestDirectory::new();
    directory.write(b"first");
    let root = directory.root();
    let read = root.read(Path::new(RECORD), LIMITS).unwrap();
    directory.write(b"other");
    assert!(matches!(
        read.remove(LIMITS),
        Err(RecordFileError::ContentsChanged { .. })
    ));
    directory.assert_bytes(b"other");
    root.read(Path::new(RECORD), LIMITS)
        .unwrap()
        .remove(LIMITS)
        .unwrap();
    assert!(
        root.read_optional(Path::new(RECORD), LIMITS)
            .unwrap()
            .is_none()
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn shared_stage_writer_keeps_write_new_exclusive() {
    let directory = TestDirectory::new();
    let root = directory.root();
    root.write_new(Path::new(RECORD), b"first", LIMITS).unwrap();
    assert!(matches!(
        root.write_new(Path::new(RECORD), b"other", LIMITS),
        Err(RecordFileError::DestinationExists { .. })
    ));
    directory.assert_bytes(b"first");
    directory.assert_no_stage();
}

#[test]
fn lock_contends_then_reacquires_the_same_persistent_file_after_drop() {
    let directory = TestDirectory::new();
    directory.write(b"persistent bytes");
    let root = directory.root();
    let other_root = directory.root();
    let guard = root.try_lock(Path::new(RECORD)).unwrap().unwrap();
    let mut original = root.read(Path::new(RECORD), LIMITS).unwrap();
    guard.verify_current().unwrap();
    assert!(other_root.try_lock(Path::new(RECORD)).unwrap().is_none());
    drop(guard);
    let reacquired = other_root.try_lock(Path::new(RECORD)).unwrap().unwrap();
    original.verify_current(LIMITS).unwrap();
    reacquired.verify_current().unwrap();
    directory.assert_bytes(b"persistent bytes");
}

#[test]
fn lock_creates_a_persistent_file_and_owns_its_directory() {
    let directory = TestDirectory::new();
    let root = directory.root();
    let guard = root.try_lock(Path::new(RECORD)).unwrap().unwrap();
    drop(root);
    guard.verify_current().unwrap();
    directory.assert_bytes(b"");
    drop(guard);
    directory.assert_bytes(b"");
    directory
        .root()
        .try_lock(Path::new(RECORD))
        .unwrap()
        .unwrap()
        .verify_current()
        .unwrap();
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn lock_rejects_a_replaced_pathname() {
    let directory = TestDirectory::new();
    let root = directory.root();
    let guard = root.try_lock(Path::new(RECORD)).unwrap().unwrap();
    fs::rename(directory.0.join(RECORD), directory.0.join("retained")).unwrap();
    directory.write(b"");
    assert!(matches!(
        guard.verify_current(),
        Err(RecordFileError::NotRegularFile { .. })
    ));
    drop(guard);
    directory.assert_bytes(b"");
}

#[test]
fn lock_rejects_invalid_destinations_and_directories() {
    let directory = TestDirectory::new();
    let root = directory.root();
    assert!(matches!(
        root.try_lock(Path::new("../lock")),
        Err(RecordFileError::InvalidDestination { .. })
    ));
    fs::create_dir(directory.0.join(RECORD)).unwrap();
    assert!(matches!(
        root.try_lock(Path::new(RECORD)),
        Err(RecordFileError::NotRegularFile { .. })
    ));
}

#[cfg(unix)]
#[test]
fn lock_rejects_symlinks_and_detects_a_symlink_replacing_its_pathname() {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new();
    let root = directory.root();
    symlink("target", directory.0.join(RECORD)).unwrap();
    assert!(matches!(
        root.try_lock(Path::new(RECORD)),
        Err(RecordFileError::NotRegularFile { .. })
    ));
    assert!(!directory.0.join("target").exists());
    fs::remove_file(directory.0.join(RECORD)).unwrap();
    let guard = root.try_lock(Path::new(RECORD)).unwrap().unwrap();
    fs::rename(directory.0.join(RECORD), directory.0.join("target")).unwrap();
    symlink("target", directory.0.join(RECORD)).unwrap();
    assert!(matches!(
        guard.verify_current(),
        Err(RecordFileError::NotRegularFile { .. })
    ));
    assert!(matches!(
        root.try_lock(Path::new(RECORD)),
        Err(RecordFileError::NotRegularFile { .. })
    ));
}

#[test]
fn supplied_staging_root_keeps_prepared_bytes_out_of_source_root() {
    let directory = TestDirectory::new();
    directory.write(b"first");
    let root = directory.root();
    let staging_path = directory.0.join("build").join("package-manager");
    fs::create_dir_all(&staging_path).unwrap();
    let staging = RecordFileRoot::from_directory(
        CapabilityDirectory::open_ambient_dir(&staging_path, ambient_authority()).unwrap(),
        staging_path.clone(),
    )
    .unwrap();
    let prepared = stage::prepare(&staging, b"proposed", LIMITS, None).unwrap();
    assert_eq!(prepared.display_path.parent(), Some(staging_path.as_path()));
    assert_eq!(fs::read_dir(&staging_path).unwrap().count(), 1);
    directory.assert_no_stage();
    directory.assert_bytes(b"first");
    root.read(Path::new(RECORD), LIMITS)
        .unwrap()
        .replace_in(b"proposed", LIMITS, &staging)
        .unwrap();
    root.write_new_in(Path::new("second.record"), b"second", LIMITS, &staging)
        .unwrap();
    directory.assert_bytes(b"proposed");
    assert_eq!(
        fs::read(directory.0.join("second.record")).unwrap(),
        b"second"
    );
    directory.assert_no_stage();
    // A pre-publication stage remains only in the supplied control directory.
    assert_eq!(fs::read_dir(&staging_path).unwrap().count(), 1);
    drop(prepared);
    assert_eq!(fs::read_dir(&staging_path).unwrap().count(), 0);
}

#[test]
fn supplied_staging_root_cleans_failed_publications_without_changing_destination() {
    let directory = TestDirectory::new();
    let staging_directory = TestDirectory::new();
    directory.write(b"first");
    let root = directory.root();
    let staging = staging_directory.root();
    let read = root.read(Path::new(RECORD), LIMITS).unwrap();
    directory.write(b"other");
    assert!(matches!(
        read.replace_in(b"proposed", LIMITS, &staging),
        Err(RecordFileError::ContentsChanged { .. })
    ));
    assert!(matches!(
        root.write_new_in(Path::new(RECORD), b"proposed", LIMITS, &staging),
        Err(RecordFileError::DestinationExists { .. })
    ));
    directory.assert_bytes(b"other");
    directory.assert_no_stage();
    staging_directory.assert_no_stage();
}

#[cfg(unix)]
#[test]
fn supplied_staging_root_preserves_destination_executable_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let directory = TestDirectory::new();
    let staging_directory = TestDirectory::new();
    directory.write(b"first");
    let path = directory.0.join(RECORD);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o751)).unwrap();
    let root = directory.root();
    root.read(Path::new(RECORD), LIMITS)
        .unwrap()
        .replace_in(b"proposed", LIMITS, &staging_directory.root())
        .unwrap();
    directory.assert_bytes(b"proposed");
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o7777,
        0o751
    );
    directory.assert_no_stage();
    staging_directory.assert_no_stage();
}
