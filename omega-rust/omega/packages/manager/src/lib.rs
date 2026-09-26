#![forbid(unsafe_code)]

//! The compiler and command operations for Omega's registry-free package
//! manager.
//!
//! Start with [`package_manager`] for command dispatch, review and publication.
//! [`operations`] also exposes compiler preparation and read-only inspection.

pub mod admission;
pub mod declarations;
pub mod lock;
pub mod operations;
pub mod package_manager;
pub mod resolution;
pub mod review;

pub use package_manager::{
    PackageCommand, PackageCommandError, PackageCommandKind, PackageCommandOptions,
    PackageCommandOutcome, PackageCommandStatus, execute_package_command,
};

// The unit tests compile real packages on several threads; mimalloc's
// per-thread heaps avoid the system allocator's zone locks. Test-only: the
// product keeps the system allocator.
#[cfg(test)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
