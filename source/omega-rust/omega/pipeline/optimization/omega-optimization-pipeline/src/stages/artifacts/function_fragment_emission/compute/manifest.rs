use omega_machine_code::FunctionFragmentEmissionPlan;
use omega_optimization_core::FunctionFragmentEmissionManifestIdentity;

use crate::FunctionRelativeOptimizationRealizationManifest;

use super::super::{
    FunctionFragmentEmissionError, FunctionFragmentEmissionManifest,
    FunctionFragmentEmissionSourceKind, FunctionFragmentEmissionStage,
    FunctionFragmentEmissionUnavailableData, ValidatedFunctionFragmentEmissionManifest,
};
use super::statistics;

pub(super) fn seal_ordinary(
    fragments: FunctionFragmentEmissionPlan,
    source: &FunctionRelativeOptimizationRealizationManifest,
    source_kind: FunctionFragmentEmissionSourceKind,
) -> Result<super::ordinary::Emission, FunctionFragmentEmissionError> {
    seal_for_fixups(fragments, source, source_kind)
}

pub(super) fn seal_structural(
    fragments: FunctionFragmentEmissionPlan,
    source: &FunctionRelativeOptimizationRealizationManifest,
) -> Result<super::ordinary::Emission, FunctionFragmentEmissionError> {
    seal_for_fixups(
        fragments,
        source,
        FunctionFragmentEmissionSourceKind::StructuralUnitV1,
    )
}

fn seal_for_fixups(
    fragments: FunctionFragmentEmissionPlan,
    source: &FunctionRelativeOptimizationRealizationManifest,
    source_kind: FunctionFragmentEmissionSourceKind,
) -> Result<super::ordinary::Emission, FunctionFragmentEmissionError> {
    let statistics = statistics::compute(&fragments)?;
    let stage = if statistics.unresolved_internal_machine_fixups == 0 {
        FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1
    } else {
        FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1
    };
    Ok(seal_with_statistics(
        fragments,
        source,
        source_kind,
        stage,
        statistics,
    ))
}

fn seal_with_statistics(
    fragments: FunctionFragmentEmissionPlan,
    source: &FunctionRelativeOptimizationRealizationManifest,
    source_kind: FunctionFragmentEmissionSourceKind,
    stage: FunctionFragmentEmissionStage,
    statistics: super::super::FunctionFragmentEmissionStatistics,
) -> super::ordinary::Emission {
    let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
    let mut record = FunctionFragmentEmissionManifest {
        identity: FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"pending"),
        stage,
        source_kind,
        source_realization: source.identity,
        selections: source.selections,
        psi: fragments.psi,
        fuel_schedule: fragments.fuel_schedule,
        selected: fragments.selected,
        post_allocation_manifest: source.post_allocation_manifest,
        post_allocation_machine: source.post_allocation_machine,
        final_pre_layout: source.pre_layout,
        final_resolved_layout: source.resolved_layout,
        whole_function_exit_contract: source.whole_function_exit_contract,
        fragments: fragments.identity,
        target: fragments.target,
        statistics,
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    (
        fragments,
        ValidatedFunctionFragmentEmissionManifest { record },
    )
}
