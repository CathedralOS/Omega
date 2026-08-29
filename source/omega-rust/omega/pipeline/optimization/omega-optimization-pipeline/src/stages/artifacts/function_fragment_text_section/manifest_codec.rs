use omega_object_file::{TextSectionPlacementPolicy, TextSectionRelocationRequirements};
use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, Optimization,
    OptimizationSelectionIdentity, PostAllocationOptimizationManifestIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    FunctionFragmentEmissionSourceKind, ResolvedSelectedFormLayoutIdentity,
    SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity,
};

use super::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGTSP\0\0";
const MANIFEST_VERSION: u32 = 9;

impl FunctionFragmentTextSectionManifest {
    pub fn recomputed_identity(&self) -> FunctionFragmentTextSectionManifestIdentity {
        let mut canonical = b"omega.function-fragment-text-section-manifest.v9\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_content(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(MANIFEST_MAGIC);
        encoded.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FunctionFragmentTextSectionManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(FunctionFragmentTextSectionManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != MANIFEST_VERSION {
            return Err(
                FunctionFragmentTextSectionManifestDecodeError::UnsupportedVersion(version),
            );
        }
        let identity = FunctionFragmentTextSectionManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => FunctionFragmentTextSectionStage::ValidatedRelocationFreeTextSectionPlacementV1,
            tag => return Err(FunctionFragmentTextSectionManifestDecodeError::UnknownStage(tag)),
        };
        let source_kind = match cursor.byte()? {
            1 => FunctionFragmentEmissionSourceKind::X86Rel8V1,
            2 => FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                optimization: decode_post_allocation_optimization(cursor.byte()?)?,
            },
            3 => FunctionFragmentEmissionSourceKind::AllocationRecoveryV1,
            4 => FunctionFragmentEmissionSourceKind::UnitBaselineV1,
            5 => FunctionFragmentEmissionSourceKind::StructuralUnitV1,
            6 => FunctionFragmentEmissionSourceKind::SelectedLoweringV1,
            tag => {
                return Err(FunctionFragmentTextSectionManifestDecodeError::UnknownSourceKind(tag));
            }
        };
        let source_fragment_manifest =
            FunctionFragmentEmissionManifestIdentity::from_bytes(cursor.array()?);
        let source_realization =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?);
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let marker = u16::from_le_bytes(cursor.array()?);
        let vocabulary_marker = VocabularyMarker::new(marker)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::UnknownVocabulary(marker))?;
        let psi = TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
        };
        let fuel_marker = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(fuel_marker)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::InvalidFuelSchedule)?;
        let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let post_allocation_manifest =
            PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let post_allocation_machine =
            omega_machine_optimizer::PostAllocationMachineIdentity::from_bytes(cursor.array()?);
        let final_pre_layout = SelectedFormEncodingIdentity::from_bytes(cursor.array()?);
        let final_resolved_layout = ResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
        let whole_function_exit_contract =
            WholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
        let fragments = FunctionFragmentEmissionIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let semantic_entry_raw = u64::from_le_bytes(cursor.array()?);
        let semantic_entry = MachineId::new(semantic_entry_raw)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::InvalidSemanticEntry)?;
        let semantic_entry_offset = u64::from_le_bytes(cursor.array()?);
        let placement_policy = match cursor.byte()? {
            1 => TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
            tag => {
                return Err(
                    FunctionFragmentTextSectionManifestDecodeError::UnknownPlacementPolicy(tag),
                );
            }
        };
        let text_section = TerminalRelocationFreeTextSectionIdentity::from_bytes(cursor.array()?);
        let relocation_requirements =
            match cursor.byte()? {
                1 => TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
                tag => return Err(
                    FunctionFragmentTextSectionManifestDecodeError::UnknownRelocationRequirements(
                        tag,
                    ),
                ),
            };
        let statistics = FunctionFragmentTextSectionStatistics {
            functions: u64::from_le_bytes(cursor.array()?),
            blocks: u64::from_le_bytes(cursor.array()?),
            instruction_spans: u64::from_le_bytes(cursor.array()?),
            zero_byte_instruction_spans: u64::from_le_bytes(cursor.array()?),
            bytes: u64::from_le_bytes(cursor.array()?),
            padding_bytes: u64::from_le_bytes(cursor.array()?),
            relocation_requirements: u64::from_le_bytes(cursor.array()?),
            structural_unit_functions: u64::from_le_bytes(cursor.array()?),
            structural_unit_blocks: u64::from_le_bytes(cursor.array()?),
            structural_unit_instruction_spans: u64::from_le_bytes(cursor.array()?),
            structural_unit_zero_byte_instruction_spans: u64::from_le_bytes(cursor.array()?),
            structural_unit_bytes: u64::from_le_bytes(cursor.array()?),
            source_internal_machine_fixups: u64::from_le_bytes(cursor.array()?),
            resolved_internal_machine_fixups: u64::from_le_bytes(cursor.array()?),
            remaining_internal_machine_fixups: u64::from_le_bytes(cursor.array()?),
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
            source_kind,
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
    bytes.push(1);
    match record.source_kind {
        FunctionFragmentEmissionSourceKind::X86Rel8V1 => bytes.push(1),
        FunctionFragmentEmissionSourceKind::SelectedLoweringV1 => bytes.push(6),
        FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
            optimization,
        } => {
            bytes.push(2);
            bytes.push(optimization as u8);
        }
        FunctionFragmentEmissionSourceKind::AllocationRecoveryV1 => bytes.push(3),
        FunctionFragmentEmissionSourceKind::UnitBaselineV1 => bytes.push(4),
        FunctionFragmentEmissionSourceKind::StructuralUnitV1 => bytes.push(5),
    }
    bytes.extend_from_slice(&record.source_fragment_manifest.bytes());
    bytes.extend_from_slice(&record.source_realization.bytes());
    bytes.extend_from_slice(&record.selections.bytes());
    bytes.extend_from_slice(&record.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(record.psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&record.fuel_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&record.selected.bytes());
    bytes.extend_from_slice(&record.post_allocation_manifest.bytes());
    bytes.extend_from_slice(&record.post_allocation_machine.bytes());
    bytes.extend_from_slice(&record.final_pre_layout.bytes());
    bytes.extend_from_slice(&record.final_resolved_layout.bytes());
    bytes.extend_from_slice(&record.whole_function_exit_contract.bytes());
    bytes.extend_from_slice(&record.fragments.bytes());
    encode_target(&mut bytes, record.target);
    bytes.extend_from_slice(&record.semantic_entry.get().to_le_bytes());
    bytes.extend_from_slice(&record.semantic_entry_offset.to_le_bytes());
    bytes.push(match record.placement_policy {
        TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1 => 1,
    });
    bytes.extend_from_slice(&record.text_section.bytes());
    bytes.push(match record.relocation_requirements {
        TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1 => 1,
    });
    bytes.extend_from_slice(&record.statistics.functions.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.blocks.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.instruction_spans.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.zero_byte_instruction_spans.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.bytes.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.padding_bytes.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.relocation_requirements.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.structural_unit_functions.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.structural_unit_blocks.to_le_bytes());
    bytes.extend_from_slice(
        &record
            .statistics
            .structural_unit_instruction_spans
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &record
            .statistics
            .structural_unit_zero_byte_instruction_spans
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&record.statistics.structural_unit_bytes.to_le_bytes());
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

fn decode_post_allocation_optimization(
    tag: u8,
) -> Result<Optimization, FunctionFragmentTextSectionManifestDecodeError> {
    match tag {
        value
            if value
                == Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 as u8 =>
        {
            Ok(Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1)
        }
        value
            if value
                == Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1 as u8 =>
        {
            Ok(Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1)
        }
        value if value == Optimization::X86SelectXorZeroI64MaterializationV1 as u8 => {
            Ok(Optimization::X86SelectXorZeroI64MaterializationV1)
        }
        value
            if value
                == Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1 as u8 =>
        {
            Ok(Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1)
        }
        value => Err(
            FunctionFragmentTextSectionManifestDecodeError::UnknownPostAllocationMachineOptimization(
                value,
            ),
        ),
    }
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, FunctionFragmentTextSectionManifestDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(FunctionFragmentTextSectionManifestDecodeError::UnknownArchitecture(tag));
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(FunctionFragmentTextSectionManifestDecodeError::UnknownObjectFormat(tag));
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentTextSectionManifestDecodeError::TargetLayoutOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentTextSectionManifestDecodeError::TargetLayoutOverflow)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], FunctionFragmentTextSectionManifestDecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::Truncated)?;
        let result = self
            .bytes
            .get(self.position..end)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::Truncated)?;
        self.position = end;
        Ok(result)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], FunctionFragmentTextSectionManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| FunctionFragmentTextSectionManifestDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, FunctionFragmentTextSectionManifestDecodeError> {
        Ok(self.take(1)?[0])
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }
}
