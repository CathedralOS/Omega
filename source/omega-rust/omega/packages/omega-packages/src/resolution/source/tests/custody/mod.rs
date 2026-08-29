//! Cache custody behavior, organized by the invariant under test.

mod identity;
mod limits;
mod locks;
#[cfg(target_os = "macos")]
mod macos_acl;
mod publication;
mod repository_integrity;
mod snapshots;
#[cfg(unix)]
mod traversal;
