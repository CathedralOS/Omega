/*
Text-section publication manifest (`FunctionFragmentTextSectionManifest`) and
binary wire codec (`OMGTSP\0\0`).

We encode and decode the canonical text-section placement manifest along with
its upstream custody identities, relocation requirements, and section
statistics. Decoding establishes canonical representation data and verifies
content identity, without granting object container, symbol, or executable
publication authority.
*/

use super::{TextSectionPlacementPolicy, TextSectionRelocationRequirements};
use crate::cursor::Cursor;
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::{NativeTarget, TargetWireIdentityError};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    FunctionFragmentFrameApplicationIdentity, ResolvedSelectedFormLayoutIdentity,
    SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGTSP\0\0";
const MANIFEST_VERSION: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionStage {
    ValidatedFixedFrameInternalCallTextSectionPlacementV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionFragmentTextSectionStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub instruction_spans: u64,
    pub zero_byte_instruction_spans: u64,
    pub bytes: u64,
    pub padding_bytes: u64,
    pub relocation_requirements: u64,
    pub source_internal_machine_fixups: u64,
    pub resolved_internal_machine_fixups: u64,
    pub remaining_internal_machine_fixups: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentTextSectionManifest {
    pub identity: FunctionFragmentTextSectionManifestIdentity,
    pub stage: FunctionFragmentTextSectionStage,
    pub frame_application: FunctionFragmentFrameApplicationIdentity,
    pub source_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    pub source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: physical_instructions::PostAllocationMachineIdentity,
    pub final_pre_layout: SelectedFormEncodingIdentity,
    pub final_resolved_layout: ResolvedSelectedFormLayoutIdentity,
    pub whole_function_exit_contract: WholeFunctionExitContractIdentity,
    pub fragments: FunctionFragmentEmissionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_offset: u64,
    pub access_policy: TextSectionPlacementPolicy,
    pub text_section: TerminalRelocationFreeTextSectionIdentity,
    pub relocation_requirements: TextSectionRelocationRequirements,
    pub statistics: FunctionFragmentTextSectionStatistics,
    pub symbols: FunctionFragmentTextSectionUnavailableData,
    pub object_container: FunctionFragmentTextSectionUnavailableData,
    pub external_entry_bridge: FunctionFragmentTextSectionUnavailableData,
    pub executable_image: FunctionFragmentTextSectionUnavailableData,
    pub installation: FunctionFragmentTextSectionUnavailableData,
    pub publication: FunctionFragmentTextSectionUnavailableData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownVocabulary(u16),
    InvalidWorkSchedule,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    InvalidSemanticEntry,
    UnknownAccessPolicy(u8),
    UnknownRelocationRequirements(u8),
    UnknownUnavailableStatus,
    IdentityMismatch,
    TrailingBytes,
}

optimization_core::truncated_from_adapter!(FunctionFragmentTextSectionManifestDecodeError);

register_homes::decode_error_display! { FunctionFragmentTextSectionManifestDecodeError, "invalid function-fragment text-section manifest" }

impl FunctionFragmentTextSectionManifest {
    pub fn recomputed_identity(&self) -> FunctionFragmentTextSectionManifestIdentity {
        let mut canonical = b"omega.function-fragment-text-section-manifest.v16\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        crate::codec_bytes::encode_manifest_envelope(
            MANIFEST_MAGIC,
            MANIFEST_VERSION,
            &self.identity.bytes(),
            &encode_manifest_content(self),
        )
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FunctionFragmentTextSectionManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(FunctionFragmentTextSectionManifestDecodeError::WrongMagic);
        }
        let version = cursor.u32()?;
        if version != MANIFEST_VERSION {
            return Err(
                FunctionFragmentTextSectionManifestDecodeError::UnsupportedVersion(version),
            );
        }
        let identity = FunctionFragmentTextSectionManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            2 => FunctionFragmentTextSectionStage::ValidatedFixedFrameInternalCallTextSectionPlacementV1,
            tag => return Err(FunctionFragmentTextSectionManifestDecodeError::UnknownStage(tag)),
        };
        let frame_application =
            FunctionFragmentFrameApplicationIdentity::from_bytes(cursor.array()?);
        let source_fragment_manifest =
            FunctionFragmentEmissionManifestIdentity::from_bytes(cursor.array()?);
        let source_realization =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?);
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let marker = cursor.u16()?;
        let vocabulary_marker = VocabularyMarker::new(marker)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::UnknownVocabulary(marker))?;
        let psi = TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
        };
        let work_marker = cursor.u32()?;
        let work_schedule = WorkScheduleIdentity::new(work_marker)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::InvalidWorkSchedule)?;
        let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let post_allocation_manifest =
            PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let post_allocation_machine =
            physical_instructions::PostAllocationMachineIdentity::from_bytes(cursor.array()?);
        let final_pre_layout = SelectedFormEncodingIdentity::from_bytes(cursor.array()?);
        let final_resolved_layout = ResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
        let whole_function_exit_contract =
            WholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
        let fragments = FunctionFragmentEmissionIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let semantic_entry_raw = cursor.u64()?;
        let semantic_entry = MachineId::new(semantic_entry_raw)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::InvalidSemanticEntry)?;
        let semantic_entry_offset = cursor.u64()?;
        let access_policy = match cursor.byte()? {
            1 => TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
            tag => {
                return Err(
                    FunctionFragmentTextSectionManifestDecodeError::UnknownAccessPolicy(tag),
                );
            }
        };
        let text_section = TerminalRelocationFreeTextSectionIdentity::from_bytes(cursor.array()?);
        let relocation_requirements = match cursor.byte()? {
            1 => TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
            2 => TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1,
            3 => TextSectionRelocationRequirements::UnresolvedObjectSymbolFieldsV1,
            tag => {
                return Err(
                    FunctionFragmentTextSectionManifestDecodeError::UnknownRelocationRequirements(
                        tag,
                    ),
                );
            }
        };
        let statistics = FunctionFragmentTextSectionStatistics {
            functions: cursor.u64()?,
            blocks: cursor.u64()?,
            instruction_spans: cursor.u64()?,
            zero_byte_instruction_spans: cursor.u64()?,
            bytes: cursor.u64()?,
            padding_bytes: cursor.u64()?,
            relocation_requirements: cursor.u64()?,
            source_internal_machine_fixups: cursor.u64()?,
            resolved_internal_machine_fixups: cursor.u64()?,
            remaining_internal_machine_fixups: cursor.u64()?,
        };
        for _ in 0..6 {
            if cursor.byte()? != 1 {
                return Err(
                    FunctionFragmentTextSectionManifestDecodeError::UnknownUnavailableStatus,
                );
            }
        }
        if cursor.remaining() != 0 {
            return Err(FunctionFragmentTextSectionManifestDecodeError::TrailingBytes);
        }
        let unavailable = FunctionFragmentTextSectionUnavailableData::Unavailable;
        let manifest = Self {
            identity,
            stage,
            frame_application,
            source_fragment_manifest,
            source_realization,
            selections,
            psi,
            work_schedule,
            selected,
            post_allocation_manifest,
            post_allocation_machine,
            final_pre_layout,
            final_resolved_layout,
            whole_function_exit_contract,
            fragments,
            target,
            semantic_entry,
            semantic_entry_offset,
            access_policy,
            text_section,
            relocation_requirements,
            statistics,
            symbols: unavailable,
            object_container: unavailable,
            external_entry_bridge: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        if manifest.recomputed_identity() != identity {
            return Err(FunctionFragmentTextSectionManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

fn encode_manifest_content(record: &FunctionFragmentTextSectionManifest) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(match record.stage {
        FunctionFragmentTextSectionStage::ValidatedFixedFrameInternalCallTextSectionPlacementV1 => {
            2
        }
    });
    bytes.extend_from_slice(&record.frame_application.bytes());
    bytes.extend_from_slice(&record.source_fragment_manifest.bytes());
    bytes.extend_from_slice(&record.source_realization.bytes());
    bytes.extend_from_slice(&record.selections.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(record.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.work_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&record.selected.bytes());
    bytes.extend_from_slice(&record.post_allocation_manifest.bytes());
    bytes.extend_from_slice(&record.post_allocation_machine.bytes());
    bytes.extend_from_slice(&record.final_pre_layout.bytes());
    bytes.extend_from_slice(&record.final_resolved_layout.bytes());
    bytes.extend_from_slice(&record.whole_function_exit_contract.bytes());
    bytes.extend_from_slice(&record.fragments.bytes());
    bytes.extend_from_slice(&record.target.wire_identity_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.semantic_entry.get().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.semantic_entry_offset.to_le_bytes());
    bytes.push(match record.access_policy {
        TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1 => 1,
    });
    bytes.extend_from_slice(&record.text_section.bytes());
    bytes.push(match record.relocation_requirements {
        TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1 => 1,
        TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1 => 2,
        TextSectionRelocationRequirements::UnresolvedObjectSymbolFieldsV1 => 3,
    });
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.functions.to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.blocks.to_le_bytes());
    crate::codec_bytes::push_encoded(
        &mut bytes,
        record.statistics.instruction_spans.to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(
        &mut bytes,
        record.statistics.zero_byte_instruction_spans.to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.bytes.to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.padding_bytes.to_le_bytes());
    crate::codec_bytes::push_encoded(
        &mut bytes,
        record.statistics.relocation_requirements.to_le_bytes(),
    );
    bytes.extend_from_slice(
        &record
            .statistics
            .source_internal_machine_fixups
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &record
            .statistics
            .resolved_internal_machine_fixups
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &record
            .statistics
            .remaining_internal_machine_fixups
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&[1; 6]);
    bytes
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, FunctionFragmentTextSectionManifestDecodeError> {
    let encoded = cursor.array::<18>()?;
    NativeTarget::from_wire_identity_bytes(&encoded).map_err(|error| match error {
        TargetWireIdentityError::UnknownArchitecture(tag) => {
            FunctionFragmentTextSectionManifestDecodeError::UnknownArchitecture(tag)
        }
        TargetWireIdentityError::UnknownObjectFormat(tag) => {
            FunctionFragmentTextSectionManifestDecodeError::UnknownObjectFormat(tag)
        }
        TargetWireIdentityError::LayoutOverflow => {
            FunctionFragmentTextSectionManifestDecodeError::TargetLayoutOverflow
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
        FunctionFragmentFrameApplicationIdentity, FunctionFragmentTextSectionManifest,
        FunctionFragmentTextSectionManifestDecodeError,
        FunctionFragmentTextSectionManifestIdentity, FunctionFragmentTextSectionStage,
        FunctionFragmentTextSectionStatistics, FunctionFragmentTextSectionUnavailableData,
        FunctionRelativeOptimizationRealizationManifestIdentity, MachineId, NativeTarget,
        OptimizationSelectionIdentity, PostAllocationOptimizationManifestIdentity,
        ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity,
        SelectedInstructionPlanIdentity, TerminalPsiIdentity,
        TerminalRelocationFreeTextSectionIdentity, TextSectionPlacementPolicy,
        TextSectionRelocationRequirements, WholeFunctionExitContractIdentity, WorkScheduleIdentity,
    };

    #[test]
    fn prior_wire_versions_with_route_taxonomy_reject() {
        let encoded = record().encode();
        for version in 0..16_u32 {
            let mut stale = encoded.clone();
            crate::codec_bytes::patch_encoded(&mut stale, 8, version.to_le_bytes());
            assert_eq!(
                FunctionFragmentTextSectionManifest::decode(&stale),
                Err(FunctionFragmentTextSectionManifestDecodeError::UnsupportedVersion(version))
            );
        }
    }

    fn record() -> FunctionFragmentTextSectionManifest {
        let unavailable = FunctionFragmentTextSectionUnavailableData::Unavailable;
        let mut value = FunctionFragmentTextSectionManifest {
            identity: FunctionFragmentTextSectionManifestIdentity::from_bytes([0; 32]),
            stage: FunctionFragmentTextSectionStage::ValidatedFixedFrameInternalCallTextSectionPlacementV1,
            frame_application: FunctionFragmentFrameApplicationIdentity::from_bytes([16; 32]),
            source_fragment_manifest: FunctionFragmentEmissionManifestIdentity::from_bytes([1; 32]),
            source_realization: FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(
                [2; 32],
            ),
            selections: OptimizationSelectionIdentity::from_bytes([3; 32]),
            psi: TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([4; 32]),
            },
            work_schedule: WorkScheduleIdentity::new(5).unwrap(),
            selected: SelectedInstructionPlanIdentity::from_bytes([6; 32]),
            post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes([7; 32]),
            post_allocation_machine: physical_instructions::PostAllocationMachineIdentity::from_bytes(
                [8; 32],
            ),
            final_pre_layout: SelectedFormEncodingIdentity::from_bytes([9; 32]),
            final_resolved_layout: ResolvedSelectedFormLayoutIdentity::from_bytes([10; 32]),
            whole_function_exit_contract: WholeFunctionExitContractIdentity::from_bytes([11; 32]),
            fragments: FunctionFragmentEmissionIdentity::from_bytes([12; 32]),
            target: NativeTarget::linux_x64(),
            semantic_entry: MachineId::new(13).unwrap(),
            semantic_entry_offset: 14,
            access_policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
            text_section: TerminalRelocationFreeTextSectionIdentity::from_bytes([15; 32]),
            relocation_requirements:
                TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
            statistics: FunctionFragmentTextSectionStatistics::default(),
            symbols: unavailable,
            object_container: unavailable,
            external_entry_bridge: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        value.identity = value.recomputed_identity();
        value
    }

    #[test]
    fn publication_roundtrips_without_route_taxonomy() {
        let record = record();
        let bytes = record.encode();
        assert_eq!(&bytes[..8], b"OMGTSP\0\0");
        assert_eq!(&bytes[8..12], &16_u32.to_le_bytes());
        assert_eq!(bytes.len(), 589);
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&bytes),
            Ok(record)
        );
    }

    #[test]
    fn codec_checks_frame_identity_and_rejects_retired_direct_stage() {
        let mut record = record();
        record.statistics.bytes = u64::MAX;
        record.identity = record.recomputed_identity();
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&record.encode()),
            Ok(record.clone())
        );
        let mut retired = record.encode();
        retired[44] = 1;
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&retired),
            Err(FunctionFragmentTextSectionManifestDecodeError::UnknownStage(1))
        );
    }
}
