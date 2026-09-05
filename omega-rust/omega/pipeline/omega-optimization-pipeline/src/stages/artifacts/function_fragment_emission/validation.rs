//! Check publication metadata against admitted source facts and checked fragments.

use super::{
    FunctionFragmentEmissionError, FunctionFragmentEmissionSourceKind,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionUnavailableData,
    StagedOptimizedFunctionFragmentEmission,
};

pub(super) fn manifest(
    staged: &StagedOptimizedFunctionFragmentEmission,
) -> Result<(), FunctionFragmentEmissionError> {
    let current = &staged.source;
    let source = current.function_relative_manifest().record();
    let fragments = &staged.fragments;
    let record = staged.manifest.record();
    let counts = omega_machine_emission::function_fragment_emission_statistics(fragments)?;
    let source_kind = if current
        .program()
        .selected
        .structural_unit_functions
        .is_empty()
    {
        current.source_kind()
    } else {
        FunctionFragmentEmissionSourceKind::StructuralUnitV1
    };
    let stage = if counts.unresolved_internal_machine_fixups == 0 {
        FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1
    } else {
        FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1
    };
    let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
    if source.selected != fragments.selected
        || source.resolved_layout != current.program().layout.identity
        || current.post_allocation_manifest().record().selected != fragments.selected
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    if record.identity != record.recomputed_identity()
        || record.stage != stage
        || record.source_kind != source_kind
        || record.source_realization != source.identity
        || record.selections != source.selections
        || record.psi != fragments.psi
        || record.fuel_schedule != fragments.fuel_schedule
        || record.selected != fragments.selected
        || record.post_allocation_manifest != source.post_allocation_manifest
        || record.post_allocation_machine != source.post_allocation_machine
        || record.final_pre_layout != source.pre_layout
        || record.final_resolved_layout != source.resolved_layout
        || record.whole_function_exit_contract != source.whole_function_exit_contract
        || record.fragments != fragments.identity
        || record.target != fragments.target
        || record.statistics != counts
        || record.section_placement != unavailable
        || record.symbols != unavailable
        || record.object_relocations != unavailable
        || record.executable_image != unavailable
        || record.installation != unavailable
        || record.publication != unavailable
    {
        return Err(FunctionFragmentEmissionError::ManifestMismatch);
    }
    Ok(())
}
