//! Complete package-manager operations composed from lower-level owners.
//!
//! Start with [`audit_source`] to acquire and inspect one source without
//! admitting it.

pub mod audit_source;

pub use audit_source::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, audit_package_source,
    audit_package_source_locator,
};
