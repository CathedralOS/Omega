//! Read-only package-source inspection.

mod execution;
mod report;
mod request;

pub use execution::{ inspect_package_source_locator};
pub(crate) use execution::{inspect_package_source};
pub(crate) use report::PackageSourceInspection;
pub use request::{
    SourceAdapter,
};
pub(crate) use request::{
    PackageSourceInspectionError, PackageSourceRequest, PackageSourceRequestParseError};
