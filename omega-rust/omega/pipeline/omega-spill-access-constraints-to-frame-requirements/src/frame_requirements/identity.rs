use sha2::{Digest, Sha256};

use super::{
    FrameAbiPreservationConvention, NonAuthoritativeSpillFrameRequirementIdentity,
    NonAuthoritativeSpillFrameRequirementPlan, NonAuthoritativeSpillFrameRequirementPolicy,
};

pub fn non_authoritative_spill_frame_requirement_identity(
    plan: &NonAuthoritativeSpillFrameRequirementPlan,
) -> NonAuthoritativeSpillFrameRequirementIdentity {
    let mut hasher = Sha256::new();
    hasher.update(b"omega.non-authoritative-spill-frame-requirements.v1");
    hasher.update(plan.abstract_spill_access_constraints.bytes());
    hasher.update(plan.register_environment.bytes());
    hasher.update([match plan.target.architecture {
        omega_target::Architecture::Aarch64 => 0,
        omega_target::Architecture::X86_64 => 1,
    }]);
    hasher.update([match plan.target.object_format {
        omega_target::ObjectFormat::Elf => 0,
        omega_target::ObjectFormat::MachO => 1,
        omega_target::ObjectFormat::Coff => 2,
    }]);
    hasher.update((plan.target.pointer_size as u64).to_le_bytes());
    hasher.update((plan.target.pointer_alignment as u64).to_le_bytes());
    hasher.update([match plan.policy {
        NonAuthoritativeSpillFrameRequirementPolicy::AbstractSpillAreaAndPreservationConventionV1 => 0,
    }]);
    hasher.update(plan.budget.encode());
    hasher.update(plan.usage.encode());
    hasher.update((plan.functions.len() as u64).to_le_bytes());
    for function in &plan.functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.abstract_spill_area_bytes.to_le_bytes());
        hasher.update(function.abstract_spill_area_alignment.to_le_bytes());
        hasher.update([match function.abi_preservation_convention {
            FrameAbiPreservationConvention::SystemVAMD64 => 0,
            FrameAbiPreservationConvention::MicrosoftX64 => 1,
            FrameAbiPreservationConvention::Aapcs64 => 2,
            FrameAbiPreservationConvention::DarwinAapcs64 => 3,
        }]);
        hasher.update(function.abi_stack_alignment.to_le_bytes());
        hasher.update(function.abi_red_zone_capacity_bytes.to_le_bytes());
    }
    NonAuthoritativeSpillFrameRequirementIdentity::from_bytes(hasher.finalize().into())
}
