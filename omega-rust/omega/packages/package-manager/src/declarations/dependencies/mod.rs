//! Dependency rows within `build.omg`.
//!
//! [`read`] retains executed ledgers and parses conservative edit windows.
//! [`edit`] produces digest-bound conservative changes to those rows.

pub(crate) mod edit;
pub(crate) mod read;

pub(crate) use edit::{BuildDependencyEditPlan, BuildFileReplacement};
pub use read::DependencyPurpose;
pub(crate) use read::DependencySourceRequest;
pub(crate) use read::PackageSelection;
