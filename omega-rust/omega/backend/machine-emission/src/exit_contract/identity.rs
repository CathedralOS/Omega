//! Exit-record identity is representation-owned; these controls use target fixtures.

pub(super) use machine_code::whole_function_exit_contract_identity as contract_identity;

#[cfg(test)]
mod tests {
    use optimization_core::Optimization;
    use physical_instructions::Aarch64MovnMaterializationIdentity;
    use register_model::RegisterViewId;
    use selected_instructions::SelectedInstructionPlanIdentity;
    use target::NativeTarget;

    use machine_code::{
        ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity,
        X86BranchRelaxationIdentity,
    };

    use super::super::model::{
        WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
        WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionFrameDisposition,
        WholeFunctionHardeningPolicy,
    };
    use super::contract_identity;

    fn contract_with_custody(
        layout_custody: WholeFunctionExitLayoutCustody,
    ) -> WholeFunctionExitContract {
        let mut contract = WholeFunctionExitContract {
            identity: WholeFunctionExitContractIdentity::from_bytes([0; 32]),
            selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            post_allocation_manifest:
                optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes([2; 32]),
            post_allocation_machine:
                physical_instructions::PostAllocationMachineIdentity::from_bytes([3; 32]),
            register_environment: register_model::TargetRegisterEnvironmentIdentity::from_bytes(
                [4; 32],
            ),
            physical_register_model: register_model::PhysicalRegisterModelIdentity::from_bytes(
                [5; 32],
            ),
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
            layout: machine_code::TargetFrameLayoutIdentity::from_bytes([10; 32]),
            protocol: machine_code::TargetFrameProtocolEncodingIdentity::from_bytes([11; 32]),
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
        let mut windows = framed.clone();
        windows.policy = WholeFunctionExitPolicy::MicrosoftX64CanonicalFixedFrameV1;
        assert_ne!(contract_identity(&windows), framed.identity);
    }
}
