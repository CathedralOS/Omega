//! Exit-record identity is representation-owned; these controls use target fixtures.

pub(super) use omega_machine_code::whole_function_exit_contract_identity as contract_identity;

#[cfg(test)]
mod tests {
    use omega_isa_x86_64::{
        X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET,
        X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET,
        X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
        X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT,
    };
    use omega_machine_code::{
        X86_64StructuralUnitInternalControlFixup, X86_64StructuralUnitInternalControlFixupKind,
        X86_64StructuralUnitInternalControlFixupState,
    };
    use omega_optimization_core::Optimization;
    use omega_physical_instructions::Aarch64MovnMaterializationIdentity;
    use omega_register_model::RegisterViewId;
    use omega_selected_instructions::{
        MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionPlanIdentity,
    };
    use omega_target::NativeTarget;
    use psi_core::{EdgeId, MachineId};

    use omega_machine_code::{
        ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity,
        X86BranchRelaxationIdentity,
    };

    use super::super::model::{
        WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
        WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionFrameDisposition,
        WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence, WholeFunctionReturnMechanism,
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
                omega_physical_instructions::PostAllocationMachineIdentity::from_bytes([3; 32]),
            register_environment:
                omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            physical_register_model:
                omega_register_model::PhysicalRegisterModelIdentity::from_bytes([5; 32]),
            pre_layout: SelectedFormEncodingIdentity::from_bytes([6; 32]),
            resolved_layout: ResolvedSelectedFormLayoutIdentity::from_bytes([7; 32]),
            layout_custody,
            target: NativeTarget::linux_x64(),
            policy: WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1,
            frame: WholeFunctionFrameDisposition::FramelessV1,
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
        let mut framed = baseline.clone();
        framed.frame = WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
            layout: omega_machine_code::TargetFrameLayoutIdentity::from_bytes([10; 32]),
            protocol: omega_machine_code::TargetFrameProtocolEncodingIdentity::from_bytes([11; 32]),
        };
        framed.policy = WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1;
        framed.identity = contract_identity(&framed);

        assert_ne!(baseline.identity, relaxed.identity);
        assert_ne!(relaxed.identity, another_relaxation.identity);
        assert_ne!(baseline.identity, movn.identity);
        assert_ne!(relaxed.identity, movn.identity);
        assert_ne!(movn.identity, another_movn.identity);
        assert_ne!(baseline.identity, generic_xor.identity);
        assert_ne!(generic_xor.identity, another_generic_rule.identity);
        assert_ne!(generic_xor.identity, another_generic_leaf.identity);
        assert_ne!(baseline.identity, framed.identity);
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
