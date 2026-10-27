use sha2::{Digest, Sha256};
use target::NativeTarget;

use super::{TargetCostModelIdentity, TargetCostModelVersion};

const TARGET_COST_MODEL_IDENTITY_V1: &[u8] = b"omega.target-cost-model.identity.v1";

pub(super) fn target_cost_model_identity(
    target: NativeTarget,
    version: TargetCostModelVersion,
) -> TargetCostModelIdentity {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(TARGET_COST_MODEL_IDENTITY_V1);
    digest.byte(version_tag(version));
    digest.raw(&target.wire_identity_bytes());
    TargetCostModelIdentity::from_bytes(digest.finish())
}

const fn version_tag(version: TargetCostModelVersion) -> u8 {
    match version {
        TargetCostModelVersion::MachineKnowledgeV1 => 1,
    }
}
