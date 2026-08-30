use omega_isa_x86_64::{
    X86_64StructuralUnitInternalControlFixup, X86_64StructuralUnitInternalControlFixupKind,
    X86_64StructuralUnitInternalControlFixupState,
};
use omega_register_model::RegisterUnitId;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use sha2::{Digest, Sha256};

use super::model::{
    WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
    WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionReturnEvidence,
    WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
};

const CONTRACT_SCHEMA: &[u8] = b"omega.terminal.whole-function-exit-contract.v8\0";

pub(super) fn contract_identity(
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
    WholeFunctionExitContractIdentity(hasher.finalize().into())
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
    }
}

fn encode_units(hasher: &mut Sha256, units: &[RegisterUnitId]) {
    hasher.update((units.len() as u64).to_le_bytes());
    for unit in units {
        hasher.update(unit.0.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use omega_isa_x86_64::{
        X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET,
        X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET,
        X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
        X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT, X86_64StructuralUnitInternalControlFixup,
        X86_64StructuralUnitInternalControlFixupKind,
        X86_64StructuralUnitInternalControlFixupState,
    };
    use omega_machine_optimizer::Aarch64MovnMaterializationIdentity;
    use omega_optimization_core::Optimization;
    use omega_register_model::RegisterViewId;
    use omega_selected_instructions::{
        MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionPlanIdentity,
    };
    use omega_target::NativeTarget;
    use psi_core::{EdgeId, MachineId};

    use crate::{
        ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity,
        X86BranchRelaxationIdentity,
    };

    use super::super::model::{
        WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
        WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionHardeningPolicy,
        WholeFunctionReturnEvidence, WholeFunctionReturnMechanism,
        WholeFunctionReturnValueEvidence, WholeFunctionStructuralUnitCallEvidence,
        WholeFunctionStructuralUnitExitEvidence,
    };
    use super::contract_identity;

    fn contract_with_custody(
        layout_custody: WholeFunctionExitLayoutCustody,
    ) -> WholeFunctionExitContract {
        let mut contract = WholeFunctionExitContract {
            identity: WholeFunctionExitContractIdentity::from_bytes([0; 32]),
            selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            post_allocation_manifest:
                omega_optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                    [2; 32],
                ),
            post_allocation_machine:
                omega_machine_optimizer::PostAllocationMachineIdentity::from_bytes([3; 32]),
            register_environment:
                omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            physical_register_model:
                omega_register_model::PhysicalRegisterModelIdentity::from_bytes([5; 32]),
            pre_layout: SelectedFormEncodingIdentity::from_bytes([6; 32]),
            resolved_layout: ResolvedSelectedFormLayoutIdentity::from_bytes([7; 32]),
            layout_custody,
            target: NativeTarget::linux_x64(),
            policy: WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1,
            hardening: WholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1,
            entry_assumption: WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1,
            stack_pointer: RegisterViewId(0),
            stack_alignment: 16,
            red_zone_bytes: 128,
            result_view: RegisterViewId(1),
            callee_saved_units: Vec::new(),
            functions: Box::new(Vec::new()),
            structural_unit_functions: Box::new(Vec::new()),
        };
        contract.identity = contract_identity(&contract);
        contract
    }

    #[test]
    fn layout_custody_and_optimization_receipts_are_identity_bound() {
        let baseline = contract_with_custody(WholeFunctionExitLayoutCustody::BaselineNearLayoutV1);
        let relaxed = contract_with_custody(
            WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation: X86BranchRelaxationIdentity::from_bytes([8; 32]),
            },
        );
        let another_relaxation = contract_with_custody(
            WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation: X86BranchRelaxationIdentity::from_bytes([9; 32]),
            },
        );
        let movn = contract_with_custody(
            WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
                materialization: Aarch64MovnMaterializationIdentity::from_bytes([8; 32]),
            },
        );
        let another_movn = contract_with_custody(
            WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
                materialization: Aarch64MovnMaterializationIdentity::from_bytes([9; 32]),
            },
        );
        let generic_xor = contract_with_custody(
            WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
                optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
                artifact_identity: [8; 32],
            },
        );
        let another_generic_rule = contract_with_custody(
            WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
                optimization: Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                artifact_identity: [8; 32],
            },
        );
        let another_generic_leaf = contract_with_custody(
            WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
                optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
                artifact_identity: [9; 32],
            },
        );

        assert_ne!(baseline.identity, relaxed.identity);
        assert_ne!(relaxed.identity, another_relaxation.identity);
        assert_ne!(baseline.identity, movn.identity);
        assert_ne!(relaxed.identity, movn.identity);
        assert_ne!(movn.identity, another_movn.identity);
        assert_ne!(baseline.identity, generic_xor.identity);
        assert_ne!(generic_xor.identity, another_generic_rule.identity);
        assert_ne!(generic_xor.identity, another_generic_leaf.identity);
    }

    #[test]
    fn structural_call_frame_fixup_and_returns_are_identity_bound() {
        let caller = MachineId::new(1).unwrap();
        let leaf = MachineId::new(2).unwrap();
        let mut contract =
            contract_with_custody(WholeFunctionExitLayoutCustody::BaselineNearLayoutV1);
        contract.target = NativeTarget::uefi_x64();
        contract.policy = WholeFunctionExitPolicy::MicrosoftX64BalancedStructuralUnitCallV1;
        contract.red_zone_bytes = 0;
        let mut call_bytes = vec![0; X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT];
        call_bytes[usize::from(X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET)] = 0xe8;
        let returned = |instruction, offset, edge| WholeFunctionReturnEvidence {
            block: SelectedBlockId(0),
            psi_return_edge: EdgeId::new(edge).unwrap(),
            instruction: SelectedInstructionId(instruction),
            offset,
            bytes: vec![0xc3],
            value: WholeFunctionReturnValueEvidence::UnitV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            mechanism: WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                stack_pointer: contract.stack_pointer,
                read_bytes: 8,
                pop_bytes: 8,
            },
        };
        *contract.structural_unit_functions = vec![
            WholeFunctionStructuralUnitExitEvidence {
                machine: caller,
                entry_block: SelectedBlockId(0),
                body_stack_delta: 0,
                modified_callee_saved_units: Vec::new(),
                call: Some(WholeFunctionStructuralUnitCallEvidence {
                    block: SelectedBlockId(0),
                    instruction: SelectedInstructionId(0),
                    operation: psi_core::OperationId::new(3).unwrap(),
                    callee: leaf,
                    offset: 0,
                    bytes: call_bytes,
                    fixup: X86_64StructuralUnitInternalControlFixup {
                        kind: X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1,
                        state: X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1,
                        callee: leaf,
                        opcode_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET,
                        field_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET,
                        next_instruction_byte_offset:
                            X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET,
                        field_byte_width: X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
                        addend: 0,
                    },
                    unit_uses: Vec::new(),
                    unit_defs: Vec::new(),
                    unit_clobbers: Vec::new(),
                    frame_byte_count: 72,
                    shadow_byte_count: 32,
                    pre_call_stack_alignment: 16,
                    frame_is_balanced: true,
                }),
                returned: returned(1, 89, 4),
            },
            WholeFunctionStructuralUnitExitEvidence {
                machine: leaf,
                entry_block: SelectedBlockId(0),
                body_stack_delta: 0,
                modified_callee_saved_units: Vec::new(),
                call: None,
                returned: returned(0, 0, 5),
            },
        ];
        contract.identity = contract_identity(&contract);

        let mut changed_frame = contract.clone();
        changed_frame.structural_unit_functions[0]
            .call
            .as_mut()
            .unwrap()
            .frame_byte_count = 71;
        changed_frame.identity = contract_identity(&changed_frame);
        let mut changed_fixup = contract.clone();
        changed_fixup.structural_unit_functions[0]
            .call
            .as_mut()
            .unwrap()
            .fixup
            .field_byte_offset += 1;
        changed_fixup.identity = contract_identity(&changed_fixup);
        let mut changed_return = contract.clone();
        changed_return.structural_unit_functions[0].returned.offset -= 1;
        changed_return.identity = contract_identity(&changed_return);
        let mut structural_leaf = contract.clone();
        structural_leaf.policy = WholeFunctionExitPolicy::MicrosoftX64FramelessStructuralUnitLeafV1;
        *structural_leaf.structural_unit_functions =
            vec![structural_leaf.structural_unit_functions[1].clone()];
        structural_leaf.identity = contract_identity(&structural_leaf);

        assert_ne!(contract.identity, changed_frame.identity);
        assert_ne!(contract.identity, changed_fixup.identity);
        assert_ne!(contract.identity, changed_return.identity);
        assert_ne!(contract.identity, structural_leaf.identity);
    }
}
