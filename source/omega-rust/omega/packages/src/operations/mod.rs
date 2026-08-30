//! Complete package-manager operations composed from lower-level owners.
//!
//! [`prepare_local_project`] is the ordinary compiler entrance for a project
//! with `build.omg`. [`inspect_source`] acquires and inspects one source without
//! admitting it. Future install and update transactions belong beside these
//! operations rather than in the command-line binary.

pub mod inspect_source;
mod prepare_project;

pub use inspect_source::{
    PackageSourceInspection, PackageSourceInspectionError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, inspect_package_source,
    inspect_package_source_locator,
};
pub use prepare_project::{PrepareLocalProjectError, PreparedLocalProject, prepare_local_project};
