/*
We define the object-container publication manifest
(`FunctionFragmentObjectContainerManifest`), its summary statistics
(`FunctionFragmentObjectContainerStatistics`), and the canonical content its
identity covers. These records capture object-format publication claims and
record counts without granting source admission or executable publication
authority.
*/

use crate::{
    ObjectLocalSymbolId, RelocationFreeObjectContainer, RelocationFreeObjectError,
    RelocationFreeObjectPlan, RelocationFreeObjectRelocationRequirements,
    RelocationFreeObjectSymbolPolicy,
};
use optimization_core::{
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    OptimizationSelectionIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentObjectContainerStage {
    ValidatedRelocationFreeObjectContainerV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentObjectContainerUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionFragmentObjectContainerStatistics {
    pub sections: u64,
    pub function_symbols: u64,
    pub object_local_symbols: u64,
    pub external_symbols: u64,
    pub text_bytes: u64,
    pub container_bytes: u64,
    pub relocation_records: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentObjectContainerManifest {
    pub identity: FunctionFragmentObjectContainerManifestIdentity,
    pub stage: FunctionFragmentObjectContainerStage,
    pub source_text_section_manifest: FunctionFragmentTextSectionManifestIdentity,
    pub text_section: TerminalRelocationFreeTextSectionIdentity,
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_symbol: ObjectLocalSymbolId,
    pub symbol_policy: RelocationFreeObjectSymbolPolicy,
    pub object: RelocationFreeObjectPlanIdentity,
    pub object_container: RelocationFreeObjectContainerIdentity,
    pub relocation_requirements: RelocationFreeObjectRelocationRequirements,
    pub statistics: FunctionFragmentObjectContainerStatistics,
    pub external_entry_bridge: FunctionFragmentObjectContainerUnavailableData,
    pub executable_image: FunctionFragmentObjectContainerUnavailableData,
    pub installation: FunctionFragmentObjectContainerUnavailableData,
    pub publication: FunctionFragmentObjectContainerUnavailableData,
}

pub(crate) fn relocation_free_object_statistics(
    object: &RelocationFreeObjectPlan,
    container: &RelocationFreeObjectContainer,
) -> Result<FunctionFragmentObjectContainerStatistics, RelocationFreeObjectError> {
    let symbols = u64::try_from(object.symbols.len())
        .map_err(|_| RelocationFreeObjectError::LengthOverflow)?;
    let container_bytes = u64::try_from(container.bytes.len())
        .map_err(|_| RelocationFreeObjectError::LengthOverflow)?;
    Ok(FunctionFragmentObjectContainerStatistics {
        sections: 1,
        function_symbols: symbols,
        object_local_symbols: symbols,
        external_symbols: u64::try_from(object.normalized_imports.len())
            .map_err(|_| RelocationFreeObjectError::LengthOverflow)?,
        text_bytes: object.text_section.byte_count,
        container_bytes,
        relocation_records: object.relocation_record_count,
    })
}

impl FunctionFragmentObjectContainerManifest {
    pub fn recomputed_identity(&self) -> FunctionFragmentObjectContainerManifestIdentity {
        let mut canonical = b"omega.function-fragment-object-container-manifest.v1\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(&canonical)
    }
}

fn encode_manifest_content(record: &FunctionFragmentObjectContainerManifest) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(1);
    bytes.extend_from_slice(&record.source_text_section_manifest.bytes());
    bytes.extend_from_slice(&record.text_section.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(record.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.work_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&record.selections.bytes());
    bytes.extend_from_slice(&record.selected.bytes());
    bytes.extend_from_slice(&record.target.wire_identity_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.semantic_entry.get().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.semantic_entry_symbol.get().to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&record.object.bytes());
    bytes.extend_from_slice(&record.object_container.bytes());
    bytes.push(match record.relocation_requirements {
        RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1 => {
            1
        }
        RelocationFreeObjectRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1 => 2,
        RelocationFreeObjectRelocationRequirements::UnresolvedObjectSymbolFieldsV1 => 3,
    });
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.sections.to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.function_symbols.to_le_bytes());
    crate::codec_bytes::push_encoded(
        &mut bytes,
        record.statistics.object_local_symbols.to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.external_symbols.to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.text_bytes.to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.container_bytes.to_le_bytes());
    crate::codec_bytes::push_encoded(
        &mut bytes,
        record.statistics.relocation_records.to_le_bytes(),
    );
    bytes.extend_from_slice(&[1; 4]);
    bytes
}
