//! Canonical version-9 exit-record identity; no producer or admission dependency.

use super::{
    WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
    WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionFrameDisposition,
    WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
};

const CONTRACT_SCHEMA: &[u8] = b"omega.terminal.whole-function-exit-contract.v14\0";

pub fn whole_function_exit_contract_identity(
    contract: &WholeFunctionExitContract,
) -> WholeFunctionExitContractIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(CONTRACT_SCHEMA);
    hasher.raw(&contract.selected.bytes());
    hasher.raw(&contract.post_allocation_manifest.bytes());
    hasher.raw(&contract.post_allocation_machine.bytes());
    hasher.raw(&contract.register_environment.bytes());
    hasher.raw(&contract.physical_register_model.bytes());
    hasher.raw(&contract.pre_layout.bytes());
    hasher.raw(&contract.resolved_layout.bytes());
    match contract.layout_custody {
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1 => hasher.byte(1),
        WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { relaxation } => {
            hasher.byte(2);
            hasher.raw(&relaxation.bytes());
        }
        WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
            optimization,
            artifact_identity,
        } => {
            hasher.byte(3);
            hasher.byte(optimization as u8);
            hasher.raw(&artifact_identity);
        }
    }
    hasher.raw(&contract.target.wire_identity_bytes());
    hasher.byte(exit_policy_tag(contract.policy));
    match contract.frame {
        WholeFunctionFrameDisposition::FramelessV1 => hasher.byte(1),
        WholeFunctionFrameDisposition::CanonicalFixedFrameV1 { layout, protocol } => {
            hasher.byte(2);
            hasher.raw(&layout.bytes());
            hasher.raw(&protocol.bytes());
        }
    }
    hasher.byte(1);
    match contract.entry_assumption {
        WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1 => {
            hasher.byte(1);
        }
        WholeFunctionEntryAssumption::CallerLinkRegisterV1 { link_register } => {
            hasher.byte(2);
            hasher.raw(&link_register.0.to_le_bytes());
        }
    }
    hasher.raw(&contract.stack_pointer.0.to_le_bytes());
    hasher.raw(&contract.stack_alignment.to_le_bytes());
    hasher.raw(&contract.red_zone_bytes.to_le_bytes());
    hasher.raw(&contract.result_view.0.to_le_bytes());
    encode_units(&mut hasher, &contract.callee_saved_units);
    hasher.u64(contract.functions.len() as u64);
    for function in contract.functions.iter() {
        hasher.raw(&function.machine.get().to_le_bytes());
        hasher.raw(&function.entry_block.0.to_le_bytes());
        hasher.raw(&function.body_stack_delta.to_le_bytes());
        encode_units(&mut hasher, &function.modified_callee_saved_units);
        hasher.u64(function.crashes.len() as u64);
        for crash in &function.crashes {
            hasher.raw(&crash.block.0.to_le_bytes());
            hasher.raw(&crash.psi_edge.get().to_le_bytes());
            hasher.byte(match crash.cause {
                terminal_psi::CrashCause::Trap => 1,
                terminal_psi::CrashCause::Abort => 2,
            });
            hasher.raw(&crash.instruction.0.to_le_bytes());
            hasher.raw(&crash.offset.to_le_bytes());
            hasher.u64(crash.bytes.len() as u64);
            hasher.raw(&crash.bytes);
        }
        hasher.u64(function.process_exits.len() as u64);
        for exited in &function.process_exits {
            hasher.raw(&exited.block.0.to_le_bytes());
            hasher.raw(&exited.completion_edge.get().to_le_bytes());
            hasher.raw(&exited.instruction.0.to_le_bytes());
            hasher.raw(&exited.offset.to_le_bytes());
            hasher.u64(exited.bytes.len() as u64);
            hasher.raw(&exited.bytes);
        }
        hasher.u64(function.returns.len() as u64);
        for returned in &function.returns {
            hasher.raw(&returned.block.0.to_le_bytes());
            hasher.raw(&returned.psi_return_edge.get().to_le_bytes());
            hasher.raw(&returned.instruction.0.to_le_bytes());
            hasher.raw(&returned.offset.to_le_bytes());
            hasher.u64(returned.bytes.len() as u64);
            hasher.raw(&returned.bytes);
            match &returned.value {
                WholeFunctionReturnValueEvidence::UnitV1 => hasher.byte(1),
                WholeFunctionReturnValueEvidence::AggregateV1 { fragments } => {
                    hasher.byte(3);
                    hasher.u64(fragments.len() as u64);
                    for fragment in fragments {
                        hasher.raw(&fragment.virtual_register.0.to_le_bytes());
                        hasher.raw(&fragment.view.0.to_le_bytes());
                        encode_units(&mut hasher, &fragment.units);
                    }
                }
                WholeFunctionReturnValueEvidence::ScalarV1 {
                    virtual_register,
                    view,
                    units,
                } => {
                    hasher.byte(2);
                    hasher.raw(&virtual_register.0.to_le_bytes());
                    hasher.raw(&view.0.to_le_bytes());
                    encode_units(&mut hasher, units);
                }
            }
            hasher.byte(1);
            match returned.mechanism {
                WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                    stack_pointer,
                    read_bytes,
                    pop_bytes,
                } => {
                    hasher.byte(1);
                    hasher.raw(&stack_pointer.0.to_le_bytes());
                    hasher.raw(&read_bytes.to_le_bytes());
                    hasher.raw(&pop_bytes.to_le_bytes());
                }
                WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
                    stack_pointer,
                    link_register,
                } => {
                    hasher.byte(2);
                    hasher.raw(&stack_pointer.0.to_le_bytes());
                    hasher.raw(&link_register.0.to_le_bytes());
                }
            }
        }
    }
    WholeFunctionExitContractIdentity::from_bytes(hasher.finish())
}

pub(crate) fn exit_policy_tag(policy: WholeFunctionExitPolicy) -> u8 {
    match policy {
        WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1 => 1,
        WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1 => 2,
        WholeFunctionExitPolicy::Aapcs64FramelessLeafV1 => 3,
        WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1 => 4,
        WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1 => 7,
        WholeFunctionExitPolicy::Aapcs64CanonicalFixedFrameV1 => 8,
        WholeFunctionExitPolicy::DarwinAapcs64CanonicalFixedFrameV1 => 9,
        WholeFunctionExitPolicy::MicrosoftX64CanonicalFixedFrameV1 => 11,
    }
}

use crate::machine_code::encode_units;
