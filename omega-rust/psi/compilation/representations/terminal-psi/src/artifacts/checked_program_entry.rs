//! Checked-source `ProgramEntry` join receipts retained beside a published
//! Terminal artifact.
//!
//! These are durable companion schemas: the producer derives them from the
//! exact checked machine and canonical Terminal module, and later Omega
//! settlement independently rejoins them against retained authority. Merely
//! constructing or carrying a receipt grants no target, calling convention,
//! runtime root, image, installation, or publication authority.

use semantic_vocabulary::MachineId;

use crate::TerminalPsiIdentity;

/// Exact checked declaration and portable entry join, without automatic
/// storage establishment or receiver eligibility. Installed arguments are
/// admitted independently against their actual invocation backing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProgramEntryTerminalReceipt {
    source_signature_identity: [u8; 32],
    source_machine_name: String,
    declaration: crate::NominalDeclarationIdentity,
    terminal_psi_identity: TerminalPsiIdentity,
    terminal_entry: MachineId,
}

impl CheckedProgramEntryTerminalReceipt {
    pub fn new(
        source_signature_identity: [u8; 32],
        source_machine_name: String,
        declaration: crate::NominalDeclarationIdentity,
        terminal_psi_identity: TerminalPsiIdentity,
        terminal_entry: MachineId,
    ) -> Self {
        Self {
            source_signature_identity,
            source_machine_name,
            declaration,
            terminal_psi_identity,
            terminal_entry,
        }
    }

    pub const fn source_signature_identity(&self) -> [u8; 32] {
        self.source_signature_identity
    }

    pub fn source_machine_name(&self) -> &str {
        &self.source_machine_name
    }

    /// Exact package/toolchain-owned declaration, independent of source arenas.
    /// Display names cannot distinguish same-named declarations across packages.
    pub const fn declaration(&self) -> &crate::NominalDeclarationIdentity {
        &self.declaration
    }

    pub const fn terminal_psi_identity(&self) -> TerminalPsiIdentity {
        self.terminal_psi_identity
    }

    pub const fn terminal_entry(&self) -> MachineId {
        self.terminal_entry
    }
}
