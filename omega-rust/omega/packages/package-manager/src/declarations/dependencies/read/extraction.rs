//! Locate conservative dependency edits in authored source; never resolve a graph.
use super::error::DependencyProjectionError;
use super::projection::extract_scoped_requests_from_source;
use super::{DependencyPurpose, DependencySourceRequest};

/// Project every unconditional direct dependency row in authored order,
/// tagged with the scope it authorizes. Edit planning needs this one flat
/// list to correlate rows and requests across both purposes.
pub(crate) fn extract_scoped_from_source(
    source: &str,
) -> Result<Vec<(DependencyPurpose, DependencySourceRequest)>, DependencyProjectionError> {
    extract_scoped_requests_from_source(source)
}
