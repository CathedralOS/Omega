use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{ArtifactWriter, temp_path_for};

struct ReportDirectory(PathBuf);

impl ReportDirectory {
    fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "omega-report-writer-{}-{sequence}",
            std::process::id(),
        ));
        fs::create_dir(&path).expect("create unique report directory");
        Self(path)
    }
}

impl Drop for ReportDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn text_and_bytes_replace_the_same_artifact_without_leaving_staging_files() {
    let directory = ReportDirectory::new();
    let writer = ArtifactWriter::new(&directory.0).unwrap();
    let destination = directory.0.join("report.txt");
    writer.write_text("report.txt", "first\n").unwrap();
    assert_eq!(fs::read(&destination).unwrap(), b"first\n");
    assert_eq!(
        writer.write_bytes("report.txt", &[0, 255, 1]).unwrap(),
        destination,
    );
    assert_eq!(fs::read(&destination).unwrap(), [0, 255, 1]);
    writer.write_text("report.txt", "replacement\n").unwrap();
    assert_eq!(fs::read(&destination).unwrap(), b"replacement\n");
    assert!(!temp_path_for(&destination).exists());
}

#[test]
fn failed_installation_preserves_destination_and_removes_staging_file() {
    let directory = ReportDirectory::new();
    let writer = ArtifactWriter::new(&directory.0).unwrap();
    let destination = directory.0.join("occupied");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("keep"), b"existing").unwrap();

    let text_error = writer.write_text("occupied", "replacement").unwrap_err();
    let bytes_error = writer.write_bytes("occupied", b"replacement").unwrap_err();
    assert_eq!(text_error.message, bytes_error.message);
    assert!(text_error.message.contains("failed to install artifact"));
    assert_eq!(fs::read(destination.join("keep")).unwrap(), b"existing");
    assert!(!temp_path_for(&destination).exists());
}
