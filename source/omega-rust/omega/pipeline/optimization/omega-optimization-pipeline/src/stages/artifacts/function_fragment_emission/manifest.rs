use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, Optimization,
    OptimizationSelectionIdentity, PostAllocationOptimizationManifestIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::FuelScheduleIdentity;
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity};

use super::error::FunctionFragmentEmissionManifestDecodeError;
use super::model::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionSourceKind,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGFFE\0\0";
const MANIFEST_VERSION: u32 = 9;

impl FunctionFragmentEmissionManifest {
    pub fn recomputed_identity(&self) -> FunctionFragmentEmissionManifestIdentity {
        let mut canonical = b"omega.function-fragment-emission-manifest.v9\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(&canonical)
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

    pub fn decode(encoded: &[u8]) -> Result<Self, FunctionFragmentEmissionManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(FunctionFragmentEmissionManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
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
        let source_kind = match cursor.byte()? {
            1 => FunctionFragmentEmissionSourceKind::X86Rel8V1,
            2 => FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                optimization: decode_post_allocation_optimization(cursor.byte()?)?,
            },
            3 => FunctionFragmentEmissionSourceKind::AllocationRecoveryV1,
            4 => FunctionFragmentEmissionSourceKind::UnitBaselineV1,
            5 => FunctionFragmentEmissionSourceKind::StructuralUnitV1,
            6 => FunctionFragmentEmissionSourceKind::SelectedLoweringV1,
            tag => return Err(FunctionFragmentEmissionManifestDecodeError::UnknownSourceKind(tag)),
        };
        let source_realization =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?);
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let marker = u16::from_le_bytes(cursor.array()?);
        let vocabulary_marker = VocabularyMarker::new(marker)
            .ok_or(FunctionFragmentEmissionManifestDecodeError::UnknownVocabulary(marker))?;
        let psi = TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
        };
        let fuel_marker = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(fuel_marker)
            .ok_or(FunctionFragmentEmissionManifestDecodeError::InvalidFuelSchedule)?;
        let selected = omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes(
            cursor.array()?,
        );
        let post_allocation_manifest =
            PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let post_allocation_machine =
            omega_machine_optimizer::PostAllocationMachineIdentity::from_bytes(cursor.array()?);
        let final_pre_layout = SelectedFormEncodingIdentity::from_bytes(cursor.array()?);
        let final_resolved_layout =
            crate::ResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
        let whole_function_exit_contract =
            WholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
        let fragments = FunctionFragmentEmissionIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let statistics = FunctionFragmentEmissionStatistics {
            functions: u64::from_le_bytes(cursor.array()?),
            blocks: u64::from_le_bytes(cursor.array()?),
            instruction_spans: u64::from_le_bytes(cursor.array()?),
            zero_byte_instruction_spans: u64::from_le_bytes(cursor.array()?),
            bytes: u64::from_le_bytes(cursor.array()?),
            resolved_conditional_branches: u64::from_le_bytes(cursor.array()?),
            logical_fuel_settlements: u64::from_le_bytes(cursor.array()?),
            structural_unit_functions: u64::from_le_bytes(cursor.array()?),
            structural_unit_blocks: u64::from_le_bytes(cursor.array()?),
            structural_unit_instruction_spans: u64::from_le_bytes(cursor.array()?),
            structural_unit_bytes: u64::from_le_bytes(cursor.array()?),
            unresolved_internal_machine_fixups: u64::from_le_bytes(cursor.array()?),
            structural_logical_fuel_settlements: u64::from_le_bytes(cursor.array()?),
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
            source_kind,
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
    bytes.extend_from_slice(&record.statistics.functions.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.blocks.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.instruction_spans.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.zero_byte_instruction_spans.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.bytes.to_le_bytes());
    bytes.extend_from_slice(
        &record
            .statistics
            .resolved_conditional_branches
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&record.statistics.logical_fuel_settlements.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.structural_unit_functions.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.structural_unit_blocks.to_le_bytes());
    bytes.extend_from_slice(
        &record
            .statistics
            .structural_unit_instruction_spans
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&record.statistics.structural_unit_bytes.to_le_bytes());
    bytes.extend_from_slice(
        &record
            .statistics
            .unresolved_internal_machine_fixups
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &record
            .statistics
            .structural_logical_fuel_settlements
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&[1; 6]);
    bytes
}

fn decode_post_allocation_optimization(
    tag: u8,
) -> Result<Optimization, FunctionFragmentEmissionManifestDecodeError> {
    match tag {
        value if value == Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 as u8 => {
            Ok(Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1)
        }
        value
            if value == Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1 as u8 =>
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
        value
            if value
                == Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1 as u8 =>
        {
            Ok(Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1)
        }
        value if value == Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1 as u8 => {
            Ok(Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1)
        }
        value if value == Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1 as u8 => {
            Ok(Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1)
        }
        value => Err(
            FunctionFragmentEmissionManifestDecodeError::UnknownPostAllocationMachineOptimization(
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
) -> Result<NativeTarget, FunctionFragmentEmissionManifestDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => return Err(FunctionFragmentEmissionManifestDecodeError::UnknownArchitecture(tag)),
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => return Err(FunctionFragmentEmissionManifestDecodeError::UnknownObjectFormat(tag)),
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentEmissionManifestDecodeError::TargetLayoutOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentEmissionManifestDecodeError::TargetLayoutOverflow)?;
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
    ) -> Result<&'a [u8], FunctionFragmentEmissionManifestDecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(FunctionFragmentEmissionManifestDecodeError::Truncated)?;
        let result = self
            .bytes
            .get(self.position..end)
            .ok_or(FunctionFragmentEmissionManifestDecodeError::Truncated)?;
        self.position = end;
        Ok(result)
    }
    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], FunctionFragmentEmissionManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| FunctionFragmentEmissionManifestDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, FunctionFragmentEmissionManifestDecodeError> {
        Ok(self.take(1)?[0])
    }
    const fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }
}
