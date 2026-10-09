use sha2::{Digest, Sha256};

use super::{
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy,
};

pub fn target_frame_protocol_encoding_identity(
    plan: &TargetFrameProtocolEncodingPlan,
) -> TargetFrameProtocolEncodingIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"omega.target-frame-protocol-encoding.v1");
    hasher.raw(&plan.frame_layout.bytes());
    hasher.raw(&plan.register_environment.bytes());
    hasher.raw(&plan.physical_register_model.bytes());
    hasher.raw(&plan.target.identity_bytes());
    hasher.byte(match plan.policy {
        TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1 => 0,
    });
    hasher.u64(plan.functions.len() as u64);
    for function in &plan.functions {
        hasher.raw(&function.machine.get().to_le_bytes());
        hasher.raw(&function.prologue.offset.to_le_bytes());
        hasher.raw(&function.prologue.length.to_le_bytes());
        hasher.raw(&function.epilogue.offset.to_le_bytes());
        hasher.raw(&function.epilogue.length.to_le_bytes());
    }
    hasher.u64(plan.bytes.len() as u64);
    hasher.update(&plan.bytes);
    TargetFrameProtocolEncodingIdentity::from_bytes(hasher.finish())
}
