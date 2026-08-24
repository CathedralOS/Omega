use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use psi_terminal_codec::{
    TerminalSemanticArtifactPublication, TerminalSemanticPublicationError, decode_module,
};

static TEST_NONCE: AtomicU64 = AtomicU64::new(0);

fn canonical_bytes() -> Vec<u8> {
    let hex = include_str!(
        "../../../../../../../bootstrap/omega-bootstrap/gates/fixtures/omega-bootstrap-terminal-v28.hex"
    );
    let compact: String = hex
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII fixture"), 16)
                .expect("hex fixture")
        })
        .collect()
}

fn test_directory(label: &str) -> TestDirectory {
    let nonce = TEST_NONCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "psi-terminal-publication-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("create isolated test directory");
    TestDirectory(path)
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn staging_paths(directory: &TestDirectory) -> Vec<PathBuf> {
    fs::read_dir(&directory.0)
        .expect("read test directory")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".terminal-semantic-stage."))
        })
        .collect()
}

#[cfg(unix)]
#[test]
fn destination_changes_only_at_the_validated_rename_boundary() {
    let directory = test_directory("atomic");
    let destination = directory.join("module.psi");
    let old = b"previous complete artifact";
    fs::write(&destination, old).unwrap();

    let bytes = canonical_bytes();
    let publication = TerminalSemanticArtifactPublication::begin(&destination).unwrap();
    let mut output = publication.producer_output().unwrap();
    let split = bytes.len() / 2;
    output.write_all(&bytes[..split]).unwrap();
    output.flush().unwrap();
    assert_eq!(fs::read(&destination).unwrap(), old);
    output.write_all(&bytes[split..]).unwrap();
    drop(output);
    assert_eq!(fs::read(&destination).unwrap(), old);

    let receipt = publication.publish(None).unwrap();
    assert_eq!(receipt.byte_len, bytes.len() as u64);
    assert_eq!(fs::read(&destination).unwrap(), bytes);
    assert!(staging_paths(&directory).is_empty());
}

#[test]
fn truncation_rejects_and_preserves_the_published_destination() {
    let directory = test_directory("truncated");
    let destination = directory.join("module.psi");
    let old = b"previous complete artifact";
    fs::write(&destination, old).unwrap();

    let bytes = canonical_bytes();
    let publication = TerminalSemanticArtifactPublication::begin(&destination).unwrap();
    publication
        .producer_output()
        .unwrap()
        .write_all(&bytes[..bytes.len() - 1])
        .unwrap();

    assert!(matches!(
        publication.publish(None),
        Err(TerminalSemanticPublicationError::Decode(_))
    ));
    assert_eq!(fs::read(&destination).unwrap(), old);
    assert!(staging_paths(&directory).is_empty());
}

#[test]
fn malformed_tampering_rejects_before_publication() {
    let directory = test_directory("malformed-tamper");
    let destination = directory.join("module.psi");
    let old = b"previous complete artifact";
    fs::write(&destination, old).unwrap();

    let mut bytes = canonical_bytes();
    bytes[0] ^= 0x80;
    let publication = TerminalSemanticArtifactPublication::begin(&destination).unwrap();
    publication
        .producer_output()
        .unwrap()
        .write_all(&bytes)
        .unwrap();

    assert!(matches!(
        publication.publish(None),
        Err(TerminalSemanticPublicationError::Decode(_))
    ));
    assert_eq!(fs::read(&destination).unwrap(), old);
    assert!(staging_paths(&directory).is_empty());
}

#[test]
fn valid_but_substituted_terminal_meaning_rejects_when_expected_is_bound() {
    let directory = test_directory("valid-tamper");
    let destination = directory.join("module.psi");
    let old = b"previous complete artifact";
    fs::write(&destination, old).unwrap();

    let expected = canonical_bytes();
    let mut substituted = expected.clone();
    let literal = substituted
        .windows(b"Hello, Omega.".len())
        .position(|window| window == b"Hello, Omega.")
        .expect("fixture retains the exact O0 literal");
    substituted[literal] = b'J';
    decode_module(&substituted).expect("the substituted literal remains canonical terminal Psi");

    let publication = TerminalSemanticArtifactPublication::begin(&destination).unwrap();
    publication
        .producer_output()
        .unwrap()
        .write_all(&substituted)
        .unwrap();
    assert!(matches!(
        publication.publish(Some(&expected)),
        Err(TerminalSemanticPublicationError::UnexpectedArtifact { .. })
    ));
    assert_eq!(fs::read(&destination).unwrap(), old);
    assert!(staging_paths(&directory).is_empty());
}

#[cfg(unix)]
#[test]
fn producer_failure_is_not_artifact_acceptance_even_for_valid_bytes() {
    let directory = test_directory("producer-failure");
    let destination = directory.join("module.psi");
    let producer_bytes = directory.join("producer.psi");
    let old = b"previous complete artifact";
    fs::write(&destination, old).unwrap();
    fs::write(&producer_bytes, canonical_bytes()).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_psi-terminal-publish"))
        .arg(&destination)
        .arg("--")
        .arg("sh")
        .arg("-c")
        .arg("cat \"$1\"; exit 9")
        .arg("producer")
        .arg(&producer_bytes)
        .status()
        .expect("run publication tool");

    assert!(!status.success());
    assert_eq!(fs::read(&destination).unwrap(), old);
    assert!(staging_paths(&directory).is_empty());
}

#[cfg(unix)]
#[test]
fn tool_accepts_the_declared_nonzero_success_status_and_expected_artifact() {
    let directory = test_directory("tool-success");
    let destination = directory.join("module.psi");
    let producer_bytes = directory.join("producer.psi");
    let bytes = canonical_bytes();
    fs::write(&producer_bytes, &bytes).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_psi-terminal-publish"))
        .args(["--success-exit", "107", "--expect"])
        .arg(&producer_bytes)
        .arg(&destination)
        .arg("--")
        .arg("sh")
        .arg("-c")
        .arg("cat \"$1\"; exit 107")
        .arg("producer")
        .arg(&producer_bytes)
        .status()
        .expect("run publication tool");

    assert!(status.success());
    assert_eq!(fs::read(&destination).unwrap(), bytes);
    assert!(staging_paths(&directory).is_empty());
}
