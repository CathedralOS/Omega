//! Fail-closed publication of canonical terminal-Psi semantic artifacts.
//!
//! Producers write only to a same-directory staging file. Publication becomes
//! visible at the destination only after the staging file has been persisted
//! and accepted by the canonical decoder. This keeps physical output failures
//! outside the source language's boundary-operation return convention.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use terminal_psi::TerminalPsiIdentity;

use crate::{CodecError, decode_module, terminal_psi_identity};

static STAGING_NONCE: AtomicU64 = AtomicU64::new(0);
const MAX_STAGING_ATTEMPTS: u64 = 1_024;

#[derive(Debug)]
pub struct TerminalSemanticArtifactPublication {
    destination: PathBuf,
    staging_path: PathBuf,
    staging_file: Option<File>,
    renamed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedTerminalSemanticArtifact {
    pub path: PathBuf,
    pub byte_len: u64,
    pub identity: TerminalPsiIdentity,
}

#[derive(Debug)]
pub enum TerminalSemanticPublicationError {
    InvalidDestination,
    StagingNameExhausted,
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Decode(CodecError),
    ExpectedArtifactDecode(CodecError),
    UnexpectedArtifact {
        expected: TerminalPsiIdentity,
        actual: TerminalPsiIdentity,
    },
}

impl TerminalSemanticArtifactPublication {
    /// Create a private staging file beside `destination`.
    ///
    /// Keeping both paths in one directory makes the final rename a same-file-
    /// system atomic visibility boundary. The destination is not touched until
    /// [`publish`](Self::publish) succeeds.
    pub fn begin(destination: impl AsRef<Path>) -> Result<Self, TerminalSemanticPublicationError> {
        let destination = destination.as_ref().to_path_buf();
        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        let file_name = destination
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or(TerminalSemanticPublicationError::InvalidDestination)?;

        for _ in 0..MAX_STAGING_ATTEMPTS {
            let nonce = STAGING_NONCE.fetch_add(1, Ordering::Relaxed);
            let mut staging_name = OsString::from(".");
            staging_name.push(file_name);
            staging_name.push(format!(
                ".terminal-semantic-stage.{}.{nonce}",
                std::process::id()
            ));
            let staging_path = parent.join(staging_name);
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&staging_path)
            {
                Ok(staging_file) => {
                    return Ok(Self {
                        destination,
                        staging_path,
                        staging_file: Some(staging_file),
                        renamed: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(TerminalSemanticPublicationError::Io {
                        action: "create staging file",
                        path: staging_path,
                        source,
                    });
                }
            }
        }

        Err(TerminalSemanticPublicationError::StagingNameExhausted)
    }

    /// Clone the physical staging-file handle for use as a producer's stdout.
    pub fn producer_output(&self) -> Result<File, TerminalSemanticPublicationError> {
        self.staging_file
            .as_ref()
            .expect("an unpublished staging file retains its handle")
            .try_clone()
            .map_err(|source| TerminalSemanticPublicationError::Io {
                action: "clone staging output",
                path: self.staging_path.clone(),
                source,
            })
    }

    pub fn staging_path(&self) -> &Path {
        &self.staging_path
    }

    /// Canonically validate and atomically publish the staged bytes.
    ///
    /// When `expected_bytes` is present, it must itself be canonical and its
    /// terminal semantic identity must equal the staged artifact's identity.
    /// This optional binding catches a valid-but-substituted artifact in gates
    /// that already know the expected terminal meaning.
    pub fn publish(
        mut self,
        expected_bytes: Option<&[u8]>,
    ) -> Result<PublishedTerminalSemanticArtifact, TerminalSemanticPublicationError> {
        let staging_file = self
            .staging_file
            .take()
            .expect("an unpublished staging file retains its handle");
        staging_file
            .sync_all()
            .map_err(|source| TerminalSemanticPublicationError::Io {
                action: "persist staging file",
                path: self.staging_path.clone(),
                source,
            })?;
        drop(staging_file);

        let staged_bytes = fs::read(&self.staging_path).map_err(|source| {
            TerminalSemanticPublicationError::Io {
                action: "read staged artifact",
                path: self.staging_path.clone(),
                source,
            }
        })?;
        let module =
            decode_module(&staged_bytes).map_err(TerminalSemanticPublicationError::Decode)?;
        let identity =
            terminal_psi_identity(&module).map_err(TerminalSemanticPublicationError::Decode)?;

        if let Some(expected_bytes) = expected_bytes {
            let expected_module = decode_module(expected_bytes)
                .map_err(TerminalSemanticPublicationError::ExpectedArtifactDecode)?;
            let expected = terminal_psi_identity(&expected_module)
                .map_err(TerminalSemanticPublicationError::ExpectedArtifactDecode)?;
            if identity != expected {
                return Err(TerminalSemanticPublicationError::UnexpectedArtifact {
                    expected,
                    actual: identity,
                });
            }
        }

        fs::rename(&self.staging_path, &self.destination).map_err(|source| {
            TerminalSemanticPublicationError::Io {
                action: "atomically publish staged artifact",
                path: self.destination.clone(),
                source,
            }
        })?;
        self.renamed = true;
        sync_parent_directory(&self.destination)?;

        Ok(PublishedTerminalSemanticArtifact {
            path: self.destination.clone(),
            byte_len: u64::try_from(staged_bytes.len())
                .expect("an in-memory artifact length fits u64"),
            identity,
        })
    }
}

impl Drop for TerminalSemanticArtifactPublication {
    fn drop(&mut self) {
        if !self.renamed {
            let _ = fs::remove_file(&self.staging_path);
        }
    }
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<(), TerminalSemanticPublicationError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| TerminalSemanticPublicationError::Io {
            action: "persist publication directory",
            path: parent.to_path_buf(),
            source,
        })
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), TerminalSemanticPublicationError> {
    Ok(())
}

impl std::fmt::Display for TerminalSemanticPublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDestination => {
                write!(formatter, "publication destination has no file name")
            }
            Self::StagingNameExhausted => {
                write!(formatter, "could not reserve a unique staging file")
            }
            Self::Io {
                action,
                path,
                source,
            } => write!(formatter, "failed to {action} {}: {source}", path.display()),
            Self::Decode(error) => write!(
                formatter,
                "staged artifact is not canonical terminal Psi: {error}"
            ),
            Self::ExpectedArtifactDecode(error) => write!(
                formatter,
                "expected artifact is not canonical terminal Psi: {error}"
            ),
            Self::UnexpectedArtifact { expected, actual } => write!(
                formatter,
                "staged terminal identity {}:{} does not match expected {}:{}",
                actual.vocabulary_marker.get(),
                actual.program_fingerprint,
                expected.vocabulary_marker.get(),
                expected.program_fingerprint
            ),
        }
    }
}

impl std::error::Error for TerminalSemanticPublicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Decode(error) | Self::ExpectedArtifactDecode(error) => Some(error),
            Self::InvalidDestination
            | Self::StagingNameExhausted
            | Self::UnexpectedArtifact { .. } => None,
        }
    }
}
