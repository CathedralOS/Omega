//! Canonical legalized-plan roster encoding shared by current and legacy identities.

use super::encoding::encode_len;
use crate::legalized_operations::{LegalizedOperationPlan, LegalizedOperationPlanIdentity};

pub fn identity(plan: &LegalizedOperationPlan) -> LegalizedOperationPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-legalized-operations.v68\0");
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.fuel_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&plan.target.identity_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.entry.get().to_le_bytes());
    bytes.extend_from_slice(b"ordinary-scalar-graph.v32\0");
    encode_len(&mut bytes, plan.scalar_functions.len());
    for function in &plan.scalar_functions {
        super::scalar_graph::encode(&mut bytes, function);
    }
    LegalizedOperationPlanIdentity::from_canonical_bytes(&bytes)
}
