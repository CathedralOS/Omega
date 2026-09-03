use psi_core::{MachineId, OperationId};

use crate::{
    ClosedConformanceApplicationCommitment, ClosedConformanceCallableResult, StructuralAccess,
    StructuralArgument,
};

/// Source-free local dynamic-dispatch custody retained by one Terminal module.
///
/// Direct rows consume one selection. Indirect rows consume one rebound
/// descriptor whose two selections retain initializer and latest-source
/// custody without placing compiler-private coordinates on the operation.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicDispatchCatalog {
    /// Ordered by owner and dense owner-local dynamic-parameter ordinal.
    pub parameters: Vec<TerminalDynamicDescriptorParameter>,
    /// Ordered by caller and call operation. Each row supplies one exact
    /// callee dynamic parameter without embedding physical ABI placement.
    pub arguments: Vec<TerminalDynamicDescriptorArgument>,
    /// Ordered by owner and dense caller-local ordinal.
    pub selections: Vec<TerminalDynamicConformanceSelection>,
    /// Ordered by owner and dense owner-local descriptor ordinal.
    pub rebound_descriptors: Vec<TerminalReboundDynamicDescriptor>,
    /// Ordered by owner and dense owner-local aggregate descriptor ordinal.
    pub stored_descriptors: Vec<TerminalStoredDynamicDescriptor>,
    /// Ordered by owner and operation.
    pub direct_dispatches: Vec<TerminalDirectDynamicDispatch>,
    /// Ordered by owner and operation.
    pub indirect_dispatches: Vec<TerminalIndirectDynamicDispatch>,
    /// Ordered by owner and operation. These consume an exact descriptor
    /// previously established in a local aggregate field.
    pub stored_dispatches: Vec<TerminalStoredDynamicDispatch>,
    /// Ordered by owner and operation. These dispatches consume a descriptor
    /// received through a dynamic parameter rather than an owner-local
    /// materialized descriptor.
    pub parameter_dispatches: Vec<TerminalParameterDynamicDispatch>,
}

/// One target-neutral existential dynamic parameter of a Terminal machine.
///
/// `ordinal` is dense only within the machine's dynamic-parameter lane.
/// `source_position` retains its position among all authored non-self runtime
/// parameters so independently built producers cannot silently reorder the
/// source interface while preserving lane-local ordinals.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicDescriptorParameter {
    pub owner: MachineId,
    pub ordinal: u32,
    pub source_position: u32,
    pub trait_identity: String,
    pub access: StructuralAccess,
    /// Complete table interface in canonical slot order. Realization identity
    /// is deliberately absent: the caller supplies one exact conforming
    /// descriptor at each invocation.
    pub requirements: Vec<TerminalDynamicRequirement>,
}

/// One callable slot required by an existential dynamic parameter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicRequirement {
    pub slot: u32,
    pub declaring_trait_identity: String,
    pub public_requirement_identity: String,
    pub result: ClosedConformanceCallableResult,
}

/// The semantic source of one dynamic argument. Physical data/table pointer
/// placement is selected only after Terminal Psi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalDynamicDescriptorSource {
    /// One owner-local exact conformance selection used directly as a
    /// descriptor argument, without fabricating a rebound version pair.
    Selection {
        ordinal: u32,
    },
    ReboundDescriptor {
        ordinal: u32,
    },
    Parameter {
        ordinal: u32,
    },
}

/// One descriptor passed by an ordinary in-module call.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicDescriptorArgument {
    pub owner: MachineId,
    pub operation: OperationId,
    pub parameter_ordinal: u32,
    pub source: TerminalDynamicDescriptorSource,
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
/// Both selections must name the same exact dynamic-trait interface. Their
/// closed conformance applications may differ. The latest selection supplies
/// the runtime instance and table identity; neither source may be discarded or
/// rewritten as direct devirtualization.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalReboundDynamicDescriptor {
    pub owner: MachineId,
    /// Dense within `owner`, starting at zero.
    pub ordinal: u32,
    pub initial_selection_ordinal: u32,
    pub rebound_selection_ordinal: u32,
}

/// One selected descriptor established in a field of a local aggregate.
/// Aggregate identity and field identity remain target-neutral; physical
/// two-word placement belongs to later representation planning.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalStoredDynamicDescriptor {
    pub owner: MachineId,
    pub ordinal: u32,
    pub establishment_operation: OperationId,
    pub selection_ordinal: u32,
    pub aggregate_type_identity: String,
    pub field_identity: String,
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

/// One scalar dispatch through a descriptor loaded from an exact local
/// aggregate field.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalStoredDynamicDispatch {
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

/// One scalar dispatch through an existential descriptor parameter.
/// Requirement identity and result shape come from the parameter's closed
/// interface; the concrete realization is selected by the incoming table.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalParameterDynamicDispatch {
    pub owner: MachineId,
    pub operation: OperationId,
    pub parameter_ordinal: u32,
    pub requirement_slot: u32,
}
