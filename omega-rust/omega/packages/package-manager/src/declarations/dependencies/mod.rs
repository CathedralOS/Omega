//! Dependency rows within `build.omg`.
//!
//! [`read`] projects exact authored rows without evaluating build logic.
//! [`edit`] produces digest-bound conservative changes to those rows.

pub(crate) mod edit;
pub(crate) mod read;

pub(crate) use edit::{BuildDependencyEditPlan, BuildFileReplacement};
pub use read::DependencyPurpose;
pub(crate) use read::PackageSelection;
pub(crate) use read::{DependencyProjectionError, DependencySourceRequest};
