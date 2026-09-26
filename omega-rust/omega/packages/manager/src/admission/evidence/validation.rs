use super::AcceptedOrdinaryEvidenceError;
use crate::resolution::graph::ResolvedPackageSourceClosure;
use package_source::local::operations::verify_package_source_snapshot;

pub(super) fn revalidate_source_custody(
    closure: &ResolvedPackageSourceClosure,
) -> Result<(), AcceptedOrdinaryEvidenceError> {
    for custody in closure.custodies() {
        verify_package_source_snapshot(
            custody.snapshot_root(),
            custody.materialization().content(),
            custody.source_limits(),
        )
        .map_err(|error| AcceptedOrdinaryEvidenceError::SourceCustody {
            package: custody.key().clone(),
            error,
        })?;
        custody.selection_evidence().revalidate().map_err(|error| {
            AcceptedOrdinaryEvidenceError::SourceSelectionCustody {
                package: custody.key().clone(),
                error,
            }
        })?;
    }
    Ok(())
}
