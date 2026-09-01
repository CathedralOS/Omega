//! Command-boundary failures with stable exit classification.

use std::{io, path::PathBuf, process::ExitCode};

use omega_optimization_policy_offline::{OfflinePolicyCorpusError, OfflinePolicyReferenceError};

use crate::arguments::USAGE;

#[derive(Debug)]
pub(super) enum OfflinePolicyCommandError {
    Usage(&'static str),
    ReadLog { path: PathBuf, source: io::Error },
    ReadCorpus { path: PathBuf, source: io::Error },
    ReadModel { path: PathBuf, source: io::Error },
    InvalidCorpus(OfflinePolicyCorpusError),
    InvalidReferenceArtifact(OfflinePolicyReferenceError),
    Publish { path: PathBuf, source: io::Error },
}

impl OfflinePolicyCommandError {
    pub(super) fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage(_) => ExitCode::from(2),
            Self::ReadLog { .. }
            | Self::ReadCorpus { .. }
            | Self::ReadModel { .. }
            | Self::InvalidCorpus(_)
            | Self::InvalidReferenceArtifact(_)
            | Self::Publish { .. } => ExitCode::FAILURE,
        }
    }
}

impl std::fmt::Display for OfflinePolicyCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => write!(formatter, "{message}\n{USAGE}"),
            Self::ReadLog { path, source } => {
                write!(
                    formatter,
                    "could not read decision log {}: {source}",
                    path.display()
                )
            }
            Self::ReadCorpus { path, source } => write!(
                formatter,
                "could not read offline policy corpus {}: {source}",
                path.display()
            ),
            Self::ReadModel { path, source } => write!(
                formatter,
                "could not read offline policy model {}: {source}",
                path.display()
            ),
            Self::InvalidCorpus(source) => source.fmt(formatter),
            Self::InvalidReferenceArtifact(source) => source.fmt(formatter),
            Self::Publish { path, source } => write!(
                formatter,
                "could not publish offline policy artifact {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for OfflinePolicyCommandError {}
