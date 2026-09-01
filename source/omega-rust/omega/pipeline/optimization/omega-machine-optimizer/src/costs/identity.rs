use omega_target::{Architecture, NativeTarget, ObjectFormat};
use sha2::{Digest, Sha256};

use super::{TargetCostModelIdentity, TargetCostModelVersion};

const TARGET_COST_MODEL_IDENTITY_V1: &[u8] = b"omega.target-cost-model.identity.v1";

pub(super) fn target_cost_model_identity(
    target: NativeTarget,
    version: TargetCostModelVersion,
) -> TargetCostModelIdentity {
    let mut hasher = Sha256::new();
    hasher.update(TARGET_COST_MODEL_IDENTITY_V1);
    hasher.update([version_tag(version)]);
    hasher.update([architecture_tag(target.architecture)]);
    hasher.update([object_format_tag(target.object_format)]);
    hasher.update((target.pointer_size as u64).to_le_bytes());
    hasher.update((target.pointer_alignment as u64).to_le_bytes());
    TargetCostModelIdentity::from_bytes(hasher.finalize().into())
}

const fn version_tag(version: TargetCostModelVersion) -> u8 {
    match version {
        TargetCostModelVersion::MachineKnowledgeV1 => 1,
    }
}

const fn architecture_tag(architecture: Architecture) -> u8 {
    match architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }
}

const fn object_format_tag(object_format: ObjectFormat) -> u8 {
    match object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }
}
