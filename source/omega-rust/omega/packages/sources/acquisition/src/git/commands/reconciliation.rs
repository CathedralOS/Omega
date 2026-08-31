//! Precedence rules for joined command, boundary, and custody outcomes.

use crate::SourceResolveError;

pub(crate) fn reconcile_git_command_result<T>(
    result: Result<T, SourceResolveError>,
    budget_result: Result<(), SourceResolveError>,
) -> Result<T, SourceResolveError> {
    match (result, budget_result) {
        (Err(error @ SourceResolveError::GitCleanupFailed { .. }), _) => Err(error),
        (_, Err(error)) => Err(error),
        (result, Ok(())) => result,
    }
}

pub(crate) fn reconcile_git_cache_operation_result<T>(
    operation_result: Result<T, SourceResolveError>,
    namespace_result: Result<(), SourceResolveError>,
    invalidation_result: Option<Result<(), SourceResolveError>>,
) -> Result<T, SourceResolveError> {
    if let Err(error) = namespace_result {
        return Err(error);
    }
    if let Some(Err(error)) = invalidation_result {
        return Err(error);
    }
    operation_result
}
