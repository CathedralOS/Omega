//! Exact selected successor edges and conditional structural payloads.

use semantic_vocabulary::EdgeId;

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
