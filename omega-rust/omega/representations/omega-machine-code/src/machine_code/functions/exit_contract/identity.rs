//! Canonical version-9 exit-record identity; no producer or admission dependency.

use crate::{
    X86_64StructuralUnitInternalControlFixup, X86_64StructuralUnitInternalControlFixupKind,
    X86_64StructuralUnitInternalControlFixupState,
};
use omega_register_model::RegisterUnitId;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use sha2::{Digest, Sha256};

use super::{
    WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
    WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionFrameDisposition,
    WholeFunctionReturnEvidence, WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
};

const CONTRACT_SCHEMA: &[u8] = b"omega.terminal.whole-function-exit-contract.v9\0";

pub fn whole_function_exit_contract_identity(
    contract: &WholeFunctionExitContract,
) -> WholeFunctionExitContractIdentity {
    let mut hasher = Sha256::new();
    hasher.update(CONTRACT_SCHEMA);
    hasher.update(contract.selected.bytes());
    hasher.update(contract.post_allocation_manifest.bytes());
    hasher.update(contract.post_allocation_machine.bytes());
    hasher.update(contract.register_environment.bytes());
    hasher.update(contract.physical_register_model.bytes());
    hasher.update(contract.pre_layout.bytes());
    hasher.update(contract.resolved_layout.bytes());
    match contract.layout_custody {
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1 => hasher.update([1]),
        WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { relaxation } => {
            hasher.update([2]);
            hasher.update(relaxation.bytes());
        }
        WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
            optimization,
            artifact_identity,
        } => {
            hasher.update([3]);
            hasher.update([optimization as u8]);
            hasher.update(artifact_identity);
        }
        WholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
            fusion,
        } => {
            hasher.update([4]);
            hasher.update(fusion.bytes());
        }
        WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
            materialization,
        } => {
            hasher.update([5]);
            hasher.update(materialization.bytes());
        }
    }
    encode_target(&mut hasher, contract.target);
    hasher.update([policy_tag(contract.policy)]);
    match contract.frame {
        WholeFunctionFrameDisposition::FramelessV1 => hasher.update([1]),
        WholeFunctionFrameDisposition::CanonicalFixedFrameV1 { layout, protocol } => {
            hasher.update([2]);
            hasher.update(layout.bytes());
            hasher.update(protocol.bytes());
        }
    }
    hasher.update([1]);
    match contract.entry_assumption {
        WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1 => {
            hasher.update([1]);
        }
        WholeFunctionEntryAssumption::CallerLinkRegisterV1 { link_register } => {
            hasher.update([2]);
            hasher.update(link_register.0.to_le_bytes());
        }
    }
    hasher.update(contract.stack_pointer.0.to_le_bytes());
    hasher.update(contract.stack_alignment.to_le_bytes());
    hasher.update(contract.red_zone_bytes.to_le_bytes());
    hasher.update(contract.result_view.0.to_le_bytes());
    encode_units(&mut hasher, &contract.callee_saved_units);
    hasher.update((contract.functions.len() as u64).to_le_bytes());
    for function in contract.functions.iter() {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.entry_block.0.to_le_bytes());
        hasher.update(function.body_stack_delta.to_le_bytes());
        encode_units(&mut hasher, &function.modified_callee_saved_units);
        hasher.update((function.returns.len() as u64).to_le_bytes());
        for returned in &function.returns {
            hasher.update(returned.block.0.to_le_bytes());
            hasher.update(returned.psi_return_edge.get().to_le_bytes());
            hasher.update(returned.instruction.0.to_le_bytes());
            hasher.update(returned.offset.to_le_bytes());
            hasher.update((returned.bytes.len() as u64).to_le_bytes());
            hasher.update(&returned.bytes);
            match &returned.value {
                WholeFunctionReturnValueEvidence::UnitV1 => hasher.update([1]),
                WholeFunctionReturnValueEvidence::ScalarI64V1 {
                    virtual_register,
                    view,
                    units,
                } => {
                    hasher.update([2]);
                    hasher.update(virtual_register.0.to_le_bytes());
                    hasher.update(view.0.to_le_bytes());
                    encode_units(&mut hasher, units);
                }
            }
            hasher.update([1]);
            match returned.mechanism {
                WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                    stack_pointer,
                    read_bytes,
                    pop_bytes,
                } => {
                    hasher.update([1]);
                    hasher.update(stack_pointer.0.to_le_bytes());
                    hasher.update(read_bytes.to_le_bytes());
                    hasher.update(pop_bytes.to_le_bytes());
                }
                WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
                    stack_pointer,
                    link_register,
                } => {
                    hasher.update([2]);
                    hasher.update(stack_pointer.0.to_le_bytes());
                    hasher.update(link_register.0.to_le_bytes());
                }
            }
        }
    }
    hasher.update((contract.structural_unit_functions.len() as u64).to_le_bytes());
    for function in contract.structural_unit_functions.iter() {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.entry_block.0.to_le_bytes());
        hasher.update(function.body_stack_delta.to_le_bytes());
        encode_units(&mut hasher, &function.modified_callee_saved_units);
        match &function.call {
            None => hasher.update([0]),
            Some(call) => {
                hasher.update([1]);
                hasher.update(call.block.0.to_le_bytes());
                hasher.update(call.instruction.0.to_le_bytes());
                hasher.update(call.operation.get().to_le_bytes());
                hasher.update(call.callee.get().to_le_bytes());
                hasher.update(call.offset.to_le_bytes());
                hasher.update((call.bytes.len() as u64).to_le_bytes());
                hasher.update(&call.bytes);
                encode_structural_fixup(&mut hasher, call.fixup);
                encode_units(&mut hasher, &call.unit_uses);
                encode_units(&mut hasher, &call.unit_defs);
                encode_units(&mut hasher, &call.unit_clobbers);
                hasher.update(call.frame_byte_count.to_le_bytes());
                hasher.update(call.shadow_byte_count.to_le_bytes());
                hasher.update(call.pre_call_stack_alignment.to_le_bytes());
                hasher.update([u8::from(call.frame_is_balanced)]);
            }
        }
        encode_return(&mut hasher, &function.returned);
    }
    WholeFunctionExitContractIdentity::from_bytes(hasher.finalize().into())
}

fn encode_return(hasher: &mut Sha256, returned: &WholeFunctionReturnEvidence) {
    hasher.update(returned.block.0.to_le_bytes());
    hasher.update(returned.psi_return_edge.get().to_le_bytes());
    hasher.update(returned.instruction.0.to_le_bytes());
    hasher.update(returned.offset.to_le_bytes());
    hasher.update((returned.bytes.len() as u64).to_le_bytes());
    hasher.update(&returned.bytes);
    match &returned.value {
        WholeFunctionReturnValueEvidence::UnitV1 => hasher.update([1]),
        WholeFunctionReturnValueEvidence::ScalarI64V1 {
            virtual_register,
            view,
            units,
        } => {
            hasher.update([2]);
            hasher.update(virtual_register.0.to_le_bytes());
            hasher.update(view.0.to_le_bytes());
            encode_units(hasher, units);
        }
    }
    hasher.update([1]);
    match returned.mechanism {
        WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
            stack_pointer,
            read_bytes,
            pop_bytes,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(read_bytes.to_le_bytes());
            hasher.update(pop_bytes.to_le_bytes());
        }
        WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
            stack_pointer,
            link_register,
        } => {
            hasher.update([2]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(link_register.0.to_le_bytes());
        }
    }
}

fn encode_structural_fixup(hasher: &mut Sha256, fixup: X86_64StructuralUnitInternalControlFixup) {
    hasher.update([match fixup.kind {
        X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1 => 1,
    }]);
    hasher.update([match fixup.state {
        X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1 => 1,
    }]);
    hasher.update(fixup.callee.get().to_le_bytes());
    hasher.update(fixup.opcode_byte_offset.to_le_bytes());
    hasher.update(fixup.field_byte_offset.to_le_bytes());
    hasher.update(fixup.next_instruction_byte_offset.to_le_bytes());
    hasher.update([fixup.field_byte_width]);
    hasher.update(fixup.addend.to_le_bytes());
}

fn encode_target(hasher: &mut Sha256, target: NativeTarget) {
    hasher.update([match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }]);
    hasher.update([match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }]);
    hasher.update((target.pointer_size as u64).to_le_bytes());
    hasher.update((target.pointer_alignment as u64).to_le_bytes());
}

fn policy_tag(policy: WholeFunctionExitPolicy) -> u8 {
    match policy {
        WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1 => 1,
        WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1 => 2,
        WholeFunctionExitPolicy::Aapcs64FramelessLeafV1 => 3,
        WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1 => 4,
        WholeFunctionExitPolicy::MicrosoftX64BalancedStructuralUnitCallV1 => 5,
        WholeFunctionExitPolicy::MicrosoftX64FramelessStructuralUnitLeafV1 => 6,
        WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1 => 7,
        WholeFunctionExitPolicy::Aapcs64CanonicalFixedFrameV1 => 8,
        WholeFunctionExitPolicy::DarwinAapcs64CanonicalFixedFrameV1 => 9,
    }
}

fn encode_units(hasher: &mut Sha256, units: &[RegisterUnitId]) {
    hasher.update((units.len() as u64).to_le_bytes());
    for unit in units {
        hasher.update(unit.0.to_le_bytes());
    }
}
