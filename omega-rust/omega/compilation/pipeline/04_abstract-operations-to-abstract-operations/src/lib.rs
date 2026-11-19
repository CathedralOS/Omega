//! Optimize the admitted abstract program and carry its checked facts into target lowering.
#![forbid(unsafe_code)]

mod abstract_optimization;
pub use abstract_optimization::{
    AbstractOptimizationError, ValidatedOptimizedAbstractPlan, optimize_abstract_operations,
};
#[cfg(feature = "test-support")]
pub use abstract_optimization::{
    VerifiedPsiOptimizationUnitBuildError, build_verified_psi_optimization_unit,
};
