//! Precedence rules for joined command, boundary, and custody outcomes.

use crate::source::SourceResolveError;

pub(in crate::source) fn reconcile_git_command_endpoint_result<T>(
    result: Result<T, SourceResolveError>,
    endpoint_result: Result<(), SourceResolveError>,
    executable_result: Result<(), SourceResolveError>,
    budget_result: Result<(), SourceResolveError>,
) -> Result<T, SourceResolveError> {
    match (result, endpoint_result, executable_result, budget_result) {
        (Err(error @ SourceResolveError::GitCleanupFailed { .. }), _, _, _) => Err(error),
        (_, Err(error), _, _) => Err(error),
        (_, _, Err(error), _) => Err(error),
        (_, _, _, Err(error)) => Err(error),
        (result, Ok(()), Ok(()), Ok(())) => result,
    }
}

pub(in crate::source) fn reconcile_git_command_result<T>(
    result: Result<T, SourceResolveError>,
    executable_result: Result<(), SourceResolveError>,
    budget_result: Result<(), SourceResolveError>,
) -> Result<T, SourceResolveError> {
    match (result, executable_result, budget_result) {
        (Err(error @ SourceResolveError::GitCleanupFailed { .. }), _, _) => Err(error),
        (_, Err(error), _) => Err(error),
        (_, _, Err(error)) => Err(error),
        (result, Ok(()), Ok(())) => result,
    }
}

pub(in crate::source) fn reconcile_git_cache_operation_result<T>(
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
