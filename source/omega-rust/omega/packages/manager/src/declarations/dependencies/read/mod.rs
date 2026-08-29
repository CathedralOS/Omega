//! Hermetic projection of literal dependency requests from `build.omg`.

mod active_aliases;
mod error;
mod extraction;
mod model;
mod policy;
mod projection;
mod source_literal;

pub use active_aliases::{ActiveDependencyAliasError, ActiveDependencyAliasScope};
pub use error::{DependencyPathProvenance, DependencyPathTaint, DependencyProjectionError};
pub use extraction::{extract_build_dependency_projection, extract_dependency_projection};
pub use model::{
    BuildDependencyProjection, DependencySourceRequest, PackageSelection, ProjectedDependencies,
    TARGET_DEPENDENCY_CONDITION_SCHEMA_VERSION, TargetDependencyColumn,
    TargetDependencyConditionSchema,
};

pub(crate) use extraction::extract_from_source;
pub(crate) use projection::validate_static_dependency_source;

#[cfg(test)]
mod tests;
