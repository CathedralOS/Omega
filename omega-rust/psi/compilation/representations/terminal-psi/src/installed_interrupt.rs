//! Checked installation requests, independent of native addresses and layout.
//!
//! The operation's six ordinary operands retain the table, descriptor tables,
//! stacks, arrival descriptions and context. These rows bind its separately
//! selected registrar and slot; native admission still owes actual backing,
//! materialization, authority and a matching physical arrival implementation.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInstalledInterruptRegistration {
    pub operation: semantic_vocabulary::OperationId,
    pub installer: semantic_vocabulary::BoundaryMachineId,
    /// Exact core MachineControl service retained by source admission.
    pub service: semantic_vocabulary::ServiceId,
    /// Exact emitted registrar requirement, independent of the Scope operation.
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
