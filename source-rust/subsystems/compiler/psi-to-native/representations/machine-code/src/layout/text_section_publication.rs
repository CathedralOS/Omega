/*
Text-section publication manifest (`FunctionFragmentTextSectionManifest`) and
its canonical content identity (`omega.function-fragment-text-section-manifest.v16\0`).
*/

use super::{TextSectionPlacementPolicy, TextSectionRelocationRequirements};
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

use crate::{
    FunctionFragmentFrameApplicationIdentity, ResolvedSelectedFormLayoutIdentity,
    SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity,
};

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
    pub post_allocation_machine: post_allocation_machine::PostAllocationMachineIdentity,
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

impl FunctionFragmentTextSectionManifest {
    pub fn recomputed_identity(&self) -> FunctionFragmentTextSectionManifestIdentity {
        let mut canonical = b"omega.function-fragment-text-section-manifest.v16\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(&canonical)
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
