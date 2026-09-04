use omega_isa_x86_64::X86_64StructuralUnitInternalControlFixup;
use omega_machine_optimizer::{Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity};
use omega_optimization_core::Optimization;
use omega_register_model::{RegisterUnitId, RegisterViewId};
use omega_selected_instructions::{
    MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use omega_target::NativeTarget;
use psi_core::{EdgeId, MachineId};

use crate::{
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity, TargetFrameLayoutIdentity,
    TargetFrameProtocolEncodingIdentity, X86BranchRelaxationIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WholeFunctionExitContractIdentity(pub(super) [u8; 32]);

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
    /// Exact Microsoft-x64 custody for one balanced structural Unit caller
    /// and its Unit leaf. This is deliberately not a frameless-leaf policy:
    /// the caller owns a canonical 72-byte outgoing frame around its call.
    MicrosoftX64BalancedStructuralUnitCallV1,
    /// Exact Microsoft-x64 custody for one structural-signature Unit leaf.
    /// The function owns no call frame and consists solely of its validated
    /// `ReturnUnit` encoding.
    MicrosoftX64FramelessStructuralUnitLeafV1,
    SystemVAMD64CanonicalFixedFrameV1,
    Aapcs64CanonicalFixedFrameV1,
    DarwinAapcs64CanonicalFixedFrameV1,
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

/// Exact authority under which the final function-relative layout entered
/// whole-function exit validation. The baseline variant never admits a
/// transformed layout; the relaxation variant is available only through the
/// dedicated independently replayed x86 relaxation API. Post-allocation
/// transformations share one normalized custody shape; their owning typed
/// result remains mandatory at the canonical admission API.
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
    ScalarI64V1 {
        virtual_register: VirtualRegisterId,
        view: RegisterViewId,
        units: Vec<RegisterUnitId>,
    },
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
pub struct WholeFunctionExitEvidence {
    pub machine: MachineId,
    pub entry_block: SelectedBlockId,
    pub body_stack_delta: i64,
    pub modified_callee_saved_units: Vec<RegisterUnitId>,
    pub returns: Vec<WholeFunctionReturnEvidence>,
}

/// Whole-function evidence for the one atomic structural Unit call bundle.
/// The rel32 remains a typed unresolved fixup until whole-text placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionStructuralUnitCallEvidence {
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub operation: psi_core::OperationId,
    pub callee: MachineId,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub fixup: X86_64StructuralUnitInternalControlFixup,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
    pub frame_byte_count: u32,
    pub shadow_byte_count: u32,
    pub pre_call_stack_alignment: u16,
    pub frame_is_balanced: bool,
}

/// Parallel custody for the bounded zero-VReg structural Unit roster. Keeping
/// this distinct prevents its function-local instruction IDs from colliding
/// with ordinary rows or with the other structural function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionStructuralUnitExitEvidence {
    pub machine: MachineId,
    pub entry_block: SelectedBlockId,
    pub body_stack_delta: i64,
    pub modified_callee_saved_units: Vec<RegisterUnitId>,
    pub call: Option<WholeFunctionStructuralUnitCallEvidence>,
    pub returned: WholeFunctionReturnEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionExitContract {
    pub identity: WholeFunctionExitContractIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub post_allocation_manifest:
        omega_optimization_core::PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: omega_machine_optimizer::PostAllocationMachineIdentity,
    pub register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    pub physical_register_model: omega_register_model::PhysicalRegisterModelIdentity,
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
    /// These rosters stay heap-owned because the validated contract is nested
    /// in several owning pipeline carriers; adding structural evidence must
    /// not inflate every ordinary carrier's stack frame.
    pub functions: Box<Vec<WholeFunctionExitEvidence>>,
    /// Parallel to `functions`; never merged by function-local instruction ID.
    pub structural_unit_functions: Box<Vec<WholeFunctionStructuralUnitExitEvidence>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedWholeFunctionExitContract {
    pub(super) contract: WholeFunctionExitContract,
}

impl ValidatedWholeFunctionExitContract {
    pub const fn contract(&self) -> &WholeFunctionExitContract {
        &self.contract
    }

    pub const fn identity(&self) -> WholeFunctionExitContractIdentity {
        self.contract.identity
    }

    #[cfg(test)]
    pub(crate) fn contract_mut(&mut self) -> &mut WholeFunctionExitContract {
        &mut self.contract
    }

    /// Test-only rel8 custody mutation with a valid enclosing identity. This
    /// grants no production construction, validation, or publication authority.
    #[cfg(test)]
    pub(crate) fn corrupt_rel8_boundary_and_reauthenticate_for_test(
        &mut self,
        boundary: Rel8ExitBoundaryForTest,
    ) {
        match boundary {
            Rel8ExitBoundaryForTest::LayoutCustody => {
                self.contract.layout_custody =
                    WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                        relaxation: X86BranchRelaxationIdentity::from_bytes([0xb1; 32]),
                    };
            }
            Rel8ExitBoundaryForTest::ResolvedLayout => {
                self.contract.resolved_layout =
                    ResolvedSelectedFormLayoutIdentity::from_bytes([0xb2; 32]);
            }
        }
        self.contract.identity = super::identity::contract_identity(&self.contract);
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum Rel8ExitBoundaryForTest {
    ResolvedLayout,
    LayoutCustody,
}
