//! Hermetic projection of literal dependency requests from `build.omg`.

mod error;
mod extraction;
mod model;
mod policy;
mod projection;
mod source_literal;

pub use error::DependencyProjectionError;
pub use extraction::{extract_build_dependency_projection, extract_dependency_projection};
pub use model::{BuildDependencyProjection, DependencySourceRequest, PackageSelection};

pub(crate) use extraction::extract_from_source;
pub(crate) use projection::validate_static_dependency_source;

#[cfg(test)]
mod tests;
