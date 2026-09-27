use super::{read_capability_file_bounded, read_opened_file_bounded};
use crate::SourceResolveError;
use crate::test_support::temp_root;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, File, Metadata, OpenOptions};
use std::ffi::OsStr;
use std::fs::FileTimes;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

struct SourceFixture {
    root: PathBuf,
    directory: Dir,
}

impl SourceFixture {
    fn new(bytes: &[u8]) -> Self {
        let root = temp_root("bounded-file-read");
        std::fs::create_dir(&root).expect("create source fixture");
        std::fs::write(root.join("source.bin"), bytes).expect("write source bytes");
        let directory =
            Dir::open_ambient_dir(&root, ambient_authority()).expect("retain source root");
        Self { root, directory }
    }

    fn open(&self) -> (File, Metadata) {
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let file = self
            .directory
            .open_with("source.bin", &options)
            .expect("open source without following links");
        let metadata = file.metadata().expect("observe opened source");
        (file, metadata)
    }

    fn replace_bytes(&self, bytes: &[u8], initial: &Metadata) {
        let mut writer = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(self.root.join("source.bin"))
            .expect("retain source writer");
        writer.write_all(bytes).expect("replace bytes in same file");
        // No clock sleeps: make the changed metadata distinguishable even on
        // a filesystem whose automatic modification times have coarse resolution.
        writer
            .set_times(
                FileTimes::new().set_modified(
                    initial
                        .modified()
                        .expect("initial modification time")
                        .into_std()
                        + Duration::from_secs(2),
                ),
            )
            .expect("advance modification time");
    }
}

impl Drop for SourceFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn assert_change_rejects(original: &[u8], replacement: &[u8]) {
    let fixture = SourceFixture::new(original);
    let (mut file, initial) = fixture.open();
    fixture.replace_bytes(replacement, &initial);
    let path = fixture.root.join("source.bin");
    let error = read_opened_file_bounded(&mut file, &initial, &path, 1024, 1024)
        .expect_err("changed input must not become a captured source file");
    assert!(
        matches!(error, SourceResolveError::LocalSourceChanged { path: observed } if observed == path)
    );
}

#[test]
fn bounded_copy_rejects_growth_within_the_remaining_budget() {
    assert_change_rejects(b"short", b"longer but still within budget");
}

#[test]
fn bounded_copy_keeps_budget_exhaustion_distinct_from_source_drift() {
    let fixture = SourceFixture::new(b"small");
    let (mut file, initial) = fixture.open();
    let path = fixture.root.join("source.bin");
    assert!(matches!(
        read_opened_file_bounded(&mut file, &initial, &path, 4, 100),
        Err(SourceResolveError::TooManyBytes { limit: 100 })
    ));
    fixture.replace_bytes(b"larger than budget", &initial);
    assert!(matches!(
        read_opened_file_bounded(&mut file, &initial, &path, 10, 100),
        Err(SourceResolveError::TooManyBytes { limit: 100 })
    ));
}

#[cfg(unix)]
#[test]
fn bounded_copy_rejects_executable_mode_changes_and_preserves_stable_modes() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = SourceFixture::new(b"bytes");
    let path = fixture.root.join("source.bin");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let (mut file, initial) = fixture.open();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(
        read_opened_file_bounded(&mut file, &initial, &path, 100, 100),
        Err(SourceResolveError::LocalSourceChanged { .. })
    ));
    for (mode, executable) in [(0o600, false), (0o700, true)] {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        assert_eq!(
            read_capability_file_bounded(
                &fixture.directory,
                OsStr::new("source.bin"),
                &path,
                100,
                100,
            )
            .expect("capture stable mode"),
            (b"bytes".to_vec(), executable)
        );
    }
}

/// The link-retarget, in-place-restore, and mode-change rejection legs above
/// depend on unix symlink spelling, kernel change time, and permission bits;
/// none exist on this host. Record the absent coverage explicitly rather
/// than letting a non-unix run drop it silently.
#[cfg(not(unix))]
#[test]
fn unix_mutable_copy_drift_legs_recorded_as_unavailable_on_this_host() {
    eprintln!(
        "SKIP: link-retarget/in-place-restore/mode-change rejection coverage \
         requires a unix host (symlink spelling and kernel change time)"
    );
}
