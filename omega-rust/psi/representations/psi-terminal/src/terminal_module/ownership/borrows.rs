use crate::{StructuralAccess, StructuralContentProjection, StructuralMultiplicity};
use psi_core::{DomainSemanticId, MachineId, OperationId};

/// Closed lifecycle interpretation for one restored-parent call publication.
///
/// The variants are deliberately not a general restoration algebra. They
/// distinguish the two exact checked tuples accepted by this bounded row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalReborrowRestorationClass {
    ExclusiveReactivation,
    SharedFreezeRestoration,
}

/// One exact member of the closed shared-freeze cohort restored by a bounded
/// restored-parent call publication.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalReborrowSharedCohortMember {
    pub child_owner_identity: String,
    pub child_owner_path: Vec<TerminalBorrowOwnerSegment>,
    pub child_place: TerminalBorrowPlace,
    pub child_access: StructuralAccess,
    pub child_activation: TerminalBorrowBoundarySource,
    pub child_weakening: TerminalBorrowBoundarySource,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalBorrowBoundarySource {
    Statement {
        statement_index: u64,
    },
    Call {
        statement_index: u64,
        call_ordinal: u64,
        target_identity: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalBorrowOwnerSegment {
    Field(String),
    Case(String),
    FixedIndex(u64),
    DynamicIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalBorrowPlaceSegment {
    Field(String),
    Case(String),
    FixedIndex(u64),
    FixedRange { start: u64, end: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalBorrowPlace {
    pub root_identity: String,
    pub segments: Vec<TerminalBorrowPlaceSegment>,
}

/// One exact child edge in a finite exclusive-reborrow root-handoff lineage.
///
/// Rows are ordered from the direct-root child toward the leaf whose closure
/// reaches state exit. The immediate parent's place and access are therefore
/// the handoff root for the first row and the preceding child's for every later
/// row. This representation has no shared-cohort or branching vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalReborrowRootHandoffStep {
    pub child_owner_identity: String,
    pub child_owner_path: Vec<TerminalBorrowOwnerSegment>,
    pub child_place: TerminalBorrowPlace,
    pub projection_remainder: Vec<TerminalBorrowPlaceSegment>,
    pub child_access: StructuralAccess,
    pub child_activation: TerminalBorrowBoundarySource,
    pub formation_boundary: TerminalBorrowBoundarySource,
    pub child_weakening: TerminalBorrowBoundarySource,
}

/// Closed publication of direct-root custody after one exact finite linear
/// exclusive-reborrow lineage has reached a checked state-exit handoff. The
/// row's vocabulary is intentionally incapable of expressing cleanup,
/// transfer, discharge, shared cohorts, or branching.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalReborrowRootHandoff {
    pub machine: MachineId,
    pub source_machine_identity: String,
    pub source_state_identity: String,
    pub direct_root_owner_identity: String,
    pub direct_root_owner_path: Vec<TerminalBorrowOwnerSegment>,
    pub direct_root_place: TerminalBorrowPlace,
    pub direct_root_access: StructuralAccess,
    pub direct_root_activation: TerminalBorrowBoundarySource,
    pub direct_root_weakening: TerminalBorrowBoundarySource,
    pub direct_root_lifetime_identity: String,
    pub lineage: Vec<TerminalReborrowRootHandoffStep>,
}

/// Closed publication of one exact use after one direct exclusive child, or
/// an exact one- or two-member shared-freeze cohort, has restored its mutable
/// parent. The canonical operation identifies the sole authorized use. Access,
/// restoration class, source call, and the exact shared roster are explicit;
/// carrier-read and restored-place facts fixed by these bounded forms remain
/// verifier rules. This vocabulary cannot express cleanup, transfer, or
/// discharge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalReborrowRestoredCallUse {
    pub machine: MachineId,
    pub operation: OperationId,
    pub restoration_class: TerminalReborrowRestorationClass,
    pub call_boundary: TerminalBorrowBoundarySource,
    pub call_target_machine: MachineId,
    pub source_machine_identity: String,
    pub source_state_identity: String,
    pub direct_root_owner_identity: String,
    pub direct_root_owner_path: Vec<TerminalBorrowOwnerSegment>,
    pub direct_root_place: TerminalBorrowPlace,
    pub direct_root_activation: TerminalBorrowBoundarySource,
    pub direct_root_weakening: TerminalBorrowBoundarySource,
    pub direct_root_lifetime_identity: String,
    pub child_owner_identity: String,
    pub child_owner_path: Vec<TerminalBorrowOwnerSegment>,
    pub child_place: TerminalBorrowPlace,
    pub projection_remainder: Vec<TerminalBorrowPlaceSegment>,
    pub child_access: StructuralAccess,
    pub child_activation: TerminalBorrowBoundarySource,
    pub formation_boundary: TerminalBorrowBoundarySource,
    pub child_weakening: TerminalBorrowBoundarySource,
    pub shared_cohort: Vec<TerminalReborrowSharedCohortMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RetainedBorrowCustody {
    pub callable_identity: String,
    pub source: RetainedBorrowPlace,
    pub result: RetainedBorrowPlace,
    pub access: StructuralAccess,
    pub callable_lifetime_parameter_count: u32,
    pub callable_lifetime_parameter_ordinal: u32,
    pub result_nominal_identity: String,
    pub result_multiplicity: StructuralMultiplicity,
    pub result_lifetime_argument_count: u32,
    pub result_lifetime_argument_ordinal: u32,
    pub result_lifetime_slot_is_erased: bool,
    pub retained_semantic_domain: DomainSemanticId,
    pub source_projection: RetainedBorrowContentProjection,
    pub result_projection: RetainedBorrowContentProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RetainedBorrowContentProjection {
    pub semantic_domain: DomainSemanticId,
    pub carrier_identity: String,
    pub projection: StructuralContentProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RetainedBorrowPlace {
    pub version: psi_core::ContentPlaceVersion,
    pub root: RetainedBorrowPlaceRoot,
    pub segments: Vec<psi_core::ContentPlaceSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RetainedBorrowPlaceRoot {
    Parameter {
        position: u32,
        identity: String,
        is_self: bool,
    },
    Result,
}
