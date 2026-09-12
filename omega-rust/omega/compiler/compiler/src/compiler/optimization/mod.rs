//! Optimization selection controls; native compilation is owned by compiler::native.

#[cfg(any(test, feature = "experimental-external-optimization-policy"))]
pub(crate) mod external_policy;
pub mod rollback;

pub use rollback::{OptimizationRollback, OptimizationRollbackInputError};
