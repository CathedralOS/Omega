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

pub mod identity;
pub mod publication;

use abstract_operations::ValueBinding;
use optimization_core::FunctionFragmentEmissionIdentity;
use optimization_unit::WorkSettlement;
use selected_instructions::{
    MachineAlternativeKey, SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance,
};
use semantic_vocabulary::{BlockId, BoundaryMachineId, EdgeId, MachineId, WorkScheduleIdentity};
use target::NativeTarget;
use target_operations::TerminalPsiProvenance;
use terminal_psi::TerminalPsiIdentity;

use crate::layout::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
};
use crate::{SelectedFormInternalMachineFixupKind, SelectedFormNormalizedForeignCallFixupKind};

pub(crate) use identity::function_fragment_emission_identity;
pub use publication::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionManifestDecodeError,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentEmissionPlan {
    pub identity: FunctionFragmentEmissionIdentity,
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

    pub fn recomputed_identity(&self) -> FunctionFragmentEmissionIdentity {
        function_fragment_emission_identity(self)
    }
}

pub mod control_flow {
    use super::*;

    pub type FunctionFragmentBranchEvidence = ResolvedBranchEvidence;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum FunctionFragmentControlProvenance {
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
            owner: target_operations::CallSiteOwner,
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
        pub nominal_cleanup_subject: Option<semantic_vocabulary::StructuralTypeId>,
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

#[cfg(test)]
mod tests {
    use super::{
        FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentControlProvenance,
        FunctionFragmentEmissionPlan, FunctionFragmentInstructionSpan,
    };
    use optimization_core::FunctionFragmentEmissionIdentity;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    };
    use semantic_vocabulary::{MachineId, ValueId, WorkScheduleIdentity};
    use target::NativeTarget;
    use target_operations::TerminalPsiProvenance;
    use terminal_psi::TerminalPsiIdentity;

    fn zero_span_plan() -> FunctionFragmentEmissionPlan {
        let mut plan = FunctionFragmentEmissionPlan {
            identity: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
            psi: TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
            },
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
            target: NativeTarget::linux_x64(),
            entry: MachineId::new(1).unwrap(),
            functions: vec![FunctionFragment {
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                nominal_cleanup_subject: None,
                provenance: TerminalPsiProvenance::default(),
                byte_count: 0,
                bytes: Vec::new(),
                blocks: vec![FunctionFragmentBlockSpan {
                    block: SelectedBlockId(0),
                    offset: 0,
                    byte_count: 0,
                    instructions: vec![FunctionFragmentInstructionSpan {
                        instruction: SelectedInstructionId(0),
                        alternative: MachineAlternativeKey {
                            family: MachineAlternativeFamily::CompareI64Zero,
                            variant: 0,
                        },
                        offset: 0,
                        bytes: Vec::new(),
                        branch: None,
                        internal_machine_fixup: None,
                        normalized_foreign_call_fixup: None,
                        image_address_fixup: None,
                        provenance: SelectedInstructionProvenance::default(),
                        control: FunctionFragmentControlProvenance::None,
                    }],
                }],
            }],
        };
        plan.identity = plan.recomputed_identity();
        plan
    }

    #[test]
    fn fragment_identity_binds_zero_spans_aggregate_bytes_and_provenance() {
        let original = zero_span_plan();
        assert_eq!(original.identity, original.recomputed_identity());

        let mut changed = original.clone();
        changed.functions[0].bytes.push(0x90);
        assert_ne!(changed.recomputed_identity(), original.identity);

        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .provenance
            .values
            .push(ValueId::new(7).unwrap());
        assert_ne!(changed.recomputed_identity(), original.identity);

        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions.clear();
        assert_ne!(changed.recomputed_identity(), original.identity);
    }
}
