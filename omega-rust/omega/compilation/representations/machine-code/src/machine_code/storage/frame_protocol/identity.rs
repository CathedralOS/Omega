use sha2::{Digest, Sha256};

use super::{
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy,
};

pub fn target_frame_protocol_encoding_identity(
    plan: &TargetFrameProtocolEncodingPlan,
) -> TargetFrameProtocolEncodingIdentity {
    let mut hasher = Sha256::new();
    hasher.update(b"omega.target-frame-protocol-encoding.v1");
    hasher.update(plan.frame_layout.bytes());
    hasher.update(plan.register_environment.bytes());
    hasher.update(plan.physical_register_model.bytes());
    hasher.update(plan.target.identity_bytes());
    hasher.update([match plan.policy {
        TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1 => 0,
    }]);
    hasher.update((plan.functions.len() as u64).to_le_bytes());
    for function in &plan.functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.prologue.offset.to_le_bytes());
        hasher.update(function.prologue.length.to_le_bytes());
        hasher.update(function.epilogue.offset.to_le_bytes());
        hasher.update(function.epilogue.length.to_le_bytes());
    }
    hasher.update((plan.bytes.len() as u64).to_le_bytes());
    hasher.update(&plan.bytes);
    TargetFrameProtocolEncodingIdentity::from_bytes(hasher.finalize().into())
}
