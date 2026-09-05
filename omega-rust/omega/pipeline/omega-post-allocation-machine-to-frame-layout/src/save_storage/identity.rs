use sha2::{Digest, Sha256};

use crate::save_storage::{
    AllocatedCalleeSavedFunctionKind, CalleeSavedModificationWitness,
    FrameAbiPreservationConvention,
};

use super::{
    NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy,
};

pub fn non_authoritative_callee_save_storage_identity(
    plan: &NonAuthoritativeCalleeSaveStoragePlan,
) -> NonAuthoritativeCalleeSaveStorageIdentity {
    let mut hasher = Sha256::new();
    hasher.update(b"omega.non-authoritative-callee-save-storage.v1");
    hasher.update(plan.callee_saved_requirements.bytes());
    hasher.update(plan.register_environment.bytes());
    hasher.update(plan.physical_register_model.bytes());
    hasher.update(plan.preservation_storage_catalog.bytes());
    target(&mut hasher, plan.target);
    hasher.update([abi_tag(plan.abi)]);
    units(&mut hasher, &plan.callee_saved_units);
    hasher.update([match plan.policy {
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1 => 0,
    }]);
    hasher.update(plan.budget.encode());
    hasher.update(plan.usage.encode());
    length(&mut hasher, plan.functions.len());
    for function in &plan.functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update([match function.kind {
            AllocatedCalleeSavedFunctionKind::Ordinary => 0,
            AllocatedCalleeSavedFunctionKind::StructuralUnit => 1,
        }]);
        hasher.update(function.abstract_area_bytes.to_le_bytes());
        hasher.update(function.abstract_area_alignment.to_le_bytes());
        length(&mut hasher, function.slots.len());
        for slot in &function.slots {
            hasher.update(slot.id.0.to_le_bytes());
            hasher.update(slot.storage_group.0.to_le_bytes());
            hasher.update(slot.storage_view.0.to_le_bytes());
            units(&mut hasher, &slot.preserved_units);
            length(&mut hasher, slot.modified_units.len());
            for requirement in &slot.modified_units {
                hasher.update(requirement.unit.0.to_le_bytes());
                length(&mut hasher, requirement.witnesses.len());
                for witness in &requirement.witnesses {
                    encode_witness(&mut hasher, *witness);
                }
            }
            hasher.update(slot.abstract_offset_bytes.to_le_bytes());
            hasher.update(slot.size_bytes.to_le_bytes());
            hasher.update(slot.alignment_bytes.to_le_bytes());
        }
    }
    NonAuthoritativeCalleeSaveStorageIdentity::from_bytes(hasher.finalize().into())
}

fn encode_witness(hasher: &mut Sha256, witness: CalleeSavedModificationWitness) {
    match witness {
        CalleeSavedModificationWitness::OperandDefinition {
            block,
            instruction,
            operand,
            virtual_register,
            home_view,
            write_semantics,
        } => {
            hasher.update([0]);
            hasher.update(block.0.to_le_bytes());
            hasher.update(instruction.0.to_le_bytes());
            hasher.update(operand.to_le_bytes());
            hasher.update(virtual_register.0.to_le_bytes());
            hasher.update(home_view.0.to_le_bytes());
            hasher.update([write_semantics_tag(write_semantics)]);
        }
        CalleeSavedModificationWitness::ImplicitDefinition { block, instruction } => {
            hasher.update([1]);
            hasher.update(block.0.to_le_bytes());
            hasher.update(instruction.0.to_le_bytes());
        }
        CalleeSavedModificationWitness::ImplicitClobber { block, instruction } => {
            hasher.update([2]);
            hasher.update(block.0.to_le_bytes());
            hasher.update(instruction.0.to_le_bytes());
        }
    }
}

fn target(hasher: &mut Sha256, target: omega_target::NativeTarget) {
    hasher.update([match target.architecture {
        omega_target::Architecture::Aarch64 => 0,
        omega_target::Architecture::X86_64 => 1,
    }]);
    hasher.update([match target.object_format {
        omega_target::ObjectFormat::Elf => 0,
        omega_target::ObjectFormat::MachO => 1,
        omega_target::ObjectFormat::Coff => 2,
    }]);
    hasher.update((target.pointer_size as u64).to_le_bytes());
    hasher.update((target.pointer_alignment as u64).to_le_bytes());
}

fn abi_tag(abi: FrameAbiPreservationConvention) -> u8 {
    match abi {
        FrameAbiPreservationConvention::SystemVAMD64 => 0,
        FrameAbiPreservationConvention::MicrosoftX64 => 1,
        FrameAbiPreservationConvention::Aapcs64 => 2,
        FrameAbiPreservationConvention::DarwinAapcs64 => 3,
    }
}

fn write_semantics_tag(value: omega_register_model::RegisterWriteSemantics) -> u8 {
    match value {
        omega_register_model::RegisterWriteSemantics::ExactView => 0,
        omega_register_model::RegisterWriteSemantics::PreservesUnwritten => 1,
        omega_register_model::RegisterWriteSemantics::ZeroExtendsParent => 2,
        omega_register_model::RegisterWriteSemantics::ZeroExtendsWithinUnit => 3,
        omega_register_model::RegisterWriteSemantics::Discards => 4,
        omega_register_model::RegisterWriteSemantics::InstructionDefined => 5,
    }
}

fn units(hasher: &mut Sha256, values: &[omega_register_model::RegisterUnitId]) {
    length(hasher, values.len());
    for value in values {
        hasher.update(value.0.to_le_bytes());
    }
}

fn length(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_le_bytes());
}
