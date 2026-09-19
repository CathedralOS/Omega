use super::{
    read_capability_file_bounded, read_opened_file_bounded, require_unchanged_source_file,
};
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
fn bounded_copy_rejects_same_length_change_after_initial_observation() {
    assert_change_rejects(b"before", b"after!");
}

#[test]
fn bounded_copy_rejects_growth_within_the_remaining_budget() {
    assert_change_rejects(b"short", b"longer but still within budget");
}

#[test]
fn bounded_copy_rejects_truncation_after_initial_observation() {
    assert_change_rejects(b"long original", b"short");
}

#[test]
fn bounded_copy_preserves_empty_and_multichunk_files_at_the_exact_budget() {
    for bytes in [Vec::new(), vec![0xa7; 128 * 1024 + 1]] {
        let fixture = SourceFixture::new(&bytes);
        let (captured, _) = read_capability_file_bounded(
            &fixture.directory,
            OsStr::new("source.bin"),
            &fixture.root.join("source.bin"),
            bytes.len() as u64,
            bytes.len() as u64,
        )
        .expect("capture unchanged bytes at exact limit");
        assert_eq!(captured, bytes);
    }
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

#[test]
fn source_identity_excludes_host_change_indicators() {
    let fixture = SourceFixture::new(b"unchanged bytes");
    let before = crate::resolve_local_source(&fixture.root, crate::LocalSourceLimits::default())
        .expect("capture initial source identity");
    let (_file, initial) = fixture.open();
    fixture.replace_bytes(b"unchanged bytes", &initial);
    let after = crate::resolve_local_source(&fixture.root, crate::LocalSourceLimits::default())
        .expect("capture stable source after metadata changed");
    assert_eq!(before.content_identity, after.content_identity);
}

#[test]
fn final_entry_rejects_a_different_file_with_identical_bytes_and_modified_time() {
    let fixture = SourceFixture::new(b"same bytes");
    let (mut file, initial) = fixture.open();
    let path = fixture.root.join("source.bin");
    assert_eq!(
        read_opened_file_bounded(&mut file, &initial, &path, 100, 100).expect("read original file"),
        b"same bytes"
    );
    // Retain the original inode so it cannot be recycled for the replacement.
    std::fs::rename(&path, fixture.root.join("retained.bin")).expect("retain original file");
    let mut replacement = std::fs::File::create(&path).expect("create replacement file");
    replacement.write_all(b"same bytes").expect("copy bytes");
    replacement
        .set_times(FileTimes::new().set_modified(initial.modified().unwrap().into_std()))
        .expect("preserve modification time");
    let final_entry = fixture.directory.symlink_metadata("source.bin").unwrap();
    assert!(matches!(
        require_unchanged_source_file(&initial, &final_entry, &path),
        Err(SourceResolveError::LocalSourceChanged { .. })
    ));
}

#[cfg(unix)]
#[test]
fn final_entry_rejects_a_link_to_the_retained_file() {
    let fixture = SourceFixture::new(b"same bytes");
    let (mut file, initial) = fixture.open();
    let path = fixture.root.join("source.bin");
    read_opened_file_bounded(&mut file, &initial, &path, 100, 100).expect("read original");
    std::fs::rename(&path, fixture.root.join("retained.bin")).expect("retain original file");
    std::os::unix::fs::symlink("retained.bin", &path).expect("replace entry with link");
    let final_entry = fixture.directory.symlink_metadata("source.bin").unwrap();
    assert!(matches!(
        require_unchanged_source_file(&initial, &final_entry, &path),
        Err(SourceResolveError::LocalSourceChanged { .. })
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
