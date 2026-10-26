//! Retained execution code and the portable publication boundary.
//!
//! Both products lend the same immutable Terminal module to the interpreter.
//! Invocation code has no byte-export operation or artifact view; only portable
//! artifacts can enter native production or external transport.

use crate::{CanonicalTerminalArtifact, CanonicalTerminalArtifactError, TerminalArtifactView};
use terminal_psi::{TerminalModule, TerminalPsiIdentity};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TerminalPublication {
    #[default]
    Portable,
    Invocation,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalExecutionView<'code> {
    module: &'code TerminalModule,
    identity: TerminalPsiIdentity,
}

impl<'code> TerminalExecutionView<'code> {
    pub fn custody_identity(self) -> TerminalExecutionIdentity {
        TerminalExecutionIdentity {
            semantic: self.identity,
            invocation: self.module.identities.invocation_scope().cloned(),
        }
    }
    pub const fn module(self) -> &'code TerminalModule {
        self.module
    }
    pub const fn identity(self) -> TerminalPsiIdentity {
        self.identity
    }
}

/// Portable bytes may rejoin by their exact semantic identity. Private source
/// companions additionally require the same live code owner, even for equal code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExecutionIdentity {
    semantic: TerminalPsiIdentity,
    invocation: Option<terminal_psi::InvocationCodeScope>,
}

impl<'code> From<TerminalArtifactView<'code>> for TerminalExecutionView<'code> {
    fn from(artifact: TerminalArtifactView<'code>) -> Self {
        Self {
            module: artifact.module(),
            identity: artifact.identity(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InvocationTerminalCode {
    module: TerminalModule,
    identity: TerminalPsiIdentity,
}

impl InvocationTerminalCode {
    /// Retain a producer's checked semantics. Like artifact encoding, this is
    /// not source checking or a grant to install host services.
    pub fn new(module: TerminalModule) -> Result<Self, crate::CodecError> {
        if module.identities.invocation_scope().is_none() {
            return Err(crate::CodecError::MalformedStructuralFoundation(
                "invocation code requires a retained local-code owner",
            ));
        }
        let identity = crate::terminal_psi_identity(&module)?;
        Ok(Self { module, identity })
    }

    pub fn view(&self) -> TerminalExecutionView<'_> {
        TerminalExecutionView {
            module: &self.module,
            identity: self.identity,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TerminalProgramProduct {
    Portable(CanonicalTerminalArtifact),
    Invocation(InvocationTerminalCode),
}

impl TerminalProgramProduct {
    pub fn view(&self) -> TerminalExecutionView<'_> {
        match self {
            Self::Portable(artifact) => artifact.view().into(),
            Self::Invocation(code) => code.view(),
        }
    }

    pub fn portable(&self) -> Result<&CanonicalTerminalArtifact, CanonicalTerminalArtifactError> {
        match self {
            Self::Portable(artifact) => Ok(artifact),
            Self::Invocation(_) => Err(private_publication_error()),
        }
    }

    pub fn into_portable(
        self,
    ) -> Result<CanonicalTerminalArtifact, CanonicalTerminalArtifactError> {
        match self {
            Self::Portable(artifact) => Ok(artifact),
            Self::Invocation(_) => Err(private_publication_error()),
        }
    }
}

fn private_publication_error() -> CanonicalTerminalArtifactError {
    CanonicalTerminalArtifactError::Semantic(crate::CodecError::MalformedStructuralFoundation(
        "invocation code is not a portable Terminal artifact",
    ))
}
