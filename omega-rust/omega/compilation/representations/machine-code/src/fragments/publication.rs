/*
Function-fragment emission manifest (`FunctionFragmentEmissionManifest`) and its self-authenticating
wire codec (`OMGFFE\0\0` v15, schema `omega.function-fragment-emission-manifest.v15\0`).

Decoding and recomputing a manifest identity check wire framing and digest integrity only; stage
labels and summary statistics are retained claims that must be verified against admitted fragments.
*/

use crate::cursor::Cursor;
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity,
};
use semantic_vocabulary::WorkScheduleIdentity;
use target::{NativeTarget, TargetWireIdentityError};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGFFE\0\0";
const MANIFEST_VERSION: u32 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownVocabulary(u16),
    InvalidWorkSchedule,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    UnknownUnavailableStatus,
    IdentityMismatch,
    TrailingBytes,
}

optimization_core::truncated_from_adapter!(FunctionFragmentEmissionManifestDecodeError);

register_homes::decode_error_display! {
    FunctionFragmentEmissionManifestDecodeError,
    "invalid function-fragment emission manifest"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionStage {
    ValidatedRelocationFreeFunctionFragmentsV1,
    ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionFragmentEmissionStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub instruction_spans: u64,
    pub zero_byte_instruction_spans: u64,
    pub bytes: u64,
    pub resolved_conditional_branches: u64,
    pub logical_work_settlements: u64,
    pub unresolved_internal_machine_fixups: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentEmissionManifest {
    pub identity: FunctionFragmentEmissionManifestIdentity,
    pub stage: FunctionFragmentEmissionStage,
    pub source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub selected: selected_instructions::SelectedInstructionPlanIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: physical_instructions::PostAllocationMachineIdentity,
    pub final_pre_layout: SelectedFormEncodingIdentity,
    pub final_resolved_layout: crate::ResolvedSelectedFormLayoutIdentity,
    pub whole_function_exit_contract: WholeFunctionExitContractIdentity,
    pub fragments: FunctionFragmentEmissionIdentity,
    pub target: NativeTarget,
    pub statistics: FunctionFragmentEmissionStatistics,
    pub section_placement: FunctionFragmentEmissionUnavailableData,
    pub symbols: FunctionFragmentEmissionUnavailableData,
    pub object_relocations: FunctionFragmentEmissionUnavailableData,
    pub executable_image: FunctionFragmentEmissionUnavailableData,
    pub installation: FunctionFragmentEmissionUnavailableData,
    pub publication: FunctionFragmentEmissionUnavailableData,
}

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
        let work_marker = cursor.u32()?;
        let work_schedule = WorkScheduleIdentity::new(work_marker)
            .ok_or(FunctionFragmentEmissionManifestDecodeError::InvalidWorkSchedule)?;
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
            logical_work_settlements: cursor.u64()?,
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
            work_schedule,
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
    crate::codec_bytes::push_encoded(&mut bytes, record.work_schedule.marker().to_le_bytes());
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
        record.statistics.logical_work_settlements.to_le_bytes(),
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

#[cfg(test)]
mod tests {
    use super::{
        FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifest,
        FunctionFragmentEmissionManifestDecodeError, FunctionFragmentEmissionManifestIdentity,
        FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
        FunctionFragmentEmissionUnavailableData,
        FunctionRelativeOptimizationRealizationManifestIdentity, NativeTarget,
        OptimizationSelectionIdentity, PostAllocationOptimizationManifestIdentity,
        SelectedFormEncodingIdentity, TerminalPsiIdentity, WholeFunctionExitContractIdentity,
        WorkScheduleIdentity,
    };

    #[test]
    fn prior_wire_versions_with_route_taxonomy_reject() {
        let encoded = record().encode();
        for version in 0..15_u32 {
            let mut stale = encoded.clone();
            crate::codec_bytes::patch_encoded(&mut stale, 8, version.to_le_bytes());
            assert_eq!(
                FunctionFragmentEmissionManifest::decode(&stale),
                Err(FunctionFragmentEmissionManifestDecodeError::UnsupportedVersion(version))
            );
        }
    }

    fn record() -> FunctionFragmentEmissionManifest {
        let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
        let mut record = FunctionFragmentEmissionManifest {
            identity: FunctionFragmentEmissionManifestIdentity::from_bytes([0; 32]),
            stage: FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1,
            source_realization: FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(
                [1; 32],
            ),
            selections: OptimizationSelectionIdentity::from_bytes([2; 32]),
            psi: TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([3; 32]),
            },
            work_schedule: WorkScheduleIdentity::new(4).unwrap(),
            selected: selected_instructions::SelectedInstructionPlanIdentity::from_bytes([5; 32]),
            post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes(
                [6; 32],
            ),
            post_allocation_machine:
                physical_instructions::PostAllocationMachineIdentity::from_bytes([7; 32]),
            final_pre_layout: SelectedFormEncodingIdentity::from_bytes([8; 32]),
            final_resolved_layout: crate::ResolvedSelectedFormLayoutIdentity::from_bytes([9; 32]),
            whole_function_exit_contract: WholeFunctionExitContractIdentity::from_bytes([10; 32]),
            fragments: FunctionFragmentEmissionIdentity::from_bytes([11; 32]),
            target: NativeTarget::linux_x64(),
            statistics: FunctionFragmentEmissionStatistics {
                functions: 1,
                blocks: 2,
                instruction_spans: 3,
                zero_byte_instruction_spans: 4,
                bytes: 5,
                resolved_conditional_branches: 6,
                logical_work_settlements: 7,
                unresolved_internal_machine_fixups: 12,
            },
            section_placement: unavailable,
            symbols: unavailable,
            object_relocations: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        record.identity = record.recomputed_identity();
        record
    }

    #[test]
    fn publication_roundtrips_without_route_taxonomy() {
        let record = record();
        let bytes = record.encode();
        assert_eq!(&bytes[..8], b"OMGFFE\0\0");
        assert_eq!(&bytes[8..12], &15_u32.to_le_bytes());
        assert_eq!(bytes.len(), 459);
        assert_eq!(FunctionFragmentEmissionManifest::decode(&bytes), Ok(record));
    }

    #[test]
    fn publication_codec_checks_integrity_not_the_truth_of_claimed_statistics() {
        let mut record = record();
        let mut encoded = record.encode();
        encoded[12] ^= 1;
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&encoded),
            Err(FunctionFragmentEmissionManifestDecodeError::IdentityMismatch),
        );

        record.statistics.bytes = u64::MAX;
        record.identity = record.recomputed_identity();
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&record.encode()),
            Ok(record)
        );
    }
}
