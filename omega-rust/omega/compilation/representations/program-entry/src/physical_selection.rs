//! Physical adapter and checked semantic entry selected for one canonical program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEntrySelection {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub adapter: semantic_vocabulary::MachineId,
    pub semantic_entry: semantic_vocabulary::MachineId,
    pub inbound_ordinals: Vec<u32>,
}
