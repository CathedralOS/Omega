//! Complete package-manager workflows composed from lower-level owners.
//!
//! [`prepare_local_project`] is the ordinary compiler entrance for a project
//! with `build.omg`. [`audit_source`] acquires and inspects one source without
//! admitting it. Future install and update transactions belong beside these
//! workflows rather than in the command-line binary.

pub mod audit_source;
mod local_project;

pub use audit_source::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, audit_package_source,
    audit_package_source_locator,
};
pub use local_project::{PrepareLocalProjectError, PreparedLocalProject, prepare_local_project};
