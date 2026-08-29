//! Git and transport executable selection, content custody, and budgets.

mod budget;
mod custody;
mod executor;
mod identity;
mod selection;

pub(in crate::resolution::source) use budget::{
    CapturedOutputLimitExceeded, GitCapturedOutputBudget,
};
#[cfg(unix)]
#[allow(unused_imports)] // Preserve the former package-internal executable API.
pub(in crate::resolution::source) use custody::verify_macos_open_executable_acl_custody;
pub(in crate::resolution::source) use executor::GitExecutor;
#[cfg(test)]
pub(in crate::resolution::source) use executor::{
    test_file_network_endpoint, test_system_git_executor,
};
#[allow(unused_imports)] // Preserve the former package-internal executable API.
pub(in crate::resolution::source) use identity::GitExecutableMetadataIdentity;
#[allow(unused_imports)] // Preserve the former package-internal executable API.
pub(in crate::resolution::source) use selection::{
    GitTransportExecutableObservation, open_git_transport_executable,
    open_https_transport_executable, resolver_connect_helper_path, system_git_candidates,
    verify_git_transport_executable,
};
