use super::super::{TextSectionPlacementPolicy, TextSectionRelocationRequirements};
use crate::cursor::Cursor;
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::{NativeTarget, TargetWireIdentityError};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity,
    WholeFunctionExitContractIdentity,
};

use super::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGTSP\0\0";
const MANIFEST_VERSION: u32 = 16;

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
            crate::FunctionFragmentFrameApplicationIdentity::from_bytes(cursor.array()?);
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
        let fuel_marker = cursor.u32()?;
        let fuel_schedule = FuelScheduleIdentity::new(fuel_marker)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::InvalidFuelSchedule)?;
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
        let placement_policy = match cursor.byte()? {
            1 => TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
            tag => {
                return Err(
                    FunctionFragmentTextSectionManifestDecodeError::UnknownPlacementPolicy(tag),
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
            fuel_schedule,
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
            placement_policy,
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
    crate::codec_bytes::push_encoded(&mut bytes, record.fuel_schedule.marker().to_le_bytes());
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
    bytes.push(match record.placement_policy {
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
