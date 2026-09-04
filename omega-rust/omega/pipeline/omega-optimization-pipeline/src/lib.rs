#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Fail-closed optimized-native realization.
//!
//! Empty and nonempty selections share the same canonical Psi-phase entrance;
//! empty is the validated identity schedule. The optimizer begins at
//! [`coordination`], then descends through the named custody stages cataloged
//! by [`stages`].

mod coordination;
mod stages;

pub use coordination::*;
pub use omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan;
pub use stages::*;

#[cfg(test)]
mod tests;
