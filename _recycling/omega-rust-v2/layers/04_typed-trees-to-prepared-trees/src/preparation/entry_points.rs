use super::rounds::{PreparationFailure, PreparationRound};
use crate::{PreparationProgress, PreparedTrees, TargetSemantics};
use omega_typed::TypeError;
use omega_typer::SourcePackage;

/// Prepare until all requested identities are frozen or until the first pass
/// whose remaining ready requests need a target. Retain the open source
/// recipes rather than treating incomplete typing as a prepared package.
pub fn prepare_source_package_with_open_target<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typed::BoundarySlot],
) -> Result<PreparationProgress<'text>, PreparationFailure> {
    PreparationRound::begin(source, boundary_slots, None)?.finish_without_external_evidence()
}

/// Prepare the static values demanded by one admitted source package.
pub fn prepare_source_package<'text>(source: &SourcePackage<'text>) -> Result<PreparedTrees<'text>, TypeError> {
    prepare_source_package_diagnosed(source).map_err(TypeError::from)
}

/// Report whether typing, static evaluation, or missing target input stopped preparation.
pub fn prepare_source_package_diagnosed<'text>(source: &SourcePackage<'text>) -> Result<PreparedTrees<'text>, PreparationFailure> {
    prepare_source_package_diagnosed_with_inputs(source, &[], None)
}

/// Report the refusing phase together with the typed cause of an L4 refusal.
/// Without a target, a demand that needs one is missing target input.
pub fn prepare_source_package_diagnosed_with_inputs<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typed::BoundarySlot],
    target: Option<&TargetSemantics>,
) -> Result<PreparedTrees<'text>, PreparationFailure> {
    PreparationRound::begin(source, boundary_slots, target)?.finish_without_external_evidence()?
        .into_complete()
}

/// Use the same Build-selected boundary slots in every typing pass, including
/// the strict pass that checks the computed identities.
pub fn prepare_source_package_with_boundary_slots<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typed::BoundarySlot],
) -> Result<PreparedTrees<'text>, TypeError> {
    prepare(source, boundary_slots, None)
}

/// Prepare under the selected target, using the same layout and realization inputs across typing retries.
pub fn prepare_source_package_for_target<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typed::BoundarySlot],
    target: &TargetSemantics,
) -> Result<PreparedTrees<'text>, TypeError> {
    prepare(source, boundary_slots, Some(target))
}

fn prepare<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typed::BoundarySlot],
    target: Option<&TargetSemantics>,
) -> Result<PreparedTrees<'text>, TypeError> {
    prepare_source_package_diagnosed_with_inputs(source, boundary_slots, target).map_err(TypeError::from)
}
