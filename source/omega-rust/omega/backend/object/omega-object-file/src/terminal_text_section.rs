use omega_optimization_core::{
    TerminalFunctionFragmentEmissionIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternativeFamily, TerminalMachineAlternativeKey, TerminalSelectedBlockId,
    TerminalSelectedInstructionId, TerminalSelectedInstructionPlanIdentity,
};
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;
use sha2::{Digest, Sha256};

const TEXT_SECTION_SCHEMA: &[u8] = b"omega.terminal.relocation-free-text-section.v1";

/// Exact deterministic placement used by the first clean Terminal text-section boundary.
///
/// Functions remain in the already validated fragment order. No sorting, padding, symbol
/// assignment, or object-container policy is implied by this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTextSectionPlacementPolicy {
    DenseValidatedFragmentOrderNoPaddingV1,
}

/// Closed relocation conclusion for the currently admitted clean Terminal instruction set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTextSectionRelocationRequirements {
    ProvenNoneForFullyResolvedInternalControlV1,
}

/// One relocation-free, section-relative concatenation of validated function fragments.
///
/// This is representation data only. It is not an [`ObjectPlan`], has no symbols or external
/// entry point, and grants no object serialization, image, installation, or publication
/// authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRelocationFreeTextSectionPlacement {
    pub identity: TerminalRelocationFreeTextSectionIdentity,
    pub source_fragments: TerminalFunctionFragmentEmissionIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_offset: u64,
    pub policy: TerminalTextSectionPlacementPolicy,
    pub section_alignment: u64,
    pub byte_count: u64,
    pub bytes: Vec<u8>,
    pub functions: Vec<TerminalPlacedFunctionFragment>,
    pub relocation_requirements: TerminalTextSectionRelocationRequirements,
}

impl TerminalRelocationFreeTextSectionPlacement {
    pub fn recomputed_identity(&self) -> TerminalRelocationFreeTextSectionIdentity {
        terminal_relocation_free_text_section_identity(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPlacedFunctionFragment {
    pub source_function_index: u64,
    pub machine: MachineId,
    pub section_offset: u64,
    pub byte_count: u64,
    pub blocks: Vec<TerminalPlacedBlockSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPlacedBlockSpan {
    pub block: TerminalSelectedBlockId,
    pub function_offset: u64,
    pub section_offset: u64,
    pub byte_count: u64,
    pub instructions: Vec<TerminalPlacedInstructionSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPlacedInstructionSpan {
    pub instruction: TerminalSelectedInstructionId,
    pub alternative: TerminalMachineAlternativeKey,
    pub function_offset: u64,
    pub section_offset: u64,
    pub byte_count: u64,
}

pub fn terminal_relocation_free_text_section_identity(
    section: &TerminalRelocationFreeTextSectionPlacement,
) -> TerminalRelocationFreeTextSectionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(TEXT_SECTION_SCHEMA);
    hasher.update(section.source_fragments.bytes());
    hasher.update(section.terminal_psi.vocabulary_marker.get().to_le_bytes());
    hasher.update(section.terminal_psi.program_fingerprint.as_bytes());
    hasher.update(section.fuel_schedule.marker().to_le_bytes());
    hasher.update(section.selected.bytes());
    encode_target(&mut hasher, section.target);
    hasher.update(section.semantic_entry.get().to_le_bytes());
    hasher.update(section.semantic_entry_offset.to_le_bytes());
    hasher.update([match section.policy {
        TerminalTextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1 => 1,
    }]);
    hasher.update(section.section_alignment.to_le_bytes());
    hasher.update(section.byte_count.to_le_bytes());
    encode_bytes(&mut hasher, &section.bytes);
    hasher.update((section.functions.len() as u64).to_le_bytes());
    for function in &section.functions {
        hasher.update(function.source_function_index.to_le_bytes());
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.section_offset.to_le_bytes());
        hasher.update(function.byte_count.to_le_bytes());
        hasher.update((function.blocks.len() as u64).to_le_bytes());
        for block in &function.blocks {
            hasher.update(block.block.0.to_le_bytes());
            hasher.update(block.function_offset.to_le_bytes());
            hasher.update(block.section_offset.to_le_bytes());
            hasher.update(block.byte_count.to_le_bytes());
            hasher.update((block.instructions.len() as u64).to_le_bytes());
            for instruction in &block.instructions {
                hasher.update(instruction.instruction.0.to_le_bytes());
                encode_alternative(&mut hasher, instruction.alternative);
                hasher.update(instruction.function_offset.to_le_bytes());
                hasher.update(instruction.section_offset.to_le_bytes());
                hasher.update(instruction.byte_count.to_le_bytes());
            }
        }
    }
    hasher.update([match section.relocation_requirements {
        TerminalTextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1 => 1,
    }]);
    TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(&hasher.finalize())
}

fn encode_target(hasher: &mut Sha256, target: NativeTarget) {
    hasher.update([match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }]);
    hasher.update([match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }]);
    hasher.update((target.pointer_size as u64).to_le_bytes());
    hasher.update((target.pointer_alignment as u64).to_le_bytes());
}

fn encode_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn encode_alternative(hasher: &mut Sha256, alternative: TerminalMachineAlternativeKey) {
    hasher.update([match alternative.family {
        TerminalMachineAlternativeFamily::CompareI64Zero => 0,
        TerminalMachineAlternativeFamily::MaterializeI64 => 1,
        TerminalMachineAlternativeFamily::CopyI64 => 2,
        TerminalMachineAlternativeFamily::ExactAddI64 => 3,
        TerminalMachineAlternativeFamily::ExactAddI64Immediate => 4,
        TerminalMachineAlternativeFamily::ExactSubtractI64 => 5,
        TerminalMachineAlternativeFamily::ConditionalBranchNonZero => 6,
        TerminalMachineAlternativeFamily::ReturnI64 => 7,
        TerminalMachineAlternativeFamily::ExactSubtractI64Immediate => 8,
    }]);
    hasher.update(alternative.variant.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_terminal::{SemanticFingerprint, VocabularyMarker};

    #[test]
    fn text_section_identity_binds_zero_spans_and_section_coordinates() {
        let mut section = TerminalRelocationFreeTextSectionPlacement {
            identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"pending"),
            source_fragments: TerminalFunctionFragmentEmissionIdentity::from_canonical_bytes(
                b"fragments",
            ),
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            selected: TerminalSelectedInstructionPlanIdentity::from_bytes([8; 32]),
            target: NativeTarget::linux_arm64(),
            semantic_entry: MachineId::new(1).unwrap(),
            semantic_entry_offset: 0,
            policy: TerminalTextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
            section_alignment: 4,
            byte_count: 4,
            bytes: vec![0x20, 0, 0, 0xb5],
            functions: vec![TerminalPlacedFunctionFragment {
                source_function_index: 0,
                machine: MachineId::new(1).unwrap(),
                section_offset: 0,
                byte_count: 4,
                blocks: vec![TerminalPlacedBlockSpan {
                    block: TerminalSelectedBlockId(1),
                    function_offset: 0,
                    section_offset: 0,
                    byte_count: 4,
                    instructions: vec![
                        TerminalPlacedInstructionSpan {
                            instruction: TerminalSelectedInstructionId(1),
                            alternative: TerminalMachineAlternativeKey {
                                family: TerminalMachineAlternativeFamily::CompareI64Zero,
                                variant: 1,
                            },
                            function_offset: 0,
                            section_offset: 0,
                            byte_count: 0,
                        },
                        TerminalPlacedInstructionSpan {
                            instruction: TerminalSelectedInstructionId(2),
                            alternative: TerminalMachineAlternativeKey {
                                family: TerminalMachineAlternativeFamily::ConditionalBranchNonZero,
                                variant: 2,
                            },
                            function_offset: 0,
                            section_offset: 0,
                            byte_count: 4,
                        },
                    ],
                }],
            }],
            relocation_requirements: TerminalTextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
        };
        section.identity = section.recomputed_identity();
        let original = section.identity;
        section.functions[0].blocks[0].instructions[0].section_offset = 4;
        assert_ne!(section.recomputed_identity(), original);
    }
}
