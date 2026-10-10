//! Read-only package-source inspection.

mod execution;
mod report;
mod request;

pub use execution::inspect_package_source_locator;
pub(crate) use request::PackageSourceRequest;
pub use request::SourceAdapter;
