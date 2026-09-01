//! Command-boundary failures with stable exit classification.

use std::{io, path::PathBuf, process::ExitCode};

use omega_optimization_policy_offline::OfflinePolicyCorpusError;

use crate::arguments::USAGE;

#[derive(Debug)]
pub(super) enum OfflinePolicyCommandError {
    Usage(&'static str),
    ReadLog { path: PathBuf, source: io::Error },
    InvalidCorpus(OfflinePolicyCorpusError),
    Publish { path: PathBuf, source: io::Error },
}

impl OfflinePolicyCommandError {
    pub(super) fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage(_) => ExitCode::from(2),
            Self::ReadLog { .. } | Self::InvalidCorpus(_) | Self::Publish { .. } => {
                ExitCode::FAILURE
            }
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
            Self::InvalidCorpus(source) => source.fmt(formatter),
            Self::Publish { path, source } => write!(
                formatter,
                "could not publish corpus artifact {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for OfflinePolicyCommandError {}
