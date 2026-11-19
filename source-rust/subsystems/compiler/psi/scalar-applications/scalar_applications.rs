//! Checked scalar application demand for published Psi.
#![forbid(unsafe_code)]

use semantic_vocabulary::{CallingAbiIdentity, ScalarApplicationId};
use terminal_codec::TerminalArtifactView;

/// The storage extents a specific ABI declaration needs in one published program and target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingAbiStorageDemand {
    terminal: terminal_psi::TerminalPsiIdentity,
    target: String,
    declaration: CallingAbiIdentity,
    applications: Vec<ScalarApplicationId>,
}

impl CallingAbiStorageDemand {
    pub fn new(
        artifact: TerminalArtifactView<'_>,
        target: &str,
        declaration: &CallingAbiIdentity,
    ) -> Result<Self, String> {
        let module = artifact.module();
        let retained = module
            .calling_abi_signature(declaration.id())
            .ok_or("ABI scalar demand has no canonical declaration")?;
        if retained.identity(&module.identities)? != *declaration {
            return Err("ABI scalar demand differs from its complete declaration key".into());
        }
        let applications =
            terminal_semantics::calling_abi_extent_dependencies(module, retained.id, target)?;
        Ok(Self {
            terminal: artifact.identity(),
            target: target.to_owned(),
            declaration: declaration.clone(),
            applications,
        })
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn declaration(&self) -> &CallingAbiIdentity {
        &self.declaration
    }

    pub fn applications(&self) -> &[ScalarApplicationId] {
        &self.applications
    }

    pub fn require_artifact<'artifact>(
        &self,
        artifact: impl Into<terminal_codec::TerminalExecutionView<'artifact>>,
    ) -> Result<(), String> {
        if self.terminal != artifact.into().identity() {
            return Err("ABI storage demand belongs to another Terminal artifact".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod abi_storage_tests;
