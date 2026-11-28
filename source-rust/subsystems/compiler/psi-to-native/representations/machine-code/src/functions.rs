/*
Whole-function exit contracts (`WholeFunctionExitContract`).

Every emitted machine function must account for how its frame, stack delta, callee-saved register
units, returns (`WholeFunctionReturnEvidence`), process exits (`WholeFunctionProcessExitEvidence`),
and traps (`WholeFunctionCrashEvidence`) terminate execution under the target ABI's entry assumption
and frame disposition (`WholeFunctionFrameDisposition`). Recording this exit contract alongside its
upstream layout custody (`WholeFunctionExitLayoutCustody`) lets fragment and text-section admission
verify return/epilogue correspondence without re-running frame layout synthesis. The contract
holds the machine plan, resolved layout, frame plan and frame protocol it was built from; checks
compare those plans and the contract itself, not a digest of it.
*/

use register_model::{RegisterUnitId, RegisterViewId};
use selected_instructions::{
    MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstructionId, VirtualRegisterId,
};
use semantic_vocabulary::{EdgeId, MachineId};
use target::NativeTarget;

use std::sync::Arc;

use crate::{
    ResolvedMachineLayout, SelectedFormEncodingHandle, TargetFrameLayoutPlan,
    TargetFrameProtocolEncodingPlan, X86BranchRelaxationIdentity,
};

pub mod exit_contract {
    pub use super::{
        WholeFunctionCrashEvidence, WholeFunctionEntryAssumption, WholeFunctionExitContract,
        WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy,
        WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy,
        WholeFunctionProcessExitEvidence, WholeFunctionReturnEvidence,
        WholeFunctionReturnFragmentEvidence, WholeFunctionReturnMechanism,
        WholeFunctionReturnValueEvidence,
    };
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WholeFunctionFrameDisposition {
    FramelessV1,
    CanonicalFixedFrameV1 {
        layout: Arc<TargetFrameLayoutPlan>,
        protocol: Arc<TargetFrameProtocolEncodingPlan>,
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
    pub post_allocation_machine: std::sync::Arc<post_allocation_machine::PostAllocationMachinePlan>,
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub physical_register_model: register_model::PhysicalRegisterModelIdentity,
    pub pre_layout: SelectedFormEncodingHandle,
    pub resolved_layout: Arc<ResolvedMachineLayout>,
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
}
