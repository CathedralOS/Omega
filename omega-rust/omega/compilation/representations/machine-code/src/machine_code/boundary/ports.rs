//! Exact semantic service and byte intervals for privileged port effects.

use semantic_vocabulary::{OperationId, ServiceId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortEffectRecord {
    pub psi_operation: OperationId,
    pub service: ServiceId,
    pub port: u16,
    pub value: u8,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}
