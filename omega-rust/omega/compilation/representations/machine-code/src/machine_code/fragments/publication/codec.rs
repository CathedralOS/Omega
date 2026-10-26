use crate::cursor::Cursor;
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;
use target::{NativeTarget, TargetWireIdentityError};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity};

use super::error::FunctionFragmentEmissionManifestDecodeError;
use super::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionStage,
    FunctionFragmentEmissionStatistics, FunctionFragmentEmissionUnavailableData,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGFFE\0\0";
const MANIFEST_VERSION: u32 = 15;

impl FunctionFragmentEmissionManifest {
    pub fn recomputed_identity(&self) -> FunctionFragmentEmissionManifestIdentity {
        let mut canonical = b"omega.function-fragment-emission-manifest.v15\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        crate::codec_bytes::encode_manifest_envelope(
            MANIFEST_MAGIC,
            MANIFEST_VERSION,
            &self.identity.bytes(),
            &encode_manifest_content(self),
        )
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FunctionFragmentEmissionManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(FunctionFragmentEmissionManifestDecodeError::WrongMagic);
        }
        let version = cursor.u32()?;
        if version != MANIFEST_VERSION {
            return Err(FunctionFragmentEmissionManifestDecodeError::UnsupportedVersion(version));
        }
        let identity = FunctionFragmentEmissionManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1,
            2 => FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1,
            tag => {
                return Err(FunctionFragmentEmissionManifestDecodeError::UnknownStage(
                    tag,
                ));
            }
        };
        let source_realization =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?);
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let marker = cursor.u16()?;
        let vocabulary_marker = VocabularyMarker::new(marker)
            .ok_or(FunctionFragmentEmissionManifestDecodeError::UnknownVocabulary(marker))?;
        let psi = TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
        };
        let fuel_marker = cursor.u32()?;
        let fuel_schedule = FuelScheduleIdentity::new(fuel_marker)
            .ok_or(FunctionFragmentEmissionManifestDecodeError::InvalidFuelSchedule)?;
        let selected =
            selected_instructions::SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let post_allocation_manifest =
            PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let post_allocation_machine =
            physical_instructions::PostAllocationMachineIdentity::from_bytes(cursor.array()?);
        let final_pre_layout = SelectedFormEncodingIdentity::from_bytes(cursor.array()?);
        let final_resolved_layout =
            crate::ResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
        let whole_function_exit_contract =
            WholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
        let fragments = FunctionFragmentEmissionIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let statistics = FunctionFragmentEmissionStatistics {
            functions: cursor.u64()?,
            blocks: cursor.u64()?,
            instruction_spans: cursor.u64()?,
            zero_byte_instruction_spans: cursor.u64()?,
            bytes: cursor.u64()?,
            resolved_conditional_branches: cursor.u64()?,
            logical_fuel_settlements: cursor.u64()?,
            unresolved_internal_machine_fixups: cursor.u64()?,
        };
        for _ in 0..6 {
            if cursor.byte()? != 1 {
                return Err(FunctionFragmentEmissionManifestDecodeError::UnknownUnavailableStatus);
            }
        }
        if cursor.remaining() != 0 {
            return Err(FunctionFragmentEmissionManifestDecodeError::TrailingBytes);
        }
        let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
        let record = Self {
            identity,
            stage,
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
            statistics,
            section_placement: unavailable,
            symbols: unavailable,
            object_relocations: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        if record.recomputed_identity() != identity {
            return Err(FunctionFragmentEmissionManifestDecodeError::IdentityMismatch);
        }
        Ok(record)
    }
}

fn encode_manifest_content(record: &FunctionFragmentEmissionManifest) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(match record.stage {
        FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1 => 1,
        FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1 => 2,
    });
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
    bytes.extend_from_slice(
        &record
            .statistics
            .resolved_conditional_branches
            .to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(
        &mut bytes,
        record.statistics.logical_fuel_settlements.to_le_bytes(),
    );
    bytes.extend_from_slice(
        &record
            .statistics
            .unresolved_internal_machine_fixups
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&[1; 6]);
    bytes
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, FunctionFragmentEmissionManifestDecodeError> {
    let encoded = cursor.array::<18>()?;
    NativeTarget::from_wire_identity_bytes(&encoded).map_err(|error| match error {
        TargetWireIdentityError::UnknownArchitecture(tag) => {
            FunctionFragmentEmissionManifestDecodeError::UnknownArchitecture(tag)
        }
        TargetWireIdentityError::UnknownObjectFormat(tag) => {
            FunctionFragmentEmissionManifestDecodeError::UnknownObjectFormat(tag)
        }
        TargetWireIdentityError::LayoutOverflow => {
            FunctionFragmentEmissionManifestDecodeError::TargetLayoutOverflow
        }
    })
}
