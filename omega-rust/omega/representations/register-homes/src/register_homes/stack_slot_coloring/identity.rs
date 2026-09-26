use sha2::{Digest, Sha256};

use super::{StackSlotColoringIdentity, StackSlotColoringPlan};

pub fn stack_slot_coloring_identity(plan: &StackSlotColoringPlan) -> StackSlotColoringIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.stack-slot-coloring.v1\0");
    bytes.extend_from_slice(&super::codec::encode_content(plan));
    StackSlotColoringIdentity(Sha256::digest(bytes).into())
}
