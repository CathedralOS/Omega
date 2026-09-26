//! Read-only package-source inspection.

mod execution;
mod report;
mod request;

pub use execution::{inspect_package_source, inspect_package_source_locator};
pub use report::PackageSourceInspection;
pub use request::{
    PackageSourceInspectionError, PackageSourceRequest, PackageSourceRequestParseError,
    SourceAdapter,
};
