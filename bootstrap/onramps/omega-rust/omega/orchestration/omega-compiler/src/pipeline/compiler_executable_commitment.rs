use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

const COMPILER_EXECUTABLE_COMMITMENT_DOMAIN: &[u8] = b"OMEGA-COMPILER-EXECUTABLE-COMMITMENT\0";

/// Compiler-owned commitment to the exact bytes read from a compiler
/// executable path.
///
/// This identifies an observed executable artifact. It does not certify that
/// artifact, identify its source closure, or prove that the observed bytes are
/// the process image currently loaded by the operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompilerExecutableCommitment {
    digest: [u8; 32],
}

impl CompilerExecutableCommitment {
    /// Derives a commitment from the exact bytes currently readable at `path`.
    pub(crate) fn derive_from_path(
        path: impl AsRef<Path>,
    ) -> Result<Self, CompilerExecutableCommitmentError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| {
            CompilerExecutableCommitmentError::ReadExecutable {
                path: path.to_path_buf(),
                source,
            }
        })?;
        Ok(Self::derive_from_bytes(&bytes))
    }

    /// Derives a commitment from the exact bytes currently readable at the
    /// path reported for this process by [`std::env::current_exe`].
    pub fn derive_current() -> Result<Self, CompilerExecutableCommitmentError> {
        let path = std::env::current_exe().map_err(|source| {
            CompilerExecutableCommitmentError::LocateCurrentExecutable { source }
        })?;
        Self::derive_from_path(path)
    }

    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    fn derive_from_bytes(bytes: &[u8]) -> Self {
        let mut digest = Sha256::new();
        digest.update(COMPILER_EXECUTABLE_COMMITMENT_DOMAIN);
        digest.update(
            u64::try_from(bytes.len())
                .expect("compiler executable byte length fits u64")
                .to_le_bytes(),
        );
        digest.update(bytes);
        Self {
            digest: digest.finalize().into(),
        }
    }
}

/// Failure to locate or read the executable artifact being committed.
#[derive(Debug)]
pub enum CompilerExecutableCommitmentError {
    LocateCurrentExecutable { source: io::Error },
    ReadExecutable { path: PathBuf, source: io::Error },
}

impl fmt::Display for CompilerExecutableCommitmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocateCurrentExecutable { source } => {
                write!(
                    formatter,
                    "cannot locate the current compiler executable: {source}"
                )
            }
            Self::ReadExecutable { path, source } => write!(
                formatter,
                "cannot read compiler executable `{}`: {source}",
                path.display()
            ),
        }
    }
}

impl Error for CompilerExecutableCommitmentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LocateCurrentExecutable { source } | Self::ReadExecutable { source, .. } => {
                Some(source)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CompilerExecutableCommitment, CompilerExecutableCommitmentError};
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "omega-compiler-executable-commitment-{label}-{}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir(&path).expect("create executable commitment test directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.path)
                .expect("remove executable commitment test directory");
        }
    }

    #[test]
    fn identical_bytes_have_the_same_commitment_across_relocation() {
        let first_directory = TestDirectory::new("relocation-first");
        let second_directory = TestDirectory::new("relocation-second");
        let first_path = first_directory.path().join("omega");
        let second_path = second_directory.path().join("renamed-omega");
        std::fs::write(&first_path, b"same compiler bytes").expect("write first executable");
        std::fs::write(&second_path, b"same compiler bytes").expect("write second executable");

        let first = CompilerExecutableCommitment::derive_from_path(first_path)
            .expect("derive first commitment");
        let second = CompilerExecutableCommitment::derive_from_path(second_path)
            .expect("derive second commitment");

        assert_eq!(first, second);
    }

    #[test]
    fn changed_bytes_change_the_commitment() {
        let directory = TestDirectory::new("changed-bytes");
        let path = directory.path().join("omega");
        std::fs::write(&path, b"compiler bytes before").expect("write initial executable");
        let before = CompilerExecutableCommitment::derive_from_path(&path)
            .expect("derive initial commitment");

        std::fs::write(&path, b"compiler bytes after").expect("write changed executable");
        let after = CompilerExecutableCommitment::derive_from_path(path)
            .expect("derive changed commitment");

        assert_ne!(before, after);
    }

    #[test]
    fn missing_path_reports_the_path_and_io_error() {
        let directory = TestDirectory::new("missing-path");
        let path = directory.path().join("missing-omega");

        let error = CompilerExecutableCommitment::derive_from_path(&path)
            .expect_err("missing executable must fail");

        match error {
            CompilerExecutableCommitmentError::ReadExecutable {
                path: reported_path,
                source,
            } => {
                assert_eq!(reported_path, path);
                assert_eq!(source.kind(), io::ErrorKind::NotFound);
            }
            CompilerExecutableCommitmentError::LocateCurrentExecutable { .. } => {
                panic!("explicit path derivation cannot report current-executable discovery")
            }
        }
    }

    #[test]
    fn digest_is_nonzero() {
        let directory = TestDirectory::new("nonzero");
        let path = directory.path().join("omega");
        std::fs::write(&path, b"compiler bytes").expect("write executable");

        let commitment = CompilerExecutableCommitment::derive_from_path(path)
            .expect("derive executable commitment");

        assert_ne!(commitment.digest(), [0; 32]);
    }
}
