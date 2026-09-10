//! Whole-function exit records. Content identity does not grant admission.

mod identity;
pub use identity::whole_function_exit_contract_identity;

use optimization_core::Optimization;
use physical_instructions::{Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity};
use register_model::{RegisterUnitId, RegisterViewId};
use selected_instructions::{
    MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::{EdgeId, MachineId};
use target::NativeTarget;

use crate::{
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity, TargetFrameLayoutIdentity,
    TargetFrameProtocolEncodingIdentity, X86BranchRelaxationIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WholeFunctionExitContractIdentity([u8; 32]);

impl WholeFunctionExitContractIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
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
    /// Canonical Microsoft-x64 frame with exact outgoing ABI and preservation storage.
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

/// Claimed replay role and input identity for the final function-relative layout.
/// Admission requires a baseline layout for the baseline role and independently
/// replayed relaxation for the relaxation role. Post-allocation transformations
/// share one normalized custody shape; their owning typed result remains
/// mandatory at admission. Constructing this record does not perform those checks.
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
    /// Compatibility custody for the named CBNZ realization route. New
    /// generic callers must use `PostAllocationMachineOptimizationV1`.
    Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
        fusion: Aarch64CbnzFusionIdentity,
    },
    /// Compatibility custody for the named MOVN realization route. New
    /// generic callers must use `PostAllocationMachineOptimizationV1`.
    Aarch64SelectShortestMovnSeededI64MaterializationV1 {
        materialization: Aarch64MovnMaterializationIdentity,
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
    /// Ordered direct ABI fragments; exact source/result storage is retained by selection.
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
    /// Retained source edge, not a physical return or successful termination claim.
    pub nominal_return_edge: EdgeId,
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
    /// Recompute the versioned content identity, without checking the claim.
    pub fn recomputed_identity(&self) -> WholeFunctionExitContractIdentity {
        whole_function_exit_contract_identity(self)
    }
}
