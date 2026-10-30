//! Read-only replay inputs shared by retained native product classes.
//!
//! The portable artifact is borrowed from its actual native owner. Its native
//! identity retains the product class; this projection neither converts native
//! custody nor supplies publication or receiving-installation authority.

use native_artifact::{DynamicElfNativeArtifactIdentity, NativeArtifactIdentity};
use terminal_codec::CanonicalTerminalArtifact;

/// Exact native identity, preserving the distinct dynamic-ELF identity domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedNativeProgramIdentity {
    Direct(NativeArtifactIdentity),
    DynamicElf(DynamicElfNativeArtifactIdentity),
}

impl std::fmt::Display for RetainedNativeProgramIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct(identity) => std::fmt::Display::fmt(identity, formatter),
            Self::DynamicElf(identity) => std::fmt::Display::fmt(identity, formatter),
        }
    }
}

/// Borrowed portable program and its exact native owner identity before publication.
pub struct RetainedNativeProgram<'artifact> {
    pub(crate) artifact: &'artifact CanonicalTerminalArtifact,
    pub(crate) identity: RetainedNativeProgramIdentity,
}

impl<'artifact> RetainedNativeProgram<'artifact> {
    pub fn psi_artifact(&self) -> &'artifact CanonicalTerminalArtifact {
        self.artifact
    }

    pub fn identity(&self) -> RetainedNativeProgramIdentity {
        self.identity
    }
}
