//! Rejoin selected origins in the complete retained call graph, not its legacy
//! pointer-only mirror. Inline aggregates and fresh results deliberately have
//! no mirror record; their selected contract still owns every operand and claim.

use crate::OptimizedBoundaryOccurrence;
use crate::physical::fragment_publication::FragmentPublicationBinding;
use semantic_vocabulary::MachineId;
use target_operations::CallSiteOwner;

pub(super) fn rejoin<'publication>(
    publication: &'publication FragmentPublicationBinding,
    occurrence: &OptimizedBoundaryOccurrence,
    callee: MachineId,
    field_offset: usize,
) -> Result<&'publication legalized_operations::LegalizedScalarCall, &'static str> {
    let owner = CallSiteOwner::from(occurrence.invocation_site());
    let mut functions = publication
        .selected_plan()
        .functions
        .iter()
        .filter(|function| function.machine == occurrence.machine());
    let (Some(function), None) = (functions.next(), functions.next()) else {
        return Err("selected boundary internal call lacks one selected caller");
    };
    let mut records = function.calls.iter().filter(|record| record.owner == owner);
    let (Some(record), None) = (records.next(), records.next()) else {
        return Err("selected boundary internal call has no unique retained origin");
    };
    let (block, instruction) =
        image_emission::unique_block_instruction(function, record.instruction)
            .ok_or("selected boundary internal call lacks its selected instruction")?;
    let retains_owner = match occurrence.invocation_site() {
        semantic_vocabulary::InvocationSite::Operation(operation) => {
            instruction.provenance.operations.contains(&operation)
        }
        semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
            instruction.provenance.edges.contains(&edge)
        }
    };
    if record.call.target.machine() != Some(callee)
        || instruction.kind.machine_callee() != Some(callee)
        || !retains_owner
    {
        return Err("selected boundary internal call changed its selected owner or callee");
    }
    let mut resolutions = publication
        .text_section()
        .resolved_internal_machine_calls
        .iter()
        .filter(|resolution| {
            resolution.caller == occurrence.machine() && resolution.owner == owner
        });
    let (Some(resolution), None) = (resolutions.next(), resolutions.next()) else {
        return Err("selected boundary internal call lacks one placed resolution");
    };
    if resolution.instruction != record.instruction
        || resolution.block != block.id
        || resolution.callee != callee
        || usize::try_from(resolution.field_section_offset) != Ok(field_offset)
    {
        return Err("selected boundary internal call changed its placed occurrence or callee");
    }
    Ok(&record.call)
}
