// Authored-order extraction of unconditional direct dependency rows for conservative edit planning.

use super::error::DependencyProjectionError;
use super::projection::extract_scoped_requests_from_source;
use super::{DependencyPurpose, DependencySourceRequest};

pub(crate) fn extract_scoped_from_source(
    source: &str,
) -> Result<Vec<(DependencyPurpose, DependencySourceRequest)>, DependencyProjectionError> {
    extract_scoped_requests_from_source(source)
}
