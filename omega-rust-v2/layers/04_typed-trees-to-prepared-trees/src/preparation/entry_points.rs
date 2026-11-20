use super::{PreparationState, prepare_progress};
use crate::{PreparationProgress, PreparedTrees, TargetSemantics};
use omega_typed::TypeError;
use omega_typer::SourcePackage;

/// Prepare until all requested identities are frozen or until the first pass
/// whose remaining ready requests need a target. Retain the open source
/// recipes rather than treating incomplete typing as a prepared package.
pub fn prepare_source_package_with_open_target<'text>(
    source: &SourcePackage<'text>,
    boundary_slots: &[omega_typed::BoundarySlot],
) -> Result<PreparationProgress<'text>, TypeError> {
    prepare_progress(source, boundary_slots, None, PreparationState::default(), None)
}

/// Prepare the static values demanded by one admitted source package.
pub fn prepare_source_package<'text>(source: &SourcePackage<'text>) -> Result<PreparedTrees<'text>, TypeError> {
    prepare_source_package_with_boundary_slots(source, &[])
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
    prepare_progress(source, boundary_slots, target, PreparationState::default(), None)?.into_complete()
}
