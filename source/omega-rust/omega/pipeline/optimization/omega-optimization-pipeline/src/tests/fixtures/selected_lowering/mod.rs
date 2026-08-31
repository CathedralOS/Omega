//! Optimizer module role: stage group. Selected-lowering fixtures are
//! classified by the optimization behavior each source graph is designed to exercise.

mod baseline;
mod pressure_recovery;
mod widened_literal_folds;

pub(crate) use baseline::*;
pub(crate) use pressure_recovery::*;
pub(crate) use widened_literal_folds::*;
