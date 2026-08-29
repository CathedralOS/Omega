//! Command-facing package-manager operations.
//!
//! Start here when looking for a complete user operation. Lower-level modules
//! expose the typed stages these workflows compose.

pub mod source_audit;

pub use source_audit::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, audit_package_source,
    audit_package_source_locator,
};
