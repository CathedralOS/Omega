//! Exact selected successor edges and conditional structural payloads.

use crate::TargetUnitScalarHomeRequirement;
use psi_core::{EdgeId, StructuralFieldId};

/// One exact successor of the bounded attached-Unit equality diamond.
/// `operation_ordinal` names the first physical operation in that arm; the
/// nominal return edge remains semantic custody even though a preceding
/// nonreturning boundary realization makes it physically unreachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetUnitConditionalSuccessor {
    pub psi_edge: EdgeId,
    pub operation_ordinal: u32,
    pub nominal_return_edge: EdgeId,
}

/// One scalar payload binding exposed only on the matching closed-sum arm.
/// The home aliases the owning structural result; it is not a separately
/// allocated scalar-call result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetUnitStructuralCasePayload {
    pub field: StructuralFieldId,
    pub field_byte_offset: u32,
    pub home: TargetUnitScalarHomeRequirement,
}

/// One exact successor of the bounded closed-sum inspection lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUnitStructuralCaseSuccessor {
    pub psi_edge: EdgeId,
    pub case: psi_core::StructuralCaseId,
    pub case_tag: i32,
    pub operation_ordinal: u32,
    pub nominal_return_edge: EdgeId,
    pub payloads: Vec<TargetUnitStructuralCasePayload>,
}
