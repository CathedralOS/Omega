//! In-memory entry facts carried alongside canonical code. These data do not grant host authority.

use semantic_vocabulary::compiler_evaluation::DeclarationCustody;

#[derive(Debug)]
pub struct PublishedEntryMetadata {
    provenance: crate::TerminalMachineProvenance,
    entry_custody: Vec<(
        crate::TerminalIdentityIndex,
        Result<DeclarationCustody, String>,
    )>,
}

impl PublishedEntryMetadata {
    pub fn new(
        provenance: crate::TerminalMachineProvenance,
        entry_custody: Vec<(
            crate::TerminalIdentityIndex,
            Result<DeclarationCustody, String>,
        )>,
    ) -> Self {
        Self {
            provenance,
            entry_custody,
        }
    }

    pub fn machine_provenance(&self) -> &crate::TerminalMachineProvenance {
        &self.provenance
    }
    pub fn entry_schema_custody(
        &self,
        declaration: crate::TerminalIdentityIndex,
    ) -> Result<DeclarationCustody, String> {
        self.entry_custody
            .iter()
            .find(|(identity, _)| *identity == declaration)
            .ok_or_else(|| "selected declaration has no authenticated source custody".to_owned())?
            .1
            .clone()
    }
}
