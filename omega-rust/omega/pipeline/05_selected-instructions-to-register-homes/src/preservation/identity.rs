use sha2::{Digest, Sha256};

use super::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedRequirementPolicy, encode_callee_saved_modification_witness_identity,
};

pub fn allocated_callee_saved_requirement_identity(
    plan: &AllocatedCalleeSavedRequirementPlan,
) -> AllocatedCalleeSavedRequirementIdentity {
    let mut hasher = Sha256::new();
    hasher.update(b"omega.allocated-callee-saved-requirements.v2");
    hasher.update(plan.selected.bytes());
    hasher.update(plan.homes.bytes());
    hasher.update(plan.post_allocation_manifest.bytes());
    hasher.update(plan.register_environment.bytes());
    hasher.update(plan.physical_register_model.bytes());
    target(&mut hasher, plan.target);
    hasher.update([abi_tag(plan.abi)]);
    units(&mut hasher, &plan.callee_saved_units);
    hasher.update([match plan.policy {
        AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1 => 0,
    }]);
    hasher.update(plan.budget.encode());
    hasher.update(plan.usage.encode());
    length(&mut hasher, plan.functions.len());
    for function in &plan.functions {
        hasher.update(function.machine.get().to_le_bytes());
        length(&mut hasher, function.modified_units.len());
        for requirement in &function.modified_units {
            hasher.update(requirement.unit.0.to_le_bytes());
            length(&mut hasher, requirement.witnesses.len());
            for witness in &requirement.witnesses {
                encode_callee_saved_modification_witness_identity(&mut hasher, *witness);
            }
        }
    }
    AllocatedCalleeSavedRequirementIdentity::from_bytes(hasher.finalize().into())
}

fn target(hasher: &mut Sha256, target: target::NativeTarget) {
    hasher.update([match target.architecture {
        target::Architecture::Aarch64 => 0,
        target::Architecture::X86_64 => 1,
    }]);
    hasher.update([match target.object_format {
        target::ObjectFormat::Elf => 0,
        target::ObjectFormat::MachO => 1,
        target::ObjectFormat::Coff => 2,
    }]);
    hasher.update((target.pointer_size as u64).to_le_bytes());
    hasher.update((target.pointer_alignment as u64).to_le_bytes());
}

fn abi_tag(abi: register_environment::FrameAbiPreservationConvention) -> u8 {
    match abi {
        register_environment::FrameAbiPreservationConvention::SystemVAMD64 => 0,
        register_environment::FrameAbiPreservationConvention::MicrosoftX64 => 1,
        register_environment::FrameAbiPreservationConvention::Aapcs64 => 2,
        register_environment::FrameAbiPreservationConvention::DarwinAapcs64 => 3,
    }
}

fn units(hasher: &mut Sha256, values: &[register_model::RegisterUnitId]) {
    length(hasher, values.len());
    for value in values {
        hasher.update(value.0.to_le_bytes());
    }
}

fn length(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_le_bytes());
}
