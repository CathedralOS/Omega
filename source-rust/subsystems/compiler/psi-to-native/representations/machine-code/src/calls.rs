/*
Machine-code call records, argument/result custody, callback address materializations, and fixups.

We group the emitted call evidence into six subordinate modules:
1. `arguments` — scalar and structural argument sources (`InternalUnitScalarArgumentSourceRecord`,
   `InternalUnitStructuralArgumentSourceRecord`) and physical argument transfer intervals.
2. `callbacks` — symbolic materializations of compiler-private callback addresses (`CallbackAddressMaterialization`).
3. `fixups` — row-relative unresolved internal-machine and normalized-foreign call fields
   (`SelectedFormInternalMachineFixup`, `SelectedFormNormalizedForeignCallFixup`).
4. `foreign` — per-call MXCSR/FPCR floating-control preservation
   (`ForeignCallFloatingControlRecord`).
5. `internal` — internal Unit call records (`InternalUnitCallRecord`).
6. `results` — durable scalar result homes (`ForeignCallScalarResultRecord`).
*/

use crate::{UnitScalarHomeRecord, UnitScalarParameterLocationRecord};
use calling_conventions::{ValuePlacement, ValueShape};
use operation_operands::values::AbstractResult;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, IntegerType, IntegerValue, MachineId, OperationId,
    PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use target_vocabulary::CallSiteOwner;
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, ProviderCandidateConformance, StructuralPathSegment,
    StructuralResultDeclaration,
};

pub mod arguments {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ForeignCallScalarArgumentRecord {
        pub parameter_index: u32,
        pub source: InternalUnitScalarArgumentSourceRecord,
        pub placement: ValuePlacement,
        pub code_offset: usize,
        pub byte_count: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct InternalUnitCallArgumentRecord {
        pub place: PlaceId,
        pub access: terminal_psi::StructuralAccess,
        pub path: Vec<StructuralPathSegment>,
        pub root_structural_type: StructuralTypeId,
        pub structural_type: StructuralTypeId,
        pub shape: ValueShape,
        pub source_byte_offset: u32,
        pub source_location: crate::StructuralSourceLocation,
        pub call_stack_bytes: u32,
        pub fixed_array_length: Option<u64>,
        pub element_stride: Option<u32>,
        pub source: InternalUnitStructuralArgumentSourceRecord,
        pub destination: ValuePlacement,
        pub code_offset: usize,
        pub byte_count: usize,
        pub bytes: Vec<u8>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum InternalUnitStructuralArgumentSourceRecord {
        Placement(ValuePlacement),
        EstablishedPrimitiveLocal { psi_operation: OperationId },
        EstablishedByteView { psi_operation: OperationId },
        EstablishedElementView { psi_operation: OperationId },
        BlockParameter { block: BlockId, place: PlaceId },
    }

    impl InternalUnitStructuralArgumentSourceRecord {
        pub fn placement(&self) -> Option<&ValuePlacement> {
            match self {
                Self::Placement(placement) => Some(placement),
                Self::EstablishedPrimitiveLocal { .. }
                | Self::EstablishedByteView { .. }
                | Self::EstablishedElementView { .. }
                | Self::BlockParameter { .. } => None,
            }
        }
    }

    impl From<ValuePlacement> for InternalUnitStructuralArgumentSourceRecord {
        fn from(placement: ValuePlacement) -> Self {
            Self::Placement(placement)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum InternalUnitScalarArgumentSourceRecord {
        SelectedProcessExit {
            source_value: ValueId,
            scalar_type: ScalarType,
            instruction: selected_instructions::SelectedInstructionId,
        },
        SelectedCall {
            source_value: ValueId,
            scalar_type: ScalarType,
            instruction: selected_instructions::SelectedInstructionId,
        },
        SelectedBoundary {
            source_value: ValueId,
            scalar_type: ScalarType,
            instruction: selected_instructions::SelectedInstructionId,
            scratch_byte_offset: u64,
        },
        Parameter {
            parameter_index: u32,
            source_value: ValueId,
            scalar_type: ScalarType,
            location: UnitScalarParameterLocationRecord,
        },
        IntegerImmediate {
            defining_operation: OperationId,
            source_value: ValueId,
            scalar_type: IntegerType,
            value: IntegerValue,
        },
        BooleanImmediate {
            defining_operation: OperationId,
            source_value: ValueId,
            value: bool,
            definition_ordinal: usize,
        },
        Home(UnitScalarHomeRecord),
    }

    impl InternalUnitScalarArgumentSourceRecord {
        pub const fn source_value(self) -> ValueId {
            match self {
                Self::SelectedProcessExit { source_value, .. }
                | Self::SelectedBoundary { source_value, .. }
                | Self::SelectedCall { source_value, .. } => source_value,
                Self::Parameter { source_value, .. } => source_value,
                Self::IntegerImmediate { source_value, .. } => source_value,
                Self::BooleanImmediate { source_value, .. } => source_value,
                Self::Home(home) => home.source_value,
            }
        }

        pub const fn scalar_type(self) -> ScalarType {
            match self {
                Self::SelectedProcessExit { scalar_type, .. }
                | Self::SelectedBoundary { scalar_type, .. }
                | Self::SelectedCall { scalar_type, .. } => scalar_type,
                Self::Parameter { scalar_type, .. } => scalar_type,
                Self::IntegerImmediate { scalar_type, .. } => ScalarType::Integer(scalar_type),
                Self::BooleanImmediate { .. } => ScalarType::Boolean,
                Self::Home(home) => home.scalar_type,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct InternalUnitScalarCallArgumentRecord {
        pub parameter_index: u32,
        pub source: InternalUnitScalarArgumentSourceRecord,
        pub destination: ValuePlacement,
        pub code_offset: usize,
        pub byte_count: usize,
    }
}

pub mod callbacks {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum CallbackAddressDestination {
        Register(target_vocabulary::MachineRegister),
        OutgoingStack {
            byte_offset: u32,
        },
        RecordField {
            root_register: target_vocabulary::MachineRegister,
            placements: calling_conventions::PrivateCallbackPlacements,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CallbackAddressEncoding {
        X86_64Relative32 {
            relocation_offset: usize,
        },
        Aarch64PageAddress {
            page_relocation_offset: usize,
            page_offset_relocation_offset: usize,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CallbackAddressMaterialization {
        pub target: target_vocabulary::TargetNativeCallbackArgument,
        pub destination: CallbackAddressDestination,
        pub code_offset: usize,
        pub byte_count: usize,
        pub encoding: CallbackAddressEncoding,
    }

    pub(crate) fn encode_callback_destination_identity(
        hasher: &mut crate::digest_encoder::DomainDigestEncoder,
        destination: &CallbackAddressDestination,
    ) {
        match destination {
            CallbackAddressDestination::Register(register) => {
                hasher.byte(0);
                hasher.byte(register.codec_index());
            }
            CallbackAddressDestination::OutgoingStack { byte_offset } => {
                hasher.byte(1);
                hasher.raw(&byte_offset.to_le_bytes());
            }
            CallbackAddressDestination::RecordField {
                root_register,
                placements,
            } => {
                hasher.byte(2);
                hasher.byte(root_register.codec_index());
                let mut encoded = Vec::new();
                calling_conventions::encode_private_callback_placements(&mut encoded, placements);
                hasher.raw(&encoded);
            }
        }
    }
}

pub mod fixups {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SelectedFormInternalMachineFixupKind {
        X86Relative32FromNextInstructionToInternalMachineV1,
        Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SelectedFormInternalMachineFixupState {
        UnresolvedZeroFieldV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SelectedFormInternalMachineFixup {
        pub kind: SelectedFormInternalMachineFixupKind,
        pub state: SelectedFormInternalMachineFixupState,
        pub callee: MachineId,
        pub opcode_row_offset: u16,
        pub patch_row_offset: u16,
        pub reference_row_offset: u16,
        pub patch_byte_width: u8,
        pub addend: i64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SelectedFormNormalizedForeignCallFixupKind {
        X86Relative32FromNextInstructionToNormalizedForeignImportV1,
        Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SelectedFormNormalizedForeignCallFixupState {
        UnresolvedImportFieldV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SelectedFormCallbackMaterializationEncoding {
        X86_64Relative32 {
            patch_row_offset: u16,
        },
        Aarch64PageAddress {
            page_patch_row_offset: u16,
            page_offset_patch_row_offset: u16,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedFormCallbackMaterialization {
        pub destination: crate::CallbackAddressDestination,
        pub code_row_offset: u16,
        pub code_byte_count: u16,
        pub encoding: SelectedFormCallbackMaterializationEncoding,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedFormNormalizedForeignCallFixup {
        pub kind: SelectedFormNormalizedForeignCallFixupKind,
        pub state: SelectedFormNormalizedForeignCallFixupState,
        pub boundary: BoundaryMachineId,
        pub ordinal: u32,
        pub opcode_row_offset: u16,
        pub patch_row_offset: u16,
        pub reference_row_offset: u16,
        pub patch_byte_width: u8,
        pub addend: i64,
        pub callbacks: Vec<SelectedFormCallbackMaterialization>,
    }

    impl SelectedFormInternalMachineFixup {
        pub(crate) fn encode_identity(
            &self,
            hasher: &mut crate::digest_encoder::DomainDigestEncoder,
        ) {
            hasher.byte(match self.kind {
                SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => 0,
                SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => 1,
            });
            hasher.byte(match self.state {
                SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1 => 0,
            });
            hasher.raw(&self.callee.get().to_le_bytes());
            hasher.raw(&self.opcode_row_offset.to_le_bytes());
            hasher.raw(&self.patch_row_offset.to_le_bytes());
            hasher.raw(&self.reference_row_offset.to_le_bytes());
            hasher.byte(self.patch_byte_width);
            hasher.raw(&self.addend.to_le_bytes());
        }
    }

    impl SelectedFormNormalizedForeignCallFixup {
        pub(crate) fn encode_identity(
            &self,
            hasher: &mut crate::digest_encoder::DomainDigestEncoder,
        ) {
            hasher.byte(match self.kind {
                SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => 0,
                SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => 1,
            });
            hasher.byte(match self.state {
                SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1 => 0,
            });
            hasher.raw(&self.boundary.get().to_le_bytes());
            hasher.raw(&self.ordinal.to_le_bytes());
            hasher.raw(&self.opcode_row_offset.to_le_bytes());
            hasher.raw(&self.patch_row_offset.to_le_bytes());
            hasher.raw(&self.reference_row_offset.to_le_bytes());
            hasher.byte(self.patch_byte_width);
            hasher.raw(&self.addend.to_le_bytes());
            hasher.len(self.callbacks.len());
            for callback in &self.callbacks {
                crate::calls::callbacks::encode_callback_destination_identity(
                    hasher,
                    &callback.destination,
                );
                hasher.raw(&callback.code_row_offset.to_le_bytes());
                hasher.raw(&callback.code_byte_count.to_le_bytes());
                match callback.encoding {
                    SelectedFormCallbackMaterializationEncoding::X86_64Relative32 {
                        patch_row_offset,
                    } => {
                        hasher.byte(0);
                        hasher.raw(&patch_row_offset.to_le_bytes());
                    }
                    SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
                        page_patch_row_offset,
                        page_offset_patch_row_offset,
                    } => {
                        hasher.byte(1);
                        hasher.raw(&page_patch_row_offset.to_le_bytes());
                        hasher.raw(&page_offset_patch_row_offset.to_le_bytes());
                    }
                }
            }
        }
    }
}

pub mod foreign {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ForeignCallFloatingControlRecord {
        pub target: target::NativeTarget,
        pub saved_slot_byte_offset: u32,
        pub save_offset: usize,
        pub save_byte_count: usize,
        pub restore_offset: usize,
        pub restore_byte_count: usize,
    }
}

pub mod internal {
    use super::*;
    use crate::{InternalUnitCallArgumentRecord, InternalUnitScalarCallArgumentRecord};

    pub fn unit_call_arguments(
        calls: &[InternalUnitCallRecord],
    ) -> impl Iterator<Item = &InternalUnitCallArgumentRecord> + '_ {
        calls.iter().flat_map(|call| &call.arguments)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct InternalUnitCallRecord {
        pub source: InternalUnitCallSource,
        pub owner: CallSiteOwner,
        pub target: MachineId,
        pub result: Option<ScalarType>,
        pub semantic_result: Option<AbstractResult>,
        pub structural_result: Option<InternalStructuralCallResult>,
        pub scalar_arguments: Vec<InternalUnitScalarCallArgumentRecord>,
        pub arguments: Vec<InternalUnitCallArgumentRecord>,
        pub claim_transfers: Vec<ClaimTransfer>,
        pub operation_ordinal: usize,
        pub code_offset: usize,
        pub byte_count: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct InternalStructuralCallResult {
        pub operation_result: terminal_psi::StructuralOperationResult,
        pub result_home: Option<InternalStructuralResultHomeRecord>,
        pub function_result: StructuralResultDeclaration,
        pub returned_claims: Vec<ClaimId>,
        pub caller_result_placement: ValuePlacement,
        pub callee_result_placement: ValuePlacement,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct InternalStructuralResultHomeRecord {
        pub requirement: target_vocabulary::TargetStructuralHomeRequirement,
        pub home_byte_offset: u32,
        pub code_offset: usize,
        pub byte_count: usize,
        pub bytes: Vec<u8>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum InternalUnitCallSource {
        ProgramEntry {
            boundary: BoundaryMachineId,
            entry: MachineId,
            source_signature: [u8; 32],
            completion_claim_sources: Vec<target_vocabulary::CompletionClaimSource>,
            completion_receipts: Vec<CompletionReceipt>,
        },
        Authored,
        InstalledProvider {
            boundary: BoundaryMachineId,
            provider: Box<ProviderCandidateConformance>,
            completion_claim_sources:
                Vec<operation_evidence::completion_claims::CompletionClaimSource>,
            completion_receipts: Vec<CompletionReceipt>,
        },
    }
}

pub mod results {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ForeignCallScalarResultRecord {
        pub home: UnitScalarHomeRecord,
        pub source: ValuePlacement,
        pub code_offset: usize,
        pub byte_count: usize,
    }
}

pub use arguments::{
    ForeignCallScalarArgumentRecord, InternalUnitCallArgumentRecord,
    InternalUnitScalarArgumentSourceRecord, InternalUnitScalarCallArgumentRecord,
    InternalUnitStructuralArgumentSourceRecord,
};
pub use callbacks::{
    CallbackAddressDestination, CallbackAddressEncoding, CallbackAddressMaterialization,
};
pub use fixups::{
    SelectedFormCallbackMaterialization, SelectedFormCallbackMaterializationEncoding,
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState, SelectedFormNormalizedForeignCallFixup,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};
pub use foreign::ForeignCallFloatingControlRecord;
pub use internal::{
    InternalStructuralCallResult, InternalUnitCallRecord, InternalUnitCallSource,
    unit_call_arguments,
};
pub use results::ForeignCallScalarResultRecord;
