//! Per-value type witnesses are checking evidence. A package stores its
//! ordinary reference and callable fields. Each construction and opening
//! describes how their types relate at that operation.
use semantic_vocabulary::{MachineId, OperationId, StructuralFieldId, StructuralTypeId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalPackageCatalog {
    /// Ordered by the public record's structural type.
    pub schemas: Vec<TerminalPackageSchema>,
    /// A site names an opening by its zero-based index in this vector.
    pub openings: Vec<TerminalPackageOpening>,
    /// Ordered by (machine, operation), with at most one row per operation.
    pub sites: Vec<TerminalPackageSite>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPackageSchema {
    pub record: StructuralTypeId,
    /// Abstract sized referents, in declaration order. The package fields
    /// can use these behind references and in callable signatures.
    pub witnesses: Vec<StructuralTypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPackageOpening {
    pub machine: MachineId,
    pub record: StructuralTypeId,
    /// The projected package itself, before selecting one of its fields.
    pub subject: crate::StructuralArgument,
    /// Fresh abstract identities for this exact subject and storage version.
    /// Replay must establish that the subject stayed live between uses.
    pub witnesses: Vec<StructuralTypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPackageSite {
    pub machine: MachineId,
    pub operation: OperationId,
    pub event: TerminalPackageEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalPackageEvent {
    /// EstablishRecord checks every initializer under these concrete types.
    Construct { witnesses: Vec<StructuralTypeId> },
    /// The field operand must come from the subject named by this opening.
    Project { opening: u32, field: StructuralFieldId },
    /// The selected descriptor and its dependent arguments must have the
    /// abstract contract and types of the same opening.
    Invoke { opening: u32 },
}
