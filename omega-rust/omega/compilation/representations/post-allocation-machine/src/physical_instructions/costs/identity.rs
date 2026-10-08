use sha2::{Digest, Sha256};
use target::NativeTarget;

use super::{TargetCostModelIdentity, TargetCostModelVersion};

const TARGET_COST_MODEL_IDENTITY_V1: &[u8] = b"omega.target-cost-model.identity.v1";

pub(super) fn target_cost_model_identity(
    target: NativeTarget,
    version: TargetCostModelVersion,
) -> TargetCostModelIdentity {
    let mut hasher = Sha256::new();
    hasher.update(TARGET_COST_MODEL_IDENTITY_V1);
    hasher.update([version_tag(version)]);
    hasher.update(target.wire_identity_bytes());
    TargetCostModelIdentity::from_bytes(hasher.finalize().into())
}

const fn version_tag(version: TargetCostModelVersion) -> u8 {
    match version {
        TargetCostModelVersion::MachineKnowledgeV1 => 1,
    }
}
