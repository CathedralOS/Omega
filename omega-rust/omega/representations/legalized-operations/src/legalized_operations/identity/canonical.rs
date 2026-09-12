//! Canonical legalized-plan roster encoding shared by current and legacy identities.

use super::shared::*;

pub(super) fn identity(plan: &LegalizedOperationPlan) -> LegalizedOperationPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-legalized-operations.v47\0");
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&plan.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.entry.get().to_le_bytes());
    {
        bytes.extend_from_slice(b"ordinary-scalar-graph.v17\0");
        encode_len(&mut bytes, plan.scalar_functions.len());
        for function in &plan.scalar_functions {
            super::scalar_graph::encode(&mut bytes, function);
        }
    }
    LegalizedOperationPlanIdentity::from_canonical_bytes(&bytes)
}
