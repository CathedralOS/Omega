/*
Unplaced function fragments (`FunctionFragmentEmissionPlan`, `FunctionFragment`), block/instruction
spans (`FunctionFragmentBlockSpan`, `FunctionFragmentInstructionSpan`), and function-relative
control/fixup records.

Each `FunctionFragment` holds the emitted bytes of one machine function in function-relative
coordinates before whole-text-section concatenation, along with its block/instruction spans,
resolved intra-function branch evidence (`FunctionFragmentBranchEvidence`), semantic successor
provenance (`FunctionFragmentControlProvenance`), and unresolved internal-machine, normalized-foreign,
and image-address fixups.
*/

use operation_evidence::provenance::WorkSettlement;
use operation_operands::values::ValueBinding;
use selected_instructions::{
    MachineAlternativeKey, SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance,
};
use semantic_vocabulary::{BlockId, BoundaryMachineId, EdgeId, MachineId, WorkScheduleIdentity};
use target::NativeTarget;
use target_vocabulary::TerminalPsiProvenance;
use terminal_psi::TerminalPsiIdentity;

use crate::layout::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
};
use crate::{SelectedFormInternalMachineFixupKind, SelectedFormNormalizedForeignCallFixupKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentEmissionPlan {
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<FunctionFragment>,
}

impl FunctionFragmentEmissionPlan {
    pub fn function(&self, machine: MachineId) -> Option<&FunctionFragment> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

    pub fn blocks(&self) -> impl Iterator<Item = &FunctionFragmentBlockSpan> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }

    pub fn instructions(&self) -> impl Iterator<Item = &FunctionFragmentInstructionSpan> + '_ {
        self.functions
            .iter()
            .flat_map(|function| function.instructions())
    }
}

pub mod control_flow {
    use super::*;

    pub type FunctionFragmentBranchEvidence = ResolvedBranchEvidence;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum FunctionFragmentControlProvenance {
        EliminateEmpty {
            psi_edge: EdgeId,
        },
        OperationTrap {
            operation: semantic_vocabulary::OperationId,
        },
        Crash {
            psi_edge: EdgeId,
            cause: terminal_psi::CrashCause,
        },
        HostedExitProcess {
            completion_edge: EdgeId,
        },
        None,
        Jump {
            successor: Box<FunctionFragmentSuccessorProvenance>,
        },
        InternalCall {
            target: selected_instructions::SelectedCallTarget,
            owner: target_vocabulary::CallSiteOwner,
        },
        NormalizedForeignCall {
            invocation_site: semantic_vocabulary::InvocationSite,
            boundary: BoundaryMachineId,
            ordinal: u32,
        },
        ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate,
            when_taken: Box<FunctionFragmentSuccessorProvenance>,
            when_fallthrough: Box<FunctionFragmentSuccessorProvenance>,
        },
        Return {
            psi_return_edge: EdgeId,
        },
    }

    pub type FunctionFragmentConditionalBranchPredicate = ResolvedConditionalBranchPredicate;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FunctionFragmentSuccessorProvenance {
        pub role: selected_instructions::SelectedSuccessorRole,
        pub psi_edge: Option<EdgeId>,
        pub block: SelectedBlockId,
        pub source_target: BlockId,
        pub bindings: Vec<ValueBinding>,
        pub work: Vec<WorkSettlement>,
    }

    pub type FunctionFragmentConditionalBranchEvidence = ResolvedConditionalBranchEvidence;
}

pub mod fixups {
    use super::*;

    pub type FunctionFragmentInternalMachineFixupKind = SelectedFormInternalMachineFixupKind;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FunctionFragmentInternalMachineFixupState {
        UnresolvedZeroFieldV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FunctionFragmentInternalMachineFixup {
        pub kind: FunctionFragmentInternalMachineFixupKind,
        pub state: FunctionFragmentInternalMachineFixupState,
        pub callee: MachineId,
        pub opcode_function_offset: u64,
        pub patch_function_offset: u64,
        pub reference_function_offset: u64,
        pub patch_byte_width: u8,
        pub addend: i64,
    }

    pub type FunctionFragmentNormalizedForeignCallFixupKind =
        SelectedFormNormalizedForeignCallFixupKind;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FunctionFragmentNormalizedForeignCallFixupState {
        UnresolvedImportFieldV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FunctionFragmentCallbackMaterializationEncoding {
        X86_64Relative32 {
            patch_function_offset: u64,
        },
        Aarch64PageAddress {
            page_patch_function_offset: u64,
            page_offset_patch_function_offset: u64,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FunctionFragmentCallbackMaterialization {
        pub destination: crate::CallbackAddressDestination,
        pub code_function_offset: u64,
        pub code_byte_count: u64,
        pub encoding: FunctionFragmentCallbackMaterializationEncoding,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FunctionFragmentNormalizedForeignCallFixup {
        pub kind: FunctionFragmentNormalizedForeignCallFixupKind,
        pub state: FunctionFragmentNormalizedForeignCallFixupState,
        pub boundary: BoundaryMachineId,
        pub ordinal: u32,
        pub opcode_function_offset: u64,
        pub patch_function_offset: u64,
        pub reference_function_offset: u64,
        pub patch_byte_width: u8,
        pub addend: i64,
        pub callbacks: Vec<FunctionFragmentCallbackMaterialization>,
    }
}

pub mod functions {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FunctionFragment {
        pub machine: MachineId,
        pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
        pub provenance: TerminalPsiProvenance,
        pub byte_count: u64,
        pub bytes: Vec<u8>,
        pub blocks: Vec<FunctionFragmentBlockSpan>,
    }

    impl FunctionFragment {
        pub fn instructions(&self) -> impl Iterator<Item = &FunctionFragmentInstructionSpan> + '_ {
            self.blocks.iter().flat_map(|block| &block.instructions)
        }

        pub fn instruction_span(
            &self,
            instruction: SelectedInstructionId,
        ) -> Option<&FunctionFragmentInstructionSpan> {
            self.instructions()
                .find(|span| span.instruction == instruction)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FunctionFragmentBlockSpan {
        pub block: SelectedBlockId,
        pub offset: u64,
        pub byte_count: u64,
        pub instructions: Vec<FunctionFragmentInstructionSpan>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FunctionFragmentInstructionSpan {
        pub instruction: SelectedInstructionId,
        pub alternative: MachineAlternativeKey,
        pub offset: u64,
        pub bytes: Vec<u8>,
        pub branch: Option<Box<super::FunctionFragmentBranchEvidence>>,
        pub internal_machine_fixup: Option<Box<FunctionFragmentInternalMachineFixup>>,
        pub normalized_foreign_call_fixup: Option<Box<FunctionFragmentNormalizedForeignCallFixup>>,
        pub image_address_fixup: Option<Box<crate::FunctionFragmentImageAddressFixup>>,
        pub provenance: SelectedInstructionProvenance,
        pub control: FunctionFragmentControlProvenance,
    }

    impl FunctionFragmentInstructionSpan {
        pub fn internal_machine_fixup(&self) -> Option<FunctionFragmentInternalMachineFixup> {
            self.internal_machine_fixup.as_deref().copied()
        }

        pub fn normalized_foreign_call_fixup(
            &self,
        ) -> Option<&FunctionFragmentNormalizedForeignCallFixup> {
            self.normalized_foreign_call_fixup.as_deref()
        }

        pub fn image_address_fixup(&self) -> Option<crate::FunctionFragmentImageAddressFixup> {
            self.image_address_fixup.as_deref().copied()
        }
    }
}

pub use control_flow::{
    FunctionFragmentBranchEvidence, FunctionFragmentConditionalBranchEvidence,
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentControlProvenance,
    FunctionFragmentSuccessorProvenance,
};
pub use fixups::{
    FunctionFragmentCallbackMaterialization, FunctionFragmentCallbackMaterializationEncoding,
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState, FunctionFragmentNormalizedForeignCallFixup,
    FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState,
};
pub use functions::{FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentInstructionSpan};
