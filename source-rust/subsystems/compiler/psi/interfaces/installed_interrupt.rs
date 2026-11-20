#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInstalledInterruptRegistration {
    pub machine: semantic_vocabulary::MachineId,
    pub operation: semantic_vocabulary::OperationId,
    pub installer: semantic_vocabulary::BoundaryMachineId,
    pub service: semantic_vocabulary::ServiceId,
    pub registrar_boundary: semantic_vocabulary::BoundaryMachineId,
    pub registrar: crate::NominalDeclarationIdentity,
    pub registrar_owner: crate::NominalDeclarationIdentity,
    pub registrar_application: String,
    pub registrar_declaring_application: String,
    pub registrar_arguments: Vec<String>,
    pub registrar_lifetimes: Vec<String>,
    pub registrar_requirement: String,
    pub slot_declaration: crate::NominalDeclarationIdentity,
    pub slot_commitment: [u8; 32],
    pub slot_encoding: Vec<u8>,
    pub callbacks: Vec<crate::TerminalCallbackApplication>,
}
