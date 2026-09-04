pub(super) mod alignment;
pub(super) mod block_spans;
mod relocation_custody;

use std::collections::BTreeSet;

use omega_machine_code::FunctionFragmentEmissionPlan;
use omega_object_file::{
    PlacedFunctionFragment, RelocationFreeTextSectionPlacement, TextSectionPlacementPolicy,
    TextSectionRelocationRequirements,
};
use omega_optimization_core::TerminalRelocationFreeTextSectionIdentity;
use omega_target::Architecture;

use super::super::RelocationFreeTextSectionPlacementError;
use super::conversion::usize_to_u64;

pub(super) fn place(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    let section_alignment = match fragments.target.architecture {
        Architecture::Aarch64 => 4,
        Architecture::X86_64 => 1,
    };
    let mut bytes = Vec::new();
    let mut functions = Vec::with_capacity(fragments.functions.len());
    let mut seen_machines = BTreeSet::new();
    let mut semantic_entry_offset = None;

    for (source_function_index, function) in fragments.functions.iter().enumerate() {
        if !seen_machines.insert(function.machine) {
            return Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(
                function.machine,
            ));
        }
        let section_offset = usize_to_u64(bytes.len())?;
        if function.machine == fragments.entry
            && semantic_entry_offset.replace(section_offset).is_some()
        {
            return Err(
                RelocationFreeTextSectionPlacementError::DuplicateSemanticEntry(fragments.entry),
            );
        }
        alignment::validate(
            fragments.target.architecture,
            section_offset,
            function.byte_count,
        )?;
        relocation_custody::prove_none(function)?;
        let blocks = block_spans::place(fragments.target.architecture, function, section_offset)?;
        let function_start = bytes.len();
        bytes.extend_from_slice(&function.bytes);
        if usize_to_u64(bytes.len().saturating_sub(function_start))? != function.byte_count {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        functions.push(PlacedFunctionFragment {
            source_function_index: usize_to_u64(source_function_index)?,
            machine: function.machine,
            section_offset,
            byte_count: function.byte_count,
            blocks,
        });
    }

    let semantic_entry_offset = semantic_entry_offset
        .ok_or(RelocationFreeTextSectionPlacementError::MissingSemanticEntry(fragments.entry))?;
    let byte_count = usize_to_u64(bytes.len())?;
    let mut text_section = RelocationFreeTextSectionPlacement {
        identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"pending"),
        source_fragments: fragments.identity,
        psi: fragments.psi,
        fuel_schedule: fragments.fuel_schedule,
        selected: fragments.selected,
        target: fragments.target,
        semantic_entry: fragments.entry,
        semantic_entry_offset,
        policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
        section_alignment,
        byte_count,
        bytes,
        functions,
        resolved_internal_machine_calls: Vec::new(),
        relocation_requirements:
            TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    text_section.identity = text_section.recomputed_identity();
    Ok(text_section)
}
