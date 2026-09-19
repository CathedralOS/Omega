//! Optimizer module role: executable entrance. Check placement without building another section.
mod calls;
mod functions;
mod ranges;
mod spans;
use ranges::{add, bytes, unchanged_bytes};

use super::{TextPlacementError, TextPlacementInput};
use machine_code::{
    RelocationFreeTextSectionPlacement, TextSectionPlacementPolicy,
    TextSectionRelocationRequirements,
};
use std::collections::BTreeMap;
use target::Architecture;

pub(super) fn check(
    input: TextPlacementInput<'_>,
    section: &RelocationFreeTextSectionPlacement,
) -> Result<(), TextPlacementError> {
    let fragments = input.fragments();
    let count = fragments.functions.len();
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
        || section.relocation_requirements != expected_requirements(fragments)
        || section.section_alignment != alignment
        || section.byte_count != section.bytes.len() as u64
    {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    let mut offsets = BTreeMap::new();
    let mut extent = 0;
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

    let entry = offsets
        .get(&fragments.entry)
        .ok_or(TextPlacementError::MissingSemanticEntry(fragments.entry))?;
    if section.semantic_entry_offset != *entry || section.byte_count != extent {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    functions::check(input, section, &offsets, alignment)
}

/// The section may close as fully internal only when the source carries no
/// normalized foreign call fields; otherwise the manifest must declare the
/// unresolved import-field relocation class.
fn expected_requirements(
    fragments: &machine_code::FunctionFragmentEmissionPlan,
) -> TextSectionRelocationRequirements {
    let has_foreign = fragments
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .any(|row| row.normalized_foreign_call_fixup.is_some());
    if has_foreign {
        TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1
    } else {
        TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
    }
}
