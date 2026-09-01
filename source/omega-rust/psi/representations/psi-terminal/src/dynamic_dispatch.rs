use psi_core::{MachineId, OperationId};

use crate::{ClosedConformanceApplicationCommitment, StructuralArgument};

/// Source-free local dynamic-dispatch custody retained by one Terminal module.
///
/// Direct rows consume one selection. Indirect rows consume one rebound
/// descriptor whose two selections retain initializer and latest-source
/// custody without placing compiler-private coordinates on the operation.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicDispatchCatalog {
    /// Ordered by owner and dense caller-local ordinal.
    pub selections: Vec<TerminalDynamicConformanceSelection>,
    /// Ordered by owner and dense owner-local descriptor ordinal.
    pub rebound_descriptors: Vec<TerminalReboundDynamicDescriptor>,
    /// Ordered by owner and operation.
    pub direct_dispatches: Vec<TerminalDirectDynamicDispatch>,
    /// Ordered by owner and operation.
    pub indirect_dispatches: Vec<TerminalIndirectDynamicDispatch>,
}

/// One caller-local selection of an exact nominal conformance for a dynamic value.
///
/// The report fingerprint is an index coordinate only. The adjacent strong
/// commitment and `owner` must identify exactly one closed conformance
/// application before this row has semantic meaning.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicConformanceSelection {
    pub owner: MachineId,
    /// Dense within `owner`, starting at zero.
    pub ordinal: u32,
    /// Exact structural source retained by the selected dynamic value.
    pub source: StructuralArgument,
    pub conformance_application_report_fingerprint: u64,
    pub conformance_application_commitment: ClosedConformanceApplicationCommitment,
}

/// One direct scalar dispatch through a caller-local dynamic selection.
///
/// Requirement and realization identities repeat only the exact selected row
/// coordinates needed to join the executable operation to the owning closed
/// conformance application. The application remains the sole row catalog.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDirectDynamicDispatch {
    pub owner: MachineId,
    pub operation: OperationId,
    pub selection_ordinal: u32,
    pub declaring_trait_identity: String,
    pub public_requirement_identity: String,
    pub requirement_identity: String,
    pub realization_identity: String,
    pub realization_callable_identity: String,
    pub realization: MachineId,
}

/// One descriptor whose source was replaced exactly once before dispatch.
///
/// Both selections must name the same closed conformance application. The
/// latest selection supplies the runtime instance while the common application
/// supplies the table identity; neither source may be discarded or rewritten
/// as direct devirtualization.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalReboundDynamicDescriptor {
    pub owner: MachineId,
    /// Dense within `owner`, starting at zero.
    pub ordinal: u32,
    pub initial_selection_ordinal: u32,
    pub rebound_selection_ordinal: u32,
}

/// One scalar dispatch through a materialized dynamic descriptor.
///
/// The selected row coordinates identify the only callable permitted in the
/// descriptor table. The operation itself carries no statically addressed
/// callee.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalIndirectDynamicDispatch {
    pub owner: MachineId,
    pub operation: OperationId,
    pub descriptor_ordinal: u32,
    pub declaring_trait_identity: String,
    pub public_requirement_identity: String,
    pub requirement_identity: String,
    pub realization_identity: String,
    pub realization_callable_identity: String,
    pub realization: MachineId,
}
