//! Translate validated Git requests into bounded native network operations.

use crate::error::SourceResolveError;
use crate::git::request::GitSourceRequest;
use crate::limits::LocalSourceLimits;
use omega_resolver_execution::ResolverExecutionRequestedEndpoint;
use std::ffi::OsString;

pub(super) fn requested_network_endpoint(
    request: &GitSourceRequest,
) -> Result<ResolverExecutionRequestedEndpoint, SourceResolveError> {
    ResolverExecutionRequestedEndpoint::new(
        request.requested_network_endpoint().host(),
        request.requested_network_endpoint().port(),
    )
    .map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
        message: format!("validated Git endpoint could not enter the native resolver: {error}"),
    })
}

pub(crate) fn bounded_git_fetch_arguments(
    fetch_locator: &str,
    requested_rev: &str,
    limits: LocalSourceLimits,
) -> Vec<OsString> {
    let first_inadmissible_blob_size = limits
        .max_bytes
        .checked_add(1)
        .expect("compiler-owned Git source byte ceiling leaves room for one sentinel byte");
    vec![
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--depth=1"),
        OsString::from("--no-tags"),
        OsString::from("--no-recurse-submodules"),
        OsString::from(format!(
            "--filter=blob:limit={first_inadmissible_blob_size}"
        )),
        OsString::from("--"),
        OsString::from(fetch_locator),
        OsString::from(requested_rev),
    ]
}
