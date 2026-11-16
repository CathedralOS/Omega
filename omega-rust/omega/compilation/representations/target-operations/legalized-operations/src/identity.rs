//! The prepared graph binds the shared record encodings without owning them.

mod scalar_graph;
use crate::{LegalizedOperationPlan, LegalizedOperationPlanIdentity};
pub use target_vocabulary::identity::*;

pub fn legalized_operation_plan_identity(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-legalized-operations.v89\0");
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&plan.target.identity_bytes());
    encode_option(&mut bytes, plan.profile, |bytes, profile| {
        let name = profile.target_name().as_bytes();
        encode_len(bytes, name.len());
        bytes.extend_from_slice(name);
    });
    crate::codec_bytes::push_encoded(&mut bytes, plan.entry.get().to_le_bytes());
    encode_len(&mut bytes, plan.installation_operations.len());
    for operation in &plan.installation_operations {
        crate::codec_bytes::push_encoded(&mut bytes, operation.get().to_le_bytes());
    }
    encode_len(&mut bytes, plan.function_abis.len());
    for abi in &plan.function_abis {
        function_abi::encode_function_abi(&mut bytes, abi);
    }
    bytes.extend_from_slice(&plan.borrowed_storage_layouts.identity_bytes());
    bytes.extend_from_slice(b"ordinary-scalar-graph.v46\0");
    encode_len(&mut bytes, plan.scalar_functions.len());
    let mut cached_catalog = None;
    for function in &plan.scalar_functions {
        scalar_graph::encode(&mut bytes, function, &mut cached_catalog);
    }
    LegalizedOperationPlanIdentity::from_canonical_bytes(&bytes)
}

#[cfg(test)]
mod tests {
    #[test]
    fn prepared_plan_identity_binds_installations() {
        let mut plan = crate::tests::scalar_call_unit_plan();
        let original = super::legalized_operation_plan_identity(&plan);
        plan.installation_operations
            .push(semantic_vocabulary::OperationId::new(99).unwrap());
        assert_ne!(original, super::legalized_operation_plan_identity(&plan));
    }
}
