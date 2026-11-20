#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallbackApplication {
    pub application_index: u32,
    pub registrar_machine: semantic_vocabulary::MachineId,
    pub registrar: semantic_vocabulary::OperationId,
    pub static_machine_ordinal: u32,
    pub requirement: String,
    pub calling_owner_application: String,
    pub boundary: Option<semantic_vocabulary::BoundaryMachineId>,
    pub published_contract: [u8; 32],
    pub entry_admitted: bool,
    pub body: crate::CheckedMachineDeclaration,
}
