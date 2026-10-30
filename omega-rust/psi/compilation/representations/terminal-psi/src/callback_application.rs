//! Portable checked callback application, independent of source arenas and ABI.

/// Occurrence numbers are scoped to the producing compilation and retain their
/// identity when unused applications are pruned. The module binds each number
/// to an operation and contracts; the number alone grants no authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallbackApplication {
    pub application_index: u32,
    pub registrar: semantic_vocabulary::OperationId,
    pub static_machine_ordinal: u32,
    pub requirement: String,
    pub published_contract: [u8; 32],
    /// Psi admitted the requirement's callback entry contract. An emitted
    /// boundary use without one is retained so a foreign realization refuses.
    pub entry_admitted: bool,
    pub body: crate::CheckedMachineDeclaration,
}
