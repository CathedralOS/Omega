use crate::identity::encode_fixed_constraint_site_identity;

use crate::{AllocationLegalityIdentity, AllocationLegalityPlan, FunctionAllocationLegality};

/// `allocation_legality_identity` from the digests of `plan.functions`, in
/// order, as `allocation_legality_function_digest` computes them.
pub fn allocation_legality_identity_from_digests(
    plan: &AllocationLegalityPlan,
    digests: &[[u8; 32]],
) -> AllocationLegalityIdentity {
    assert_eq!(
        digests.len(),
        plan.functions.len(),
        "one digest per allocation-legality function"
    );
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-allocation-legality.v8\0");
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    length(&mut bytes, digests.len());
    for digest in digests {
        bytes.extend_from_slice(digest);
    }
    AllocationLegalityIdentity::from_bytes(crate::digest_encoder::content_identity(&bytes).into())
}

/// One function's share of `allocation_legality_identity`.
pub fn allocation_legality_function_digest(function: &FunctionAllocationLegality) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-allocation-legality-function.v2\0");
    crate::codec_bytes::push_encoded(&mut bytes, function.machine.get().to_le_bytes());
    length(&mut bytes, function.virtual_registers.len());
    for register in &function.virtual_registers {
        crate::codec_bytes::push_encoded(&mut bytes, register.virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, register.class.0.to_le_bytes());
        length(&mut bytes, register.spans.len());
        for point in &register.spans {
            crate::codec_bytes::push_encoded(&mut bytes, point.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.start.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.end.0.to_le_bytes());
            length(&mut bytes, point.candidates.len());
            for candidate in &point.candidates {
                crate::codec_bytes::push_encoded(&mut bytes, candidate.0.to_le_bytes());
            }
        }
        length(&mut bytes, register.early_clobber_points.len());
        for point in &register.early_clobber_points {
            crate::codec_bytes::push_encoded(&mut bytes, point.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.position.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.operand.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.point.0.to_le_bytes());
            length(&mut bytes, point.candidates.len());
            for candidate in &point.candidates {
                crate::codec_bytes::push_encoded(&mut bytes, candidate.0.to_le_bytes());
            }
        }
        length(&mut bytes, register.entry_transitions.len());
        for transition in &register.entry_transitions {
            crate::codec_bytes::push_encoded(&mut bytes, transition.from_view.0.to_le_bytes());
            encode_fixed_constraint_site_identity(&mut bytes, transition.to_site);
            crate::codec_bytes::push_encoded(&mut bytes, transition.to_view.0.to_le_bytes());
        }
    }
    crate::digest_encoder::content_identity(&bytes).into()
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("allocation-legality identity length fits u64")
            .to_le_bytes(),
    );
}
