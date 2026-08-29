#![forbid(unsafe_code)]

//! Custody-preserving projection from a completed Psi optimization run to
//! executable abstract operations.
//!
//! `projection` owns the single project-and-independently-validate join. Its
//! output retains the optimizer run, replay ledger, verifier context, manifest,
//! and validation receipt. Target selection and native orchestration remain
//! downstream.

mod projection;

pub use projection::{
    OptimizedAbstractProjectionError, ValidatedOptimizedAbstractPlan, project_optimization_run,
};
