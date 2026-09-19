use super::file_read::read_canonical_file_chunks;
use super::{hash_canonical_source_file, read_canonical_source_file};
use sha2::{Digest, Sha256};
use std::fs::{File, FileTimes};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

struct SourceFile {
    path: PathBuf,
    writer: File,
}

impl SourceFile {
    fn new(bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-source-read-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut writer = File::create_new(&path).expect("create source fixture");
        writer.write_all(bytes).expect("write source fixture");
        Self { path, writer }
    }

    fn replace_bytes(&mut self, bytes: &[u8], initial: &std::fs::Metadata) {
        self.writer.seek(SeekFrom::Start(0)).expect("rewind writer");
        self.writer.write_all(bytes).expect("replace source bytes");
        // Force a distinguishable time without sleeps or assumptions about
        // the host clock's resolution. The file's size and identity stay put.
        self.writer
            .set_times(FileTimes::new().set_modified(
                initial.modified().expect("initial modification time") + Duration::from_secs(2),
            ))
            .expect("advance source modification time");
    }
}

impl Drop for SourceFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[test]
fn retained_bytes_reject_same_length_change_since_inventory_inspection() {
    let mut source = SourceFile::new(b"before");
    let initial = std::fs::symlink_metadata(&source.path).expect("initial source metadata");
    source.replace_bytes(b"after!", &initial);

    let error = read_canonical_source_file(&source.path, &initial)
        .expect_err("changed bytes cannot inherit an earlier inventory observation");
    assert!(error.contains("changed"), "{error}");
}

#[test]
fn content_hash_rejects_same_length_change_since_inventory_inspection() {
    let mut source = SourceFile::new(b"before");
    let initial = std::fs::symlink_metadata(&source.path).expect("initial source metadata");
    source.replace_bytes(b"after!", &initial);

    let error = hash_canonical_source_file(&source.path, &initial)
        .expect_err("changed bytes cannot inherit an earlier inventory observation");
    assert!(error.contains("changed"), "{error}");
}

#[test]
fn file_read_rejects_same_length_mutation_between_chunks() {
    let original = vec![b'A'; 128 * 1024];
    let replacement = vec![b'B'; original.len()];
    let mut source = SourceFile::new(&original);
    let initial = std::fs::symlink_metadata(&source.path).expect("initial source metadata");
    let path = source.path.clone();
    let mut consumed = Vec::new();
    let mut mutated = false;
    let error = read_canonical_file_chunks(&path, &initial, |chunk| {
        consumed.extend_from_slice(chunk);
        if !mutated {
            source.replace_bytes(&replacement, &initial);
            mutated = true;
        }
    })
    .expect_err("mixed reads from a changed file cannot become a snapshot or content hash");
    assert!(mutated);
    assert_eq!(consumed.len(), original.len());
    assert_ne!(consumed, original);
    assert_ne!(consumed, replacement);
    assert!(error.contains("changed"), "{error}");
}

#[test]
fn file_read_rejects_growth_between_chunks() {
    let mut source = SourceFile::new(&vec![b'A'; 128 * 1024]);
    let initial = std::fs::symlink_metadata(&source.path).expect("initial source metadata");
    let path = source.path.clone();
    let mut mutated = false;
    let error = read_canonical_file_chunks(&path, &initial, |_| {
        if !mutated {
            source
                .writer
                .seek(SeekFrom::End(0))
                .expect("seek past input");
            source.writer.write_all(b"extra").expect("grow input");
            mutated = true;
        }
    })
    .expect_err("growth cannot bypass the admitted content bound");
    assert!(error.contains("grew"), "{error}");
}

#[test]
fn file_read_rejects_truncation_between_chunks() {
    let source = SourceFile::new(&vec![b'A'; 128 * 1024]);
    let initial = std::fs::symlink_metadata(&source.path).expect("initial source metadata");
    let error = read_canonical_file_chunks(&source.path, &initial, |_| {
        source.writer.set_len(0).expect("truncate input");
    })
    .expect_err("short reads cannot become a smaller successful capture");
    assert!(error.contains("changed length"), "{error}");
}

#[cfg(unix)]
#[test]
fn file_read_rejects_replacement_with_matching_bytes_length_and_modified_time() {
    let source = SourceFile::new(b"same bytes");
    let initial = std::fs::symlink_metadata(&source.path).expect("initial source metadata");
    std::fs::remove_file(&source.path).expect("unlink original but retain its open handle");
    let mut replacement = File::create_new(&source.path).expect("create replacement inode");
    replacement
        .write_all(b"same bytes")
        .expect("write same bytes");
    replacement
        .set_times(FileTimes::new().set_modified(initial.modified().unwrap()))
        .expect("preserve modification time");

    assert!(read_canonical_source_file(&source.path, &initial).is_err());
    assert!(hash_canonical_source_file(&source.path, &initial).is_err());
}

#[test]
fn unchanged_empty_and_multichunk_files_keep_exact_bytes_and_hashes() {
    for bytes in [Vec::new(), vec![b'A'; 128 * 1024 + 17]] {
        let source = SourceFile::new(&bytes);
        let initial = std::fs::symlink_metadata(&source.path).expect("initial source metadata");
        assert_eq!(
            read_canonical_source_file(&source.path, &initial).unwrap(),
            bytes
        );
        let expected: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(
            hash_canonical_source_file(&source.path, &initial).unwrap(),
            expected
        );

        source
            .writer
            .set_times(
                FileTimes::new().set_modified(initial.modified().unwrap() + Duration::from_secs(2)),
            )
            .expect("change only host metadata");
        let refreshed = std::fs::symlink_metadata(&source.path).expect("refreshed metadata");
        assert_eq!(
            hash_canonical_source_file(&source.path, &refreshed).unwrap(),
            expected,
            "host timestamps guard reads but are not canonical content evidence"
        );
    }
}

#[cfg(unix)]
#[test]
fn file_read_rejects_path_replacement_while_original_handle_is_open() {
    let bytes = vec![b'A'; 128 * 1024];
    let source = SourceFile::new(&bytes);
    let initial = std::fs::symlink_metadata(&source.path).expect("initial source metadata");
    let mut replaced = false;
    let mut consumed = Vec::new();
    let error = read_canonical_file_chunks(&source.path, &initial, |chunk| {
        consumed.extend_from_slice(chunk);
        if !replaced {
            std::fs::remove_file(&source.path).expect("unlink original during capture");
            let mut replacement = File::create_new(&source.path).expect("replace source path");
            replacement
                .write_all(&bytes)
                .expect("retain matching bytes");
            replacement
                .set_times(FileTimes::new().set_modified(initial.modified().unwrap()))
                .expect("retain matching modification time");
            replaced = true;
        }
    })
    .expect_err("a completed read cannot validate a substituted path");
    assert!(replaced);
    assert_eq!(consumed, bytes, "reads retain the originally opened handle");
    assert!(error.contains("changed"), "{error}");
}
