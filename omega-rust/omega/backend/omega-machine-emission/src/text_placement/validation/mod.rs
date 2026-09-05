//! Optimizer module role: executable entrance. Check placement without building another section.
mod calls;
mod functions;
mod ranges;
mod spans;
use ranges::{add, bytes, unchanged_bytes};

use super::{TextPlacementError, TextPlacementInput};
use omega_machine_code::{
    RelocationFreeTextSectionPlacement, TextSectionPlacementPolicy,
    TextSectionRelocationRequirements,
};
use omega_target::Architecture;
use std::collections::BTreeMap;

pub(super) fn check(
    input: TextPlacementInput<'_>,
    section: &RelocationFreeTextSectionPlacement,
) -> Result<(), TextPlacementError> {
    let fragments = input.fragments();
    let structural = matches!(input, TextPlacementInput::Structural { .. });
    let count = if structural {
        fragments.structural_unit_functions.len()
    } else {
        fragments.functions.len()
    };
    let alignment = match fragments.target.architecture {
        Architecture::X86_64 => 1,
        Architecture::Aarch64 => 4,
    };
    if section.identity != section.recomputed_identity()
        || section.source_fragments != fragments.identity
        || section.psi != fragments.psi
        || section.fuel_schedule != fragments.fuel_schedule
        || section.selected != fragments.selected
        || section.target != fragments.target
        || section.semantic_entry != fragments.entry
        || section.functions.len() != count
        || section.policy != TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1
        || section.relocation_requirements
            != TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
        || section.section_alignment != alignment
        || section.byte_count != section.bytes.len() as u64
    {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    let mut offsets = BTreeMap::new();
    let mut extent = 0;
    if structural {
        for function in &fragments.structural_unit_functions {
            functions::source_function(
                &mut offsets,
                &mut extent,
                function.machine,
                function.byte_count,
                &function.bytes,
                alignment,
            )?;
        }
    } else {
        for function in &fragments.functions {
            functions::source_function(
                &mut offsets,
                &mut extent,
                function.machine,
                function.byte_count,
                &function.bytes,
                alignment,
            )?;
        }
    }
    let entry = offsets
        .get(&fragments.entry)
        .ok_or(TextPlacementError::MissingSemanticEntry(fragments.entry))?;
    if section.semantic_entry_offset != *entry || section.byte_count != extent {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    functions::check(input, section, &offsets, alignment)
}
