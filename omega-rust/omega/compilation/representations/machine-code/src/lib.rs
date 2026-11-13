#![forbid(unsafe_code)]

/*
Target machine-code representations for the Omega native backend (`Omega 07–09 -> Machine Emission`).

After register home assignment and post-allocation instruction realization (`PostAllocationMachinePlan`),
machine code progresses through three distinct resolution levels owned by this crate:
1. `SelectedFormEncoding` (`encoding`) — layout-independent instruction byte rows (`SelectedFormEncodingRow`)
   backed by a shared byte image and deduplicated decoded-footprint store (`SelectedFormEncodedRowStore`),
   retaining symbolic frame displacements, deferred branch placeholders, and unresolved call/image fixups.
2. `ResolvedMachineLayout` (`layout`) — function-relative block and instruction layouts with resolved
   intra-function branch displacements and x86 conditional-branch relaxation evidence (`X86BranchRelaxationAttempt`,
   `X86BranchRelaxationAction`).
3. `FunctionFragmentEmissionPlan` (`fragments`) and `RelocationFreeTextSectionPlacement` (`layout::text_section`) —
   realized function fragments, applied fixed-frame prologues/epilogues (`FunctionFragmentFrameApplication`),
   whole-function exit contracts (`WholeFunctionExitContract`), and dense text-section placements with
   in-section internal-call resolutions and object-bound import/image-address fixups.
*/

/*
Why this crate separates representation data from stage admission:
Every struct and canonical SHA-256 identity digest (`SelectedFormEncodingIdentity`,
`ResolvedSelectedFormLayoutIdentity`, `WholeFunctionExitContractIdentity`,
`FunctionFragmentEmissionIdentity`, `TerminalRelocationFreeTextSectionIdentity`) describes
the current machine-code artifact and its replay inputs so downstream stages and independent
object/artifact verifiers can audit exact byte, fixup, and provenance correspondence without
importing producer transformation passes. Constructing or decoding a record checks wire and
digest consistency only; it never grants lowering, optimization, or publication authority.
*/

pub(crate) use optimization_core::cursor;
pub(crate) use optimization_core::digest_encoder;

use register_model::RegisterViewId;
use selected_instructions::{MachineIdentityBytes, SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{BlockId, EdgeId, InvocationSite, MachineId, OperationId, PlaceId};
use target::NativeTarget;
use target_operations::CallSiteOwner;
use terminal_psi::{
    StructuralPlaceDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
};

pub(crate) mod codec_bytes {
    #[cfg(test)]
    pub(crate) use optimization_core::codec_bytes::patch_encoded;
    pub(crate) use optimization_core::codec_bytes::push_encoded;

    pub(crate) fn encode_manifest_envelope(
        magic: &[u8],
        version: u32,
        identity: &[u8],
        content: &[u8],
    ) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(magic);
        push_encoded(&mut encoded, version.to_le_bytes());
        encoded.extend_from_slice(identity);
        encoded.extend_from_slice(content);
        encoded
    }
}

pub(crate) mod identity_encoding {
    use super::*;

    pub(crate) fn encode_nested_identity(
        hasher: &mut digest_encoder::DomainDigestEncoder,
        value: impl FnOnce(&mut Vec<u8>),
    ) {
        let mut identity = Vec::new();
        value(&mut identity);
        hasher.raw(&identity);
    }

    pub(crate) fn encode_views(
        hasher: &mut digest_encoder::DomainDigestEncoder,
        views: &[RegisterViewId],
    ) {
        hasher.u64(views.len() as u64);
        for view in views {
            hasher.raw(&view.0.to_le_bytes());
        }
    }

    pub(crate) fn encode_conditional_branch_arm(
        hasher: &mut digest_encoder::DomainDigestEncoder,
        branch: &crate::ResolvedConditionalBranchEvidence,
    ) {
        hasher.byte(1);
        hasher.byte(branch.predicate.rank());
        hasher.raw(&branch.source_block.0.to_le_bytes());
        hasher.raw(
            &branch
                .when_taken_edge
                .map_or(0, |edge| edge.get())
                .to_le_bytes(),
        );
        hasher.raw(&branch.when_taken_block.0.to_le_bytes());
        hasher.raw(&branch.when_taken_offset.to_le_bytes());
        hasher.raw(
            &branch
                .when_fallthrough_edge
                .map_or(0, |edge| edge.get())
                .to_le_bytes(),
        );
        hasher.raw(&branch.when_fallthrough_block.0.to_le_bytes());
        hasher.raw(&branch.when_fallthrough_offset.to_le_bytes());
        hasher.raw(&branch.byte_displacement.to_le_bytes());
    }

    pub(crate) fn encode_jump_arm(
        hasher: &mut digest_encoder::DomainDigestEncoder,
        jump: &crate::ResolvedJumpEvidence,
    ) {
        hasher.byte(2);
        hasher.raw(&jump.source_block.0.to_le_bytes());
        hasher.raw(&jump.target_edge.map_or(0, |edge| edge.get()).to_le_bytes());
        hasher.raw(&jump.target_block.0.to_le_bytes());
        hasher.raw(&jump.target_offset.to_le_bytes());
        hasher.raw(&jump.byte_displacement.to_le_bytes());
    }

    pub(crate) fn encode_call_owner(bytes: &mut impl MachineIdentityBytes, owner: CallSiteOwner) {
        match owner {
            CallSiteOwner::Operation(operation) => {
                bytes.identity_bytes(&[0]);
                bytes.identity_bytes(&operation.get().to_le_bytes());
            }
            CallSiteOwner::InvokeNoNormal(edge) => {
                bytes.identity_bytes(&[3]);
                bytes.identity_bytes(&edge.get().to_le_bytes());
            }
            CallSiteOwner::ExternalCompletion(edge) => {
                bytes.identity_bytes(&[2]);
                bytes.identity_bytes(&edge.get().to_le_bytes());
            }
            CallSiteOwner::Installation { operation, role } => {
                bytes.identity_bytes(&[4]);
                bytes.identity_bytes(&operation.get().to_le_bytes());
                bytes.identity_bytes(&[match role {
                    target_operations::InstallationCallRole::Validate => 0,
                    target_operations::InstallationCallRole::Work => 1,
                }]);
            }
            CallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            } => {
                bytes.identity_bytes(&[1]);
                bytes.identity_bytes(&edge.get().to_le_bytes());
                bytes.identity_bytes(&action_ordinal.to_le_bytes());
            }
        }
    }

    pub(crate) fn encode_invocation_site(
        bytes: &mut impl MachineIdentityBytes,
        site: InvocationSite,
    ) {
        match site {
            InvocationSite::Operation(operation) => {
                bytes.identity_bytes(&[0]);
                bytes.identity_bytes(&operation.get().to_le_bytes());
            }
            InvocationSite::InvokeNoNormal(edge) => {
                bytes.identity_bytes(&[2]);
                bytes.identity_bytes(&edge.get().to_le_bytes());
            }
            InvocationSite::ExternalCompletion(edge) => {
                bytes.identity_bytes(&[1]);
                bytes.identity_bytes(&edge.get().to_le_bytes());
            }
        }
    }
}

pub(crate) fn encode_units(
    hasher: &mut digest_encoder::DomainDigestEncoder,
    units: &[register_model::RegisterUnitId],
) {
    hasher.u64(units.len() as u64);
    for unit in units {
        hasher.raw(&unit.0.to_le_bytes());
    }
}

pub mod image_addresses {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ByteLiteralReference {
        pub operation: OperationId,
        pub place: PlaceId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ImageAddressFields {
        X86_64Relative32 {
            patch_offset: u64, // disp32 at the end of `lea reg, [rip + disp32]`, referenced from the next instruction.
        },
        Aarch64PageAddress {
            page_patch_offset: u64,
            page_offset_patch_offset: u64,
        },
    }

    impl ImageAddressFields {
        fn shifted_by(self, by: u64, offset: impl Fn(u64, u64) -> Option<u64>) -> Option<Self> {
            Some(match self {
                Self::X86_64Relative32 { patch_offset } => Self::X86_64Relative32 {
                    patch_offset: offset(patch_offset, by)?,
                },
                Self::Aarch64PageAddress {
                    page_patch_offset,
                    page_offset_patch_offset,
                } => Self::Aarch64PageAddress {
                    page_patch_offset: offset(page_patch_offset, by)?,
                    page_offset_patch_offset: offset(page_offset_patch_offset, by)?,
                },
            })
        }

        pub fn shifted(self, by: u64) -> Option<Self> {
            self.shifted_by(by, u64::checked_add)
        }

        pub fn unshifted(self, by: u64) -> Option<Self> {
            self.shifted_by(by, u64::checked_sub)
        }

        pub fn encode_identity(self, bytes: &mut Vec<u8>) {
            match self {
                Self::X86_64Relative32 { patch_offset } => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, patch_offset.to_le_bytes());
                }
                Self::Aarch64PageAddress {
                    page_patch_offset,
                    page_offset_patch_offset,
                } => {
                    bytes.push(2);
                    crate::codec_bytes::push_encoded(bytes, page_patch_offset.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, page_offset_patch_offset.to_le_bytes());
                }
            }
        }
    }

    impl ByteLiteralReference {
        pub fn encode_identity(self, bytes: &mut Vec<u8>) {
            crate::codec_bytes::push_encoded(bytes, self.operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, self.place.get().to_le_bytes());
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SelectedFormImageAddressFixup {
        pub reference: selected_instructions::SelectedImageAddressReference,
        pub fields: ImageAddressFields,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FunctionFragmentImageAddressFixup {
        pub reference: selected_instructions::SelectedImageAddressReference,
        pub fields: ImageAddressFields,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PlacedImageAddress {
        pub machine: MachineId,
        pub block: SelectedBlockId,
        pub instruction: SelectedInstructionId,
        pub reference: selected_instructions::SelectedImageAddressReference,
        pub operation: OperationId,
        pub instruction_function_offset: u64,
        pub instruction_section_offset: u64,
        pub instruction_byte_count: u64,
        pub fields: ImageAddressFields,
    }

    impl PlacedImageAddress {
        pub fn byte_literal(&self) -> Option<ByteLiteralReference> {
            match self.reference {
                selected_instructions::SelectedImageAddressReference::ByteLiteral {
                    operation,
                    place,
                } => Some(ByteLiteralReference { operation, place }),
                _ => None,
            }
        }

        pub fn implementation_table(&self) -> Option<u32> {
            match self.reference {
                selected_instructions::SelectedImageAddressReference::ImplementationTable {
                    table,
                } => Some(table),
                _ => None,
            }
        }

        pub fn machine_descriptor(&self) -> Option<semantic_vocabulary::MachineDescriptorId> {
            match self.reference {
                selected_instructions::SelectedImageAddressReference::MachineDescriptor {
                    descriptor,
                } => Some(descriptor),
                _ => None,
            }
        }

        pub fn encode_identity(&self, bytes: &mut Vec<u8>) {
            crate::codec_bytes::push_encoded(bytes, self.machine.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, self.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, self.instruction.0.to_le_bytes());
            self.reference.encode_identity(bytes);
            crate::codec_bytes::push_encoded(bytes, self.operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, self.instruction_function_offset.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, self.instruction_section_offset.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, self.instruction_byte_count.to_le_bytes());
            self.fields.encode_identity(bytes);
        }
    }
}

pub mod instructions {
    use super::*;

    pub mod x86_fma {
        use super::*;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct X86FloatingControlRecord {
            pub target: NativeTarget,
            pub canonical_mxcsr: u32,
            pub canonical_slot_byte_offset: u32,
            pub saved_slot_byte_offset: u32,
            pub save_offset: usize,
            pub save_byte_count: usize,
            pub canonical_store_offset: usize,
            pub canonical_store_byte_count: usize,
            pub install_offset: usize,
            pub install_byte_count: usize,
            pub restore_offset: usize,
            pub restore_byte_count: usize,
        }
    }
    pub use x86_fma::X86FloatingControlRecord;
}

pub mod ownership {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct UnitAffineCleanupRecord {
        pub psi_edge: EdgeId,
        pub structural_types: abstract_operations::StructuralTypeCatalog, // Verifier-owned type closure retained to reconstruct the exact residual partition.
        pub locals: Vec<(
            OperationId,
            StructuralPlaceDeclaration,
            StructuralTypeDeclaration,
        )>,
        pub actions: Vec<TerminalAffineCleanupAction>, // Exact semantic execution order for every cleanup committed by this edge.
        pub code_offset: usize,
        pub byte_count: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct UnitContinuationRecord {
        pub operation_ordinal: usize,
        pub successor_operation_ordinal: usize,
        pub source_block: BlockId,
        pub target_block: BlockId,
        pub bindings: Vec<abstract_operations::ValueBinding>,
        pub cleanup: UnitAffineCleanupRecord, // Zero-byte cleanup boundary before the successor's first operation.
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScalarControlAffineCleanupRecord {
        pub cleanup: UnitAffineCleanupRecord,
        pub preservation: ScalarCleanupPreservationEvidence,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ScalarCleanupPreservationEvidence {
        pub frame: crate::StackAdjustmentPair,
        pub result_byte_offset: u32,
        pub result_store_offset: usize,
        pub result_load_offset: usize,
        pub aarch64_return_link: Option<crate::Aarch64ReturnLinkEvidence>,
    }
}

pub mod provenance {
    use super::*;

    pub type SemanticCodeSite = optimization_unit::PsiProvenance;

    pub fn semantic_code_site(site: InvocationSite) -> SemanticCodeSite {
        match site {
            InvocationSite::Operation(operation) => SemanticCodeSite::Operation(operation),
            InvocationSite::InvokeNoNormal(edge) | InvocationSite::ExternalCompletion(edge) => {
                SemanticCodeSite::Edge(edge)
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SemanticCodeAttribution {
        pub site: SemanticCodeSite,
        pub operation_ordinal: usize,
        pub code_offset: usize,
        pub byte_count: usize,
    }
}

pub mod boundary;
pub mod calls;
pub mod control_flow;
pub mod encoding;
pub mod fragments;
pub mod functions;
pub mod layout;
pub mod storage;

#[doc(hidden)]
pub use crate as machine_code;

pub use boundary::{
    BoundaryByteSequenceArgumentRecord, BoundaryExecutionRecord, BoundaryResultRecord,
    BoundaryScalarResultRecord, BoundarySettlementRecord, BoundaryStructuralResultRecord,
    CompletionProviderCustodyBinding, PortEffectRecord, ProviderExecutionRecord,
    derive_completion_provider_custody, execution, ports, settlements,
};
pub use calls::*;
pub use control_flow::{
    BooleanStructuralConditionEvidence, ScalarConditionalBranchEvidence,
    ScalarConditionalCondition, ScalarControlBlockEvidence, ScalarControlFlowEvidence,
    ScalarControlTerminatorEvidence, ScalarDirectConditionalBranchEvidence,
    ScalarDivisionBranchEvidence, ScalarJoinBranchEvidence,
};
pub use encoding::{
    DeferredControlEncodingReason, ResolvedPhysicalAddress, SelectedFormDecodedFootprint,
    SelectedFormDecodedFootprintHandle, SelectedFormEncodedByteSpan, SelectedFormEncodedRowStore,
    SelectedFormEncoding, SelectedFormEncodingCounts, SelectedFormEncodingIdentity,
    SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineDisposition,
};
pub use fragments::{
    FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentBranchEvidence,
    FunctionFragmentCallbackMaterialization, FunctionFragmentCallbackMaterializationEncoding,
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentControlProvenance, FunctionFragmentEmissionManifest,
    FunctionFragmentEmissionManifestDecodeError, FunctionFragmentEmissionPlan,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData, FunctionFragmentInstructionSpan,
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState, FunctionFragmentNormalizedForeignCallFixup,
    FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState, FunctionFragmentSuccessorProvenance, fixups,
    identity, publication,
};
pub use functions::{
    WholeFunctionCrashEvidence, WholeFunctionEntryAssumption, WholeFunctionExitContract,
    WholeFunctionExitContractIdentity, WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody,
    WholeFunctionExitPolicy, WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy,
    WholeFunctionProcessExitEvidence, WholeFunctionReturnEvidence,
    WholeFunctionReturnFragmentEvidence, WholeFunctionReturnMechanism,
    WholeFunctionReturnValueEvidence, exit_contract, whole_function_exit_contract_identity,
};
pub use image_addresses::{
    ByteLiteralReference, FunctionFragmentImageAddressFixup, ImageAddressFields,
    PlacedImageAddress, SelectedFormImageAddressFixup,
};
pub use instructions::*;
pub use layout::text_section::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, InternalMachineCallResolutionKind,
    InternalMachineCallResolutionState, NormalizedForeignCallResolutionKind,
    NormalizedForeignCallResolutionState, PlacedBlockSpan, PlacedCallbackAddressEncoding,
    PlacedCallbackAddressMaterialization, PlacedFunctionFragment, PlacedInstructionSpan,
    PlacedInternalMachineCallResolution, PlacedNormalizedForeignCallResolution,
    RelocationFreeTextSectionPlacement, TextSectionPlacementPolicy,
    TextSectionRelocationRequirements, relocation_free_text_section_identity,
};
pub use layout::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedJumpEvidence, ResolvedMachineLayout, ResolvedMachineProgram,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy, X86BranchRelaxationIdentity,
    resolved_machine_layout_identity,
};
pub use ownership::*;
pub use provenance::*;
pub use storage::*;
