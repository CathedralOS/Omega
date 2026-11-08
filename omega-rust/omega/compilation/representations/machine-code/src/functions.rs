/*
Whole-function exit contracts (`WholeFunctionExitContract`) and their canonical SHA-256 identity
(`WholeFunctionExitContractIdentity`, schema `omega.terminal.whole-function-exit-contract.v14\0`).

Every emitted machine function must account for how its frame, stack delta, callee-saved register
units, returns (`WholeFunctionReturnEvidence`), process exits (`WholeFunctionProcessExitEvidence`),
and traps (`WholeFunctionCrashEvidence`) terminate execution under the target ABI's entry assumption
and frame disposition (`WholeFunctionFrameDisposition`). Recording this exit contract alongside its
upstream layout custody (`WholeFunctionExitLayoutCustody`) lets fragment and text-section admission
verify return/epilogue correspondence without re-running frame layout synthesis.
*/

use optimization_core::Optimization;
use register_model::{RegisterUnitId, RegisterViewId};
use selected_instructions::{
    MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::{EdgeId, MachineId};
use target::NativeTarget;

use crate::{
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity, TargetFrameLayoutIdentity,
    TargetFrameProtocolEncodingIdentity, X86BranchRelaxationIdentity, encode_units,
};

pub mod exit_contract {
    pub use super::{
        WholeFunctionCrashEvidence, WholeFunctionEntryAssumption, WholeFunctionExitContract,
        WholeFunctionExitContractIdentity, WholeFunctionExitEvidence,
        WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionFrameDisposition,
        WholeFunctionHardeningPolicy, WholeFunctionProcessExitEvidence,
        WholeFunctionReturnEvidence, WholeFunctionReturnFragmentEvidence,
        WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
        whole_function_exit_contract_identity,
    };
}

register_homes::identity_newtype! { pub struct WholeFunctionExitContractIdentity([u8; 32]); }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholeFunctionExitPolicy {
    SystemVAMD64FramelessLeafV1,
    MicrosoftX64FramelessLeafV1,
    Aapcs64FramelessLeafV1,
    DarwinAapcs64FramelessLeafV1,
    SystemVAMD64CanonicalFixedFrameV1,
    Aapcs64CanonicalFixedFrameV1,
    DarwinAapcs64CanonicalFixedFrameV1,
    MicrosoftX64CanonicalFixedFrameV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholeFunctionFrameDisposition {
    FramelessV1,
    CanonicalFixedFrameV1 {
        layout: TargetFrameLayoutIdentity,
        protocol: TargetFrameProtocolEncodingIdentity,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholeFunctionHardeningPolicy {
    NoAdditionalEntryExitHardeningV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholeFunctionExitLayoutCustody {
    BaselineNearLayoutV1,
    X86RelaxConditionalBranchesToRel8V1 {
        relaxation: X86BranchRelaxationIdentity,
    },
    PostAllocationMachineOptimizationV1 {
        optimization: Optimization,
        artifact_identity: [u8; 32],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholeFunctionEntryAssumption {
    CallerReturnAddressAtStackPointerV1,
    CallerLinkRegisterV1 { link_register: RegisterViewId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WholeFunctionReturnMechanism {
    X86ActivationStackReturnV1 {
        stack_pointer: RegisterViewId,
        read_bytes: u16,
        pop_bytes: u16,
    },
    Aarch64LinkRegisterReturnV1 {
        stack_pointer: RegisterViewId,
        link_register: RegisterViewId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WholeFunctionReturnValueEvidence {
    UnitV1,
    AggregateV1 {
        fragments: Vec<WholeFunctionReturnFragmentEvidence>,
    },
    ScalarV1 {
        virtual_register: VirtualRegisterId,
        view: RegisterViewId,
        units: Vec<RegisterUnitId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionReturnFragmentEvidence {
    pub virtual_register: VirtualRegisterId,
    pub view: RegisterViewId,
    pub units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionReturnEvidence {
    pub block: SelectedBlockId,
    pub psi_return_edge: EdgeId,
    pub instruction: SelectedInstructionId,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub value: WholeFunctionReturnValueEvidence,
    pub trap: MachineEncodedTrapBehavior,
    pub mechanism: WholeFunctionReturnMechanism,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionProcessExitEvidence {
    pub block: SelectedBlockId,
    pub completion_edge: EdgeId,
    pub instruction: SelectedInstructionId,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionCrashEvidence {
    pub block: SelectedBlockId,
    pub psi_edge: EdgeId,
    pub cause: terminal_psi::CrashCause,
    pub instruction: SelectedInstructionId,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionExitEvidence {
    pub machine: MachineId,
    pub entry_block: SelectedBlockId,
    pub body_stack_delta: i64,
    pub modified_callee_saved_units: Vec<RegisterUnitId>,
    pub returns: Vec<WholeFunctionReturnEvidence>,
    pub process_exits: Vec<WholeFunctionProcessExitEvidence>,
    pub crashes: Vec<WholeFunctionCrashEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionExitContract {
    pub identity: WholeFunctionExitContractIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub post_allocation_manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: physical_instructions::PostAllocationMachineIdentity,
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub physical_register_model: register_model::PhysicalRegisterModelIdentity,
    pub pre_layout: SelectedFormEncodingIdentity,
    pub resolved_layout: ResolvedSelectedFormLayoutIdentity,
    pub layout_custody: WholeFunctionExitLayoutCustody,
    pub target: NativeTarget,
    pub policy: WholeFunctionExitPolicy,
    pub frame: WholeFunctionFrameDisposition,
    pub hardening: WholeFunctionHardeningPolicy,
    pub entry_assumption: WholeFunctionEntryAssumption,
    pub stack_pointer: RegisterViewId,
    pub stack_alignment: u16,
    pub red_zone_bytes: u16,
    pub result_view: RegisterViewId,
    pub callee_saved_units: Vec<RegisterUnitId>,
    pub functions: Box<Vec<WholeFunctionExitEvidence>>,
}

impl WholeFunctionExitContract {
    pub fn function(&self, machine: MachineId) -> Option<&WholeFunctionExitEvidence> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

    pub fn recomputed_identity(&self) -> WholeFunctionExitContractIdentity {
        whole_function_exit_contract_identity(self)
    }
}

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
            hasher.bytes(&crash.bytes);
        }
        hasher.u64(function.process_exits.len() as u64);
        for exited in &function.process_exits {
            hasher.raw(&exited.block.0.to_le_bytes());
            hasher.raw(&exited.completion_edge.get().to_le_bytes());
            hasher.raw(&exited.instruction.0.to_le_bytes());
            hasher.raw(&exited.offset.to_le_bytes());
            hasher.bytes(&exited.bytes);
        }
        hasher.u64(function.returns.len() as u64);
        for returned in &function.returns {
            hasher.raw(&returned.block.0.to_le_bytes());
            hasher.raw(&returned.psi_return_edge.get().to_le_bytes());
            hasher.raw(&returned.instruction.0.to_le_bytes());
            hasher.raw(&returned.offset.to_le_bytes());
            hasher.bytes(&returned.bytes);
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
