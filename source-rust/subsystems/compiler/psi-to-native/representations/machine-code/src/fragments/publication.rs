/*
Function-fragment emission manifest (`FunctionFragmentEmissionManifest`) and its
canonical content identity (`omega.function-fragment-emission-manifest.v15\0`).
*/

use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity,
};
use semantic_vocabulary::WorkScheduleIdentity;
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

use crate::{SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity};

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
    pub post_allocation_machine: post_allocation_machine::PostAllocationMachineIdentity,
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
