//! Canonical legalized-plan roster encoding shared by current and legacy identities.

use super::projected_structural_call_return::encode_projected_structural_call_return;
use super::shared::*;

pub(super) fn identity(plan: &LegalizedOperationPlan) -> LegalizedOperationPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-legalized-operations.v31\0");
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&plan.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.entry.get().to_le_bytes());
    encode_len(&mut bytes, plan.projected_structural_call_returns.len());
    for closure in &plan.projected_structural_call_returns {
        encode_projected_structural_call_return(&mut bytes, closure);
    }
    {
        bytes.extend_from_slice(b"ordinary-scalar-graph.v4\0");
        encode_len(&mut bytes, plan.scalar_functions.len());
        for function in &plan.scalar_functions {
            super::scalar_graph::encode(&mut bytes, function);
        }
    }
    LegalizedOperationPlanIdentity::from_canonical_bytes(&bytes)
}
