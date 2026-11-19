use semantic_vocabulary::MachineId;

use crate::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
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
