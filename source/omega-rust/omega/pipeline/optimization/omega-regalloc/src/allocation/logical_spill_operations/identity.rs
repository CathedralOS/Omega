use sha2::{Digest, Sha256};

use super::{LogicalSpillOperationIdentity, LogicalSpillOperationPlan};

pub fn logical_spill_operation_identity(
    plan: &LogicalSpillOperationPlan,
) -> LogicalSpillOperationIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-logical-spill-operations.v1\0");
    bytes.extend_from_slice(&super::codec::encode_content(plan));
    LogicalSpillOperationIdentity(Sha256::digest(bytes).into())
}
