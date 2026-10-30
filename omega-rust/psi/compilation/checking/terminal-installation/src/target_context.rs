//! One immutable Terminal artifact associated with an invocation target.
//!
//! The caller supplies the exact target identity from its admitted catalog.
//! This association selects no source body, provider, authority or semantics;
//! those inputs belong to the ordinary provider and layout owners.

use semantic_vocabulary::MachineId;
use terminal_codec::TerminalArtifactView;
use terminal_psi::{TerminalCallable, TerminalPsiIdentity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetContext {
    terminal_psi: TerminalPsiIdentity,
    target: String,
}

impl TerminalTargetContext {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    /// Entry selection must retain an actual checked machine in this artifact.
    /// Opaque boundary declarations supply no executable entry by themselves.
    pub fn entry(
        &self,
        identity: TerminalPsiIdentity,
        module: &terminal_psi::TerminalModule,
        callable: TerminalCallable,
    ) -> Result<MachineId, TerminalEntryError> {
        if self.terminal_psi != identity {
            return Err(TerminalEntryError::TerminalIdentityMismatch);
        }
        match callable {
            TerminalCallable::Machine(machine) if module.machine(machine).is_some() => Ok(machine),
            _ => Err(TerminalEntryError::NonExecutableEntry {
                callable,
                target: self.target.clone(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEntryError {
    TerminalIdentityMismatch,
    NonExecutableEntry {
        callable: TerminalCallable,
        target: String,
    },
}

impl std::fmt::Display for TerminalEntryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TerminalIdentityMismatch => {
                formatter.write_str("entry selection names a different Terminal artifact")
            }
            Self::NonExecutableEntry { callable, target } => write!(
                formatter,
                "callable {callable:?} has no checked executable entry for target `{target}`"
            ),
        }
    }
}
impl std::error::Error for TerminalEntryError {}

pub fn target_context(artifact: TerminalArtifactView<'_>, target: &str) -> TerminalTargetContext {
    TerminalTargetContext {
        terminal_psi: artifact.identity(),
        target: target.to_owned(),
    }
}
