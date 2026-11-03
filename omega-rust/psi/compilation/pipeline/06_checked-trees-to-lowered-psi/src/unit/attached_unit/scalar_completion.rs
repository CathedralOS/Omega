//! Scalar completion rejoins the exact final source occurrence and its live
//! scalar binding. The statement schedule owns evaluation, effects, and custody.
use super::{CheckedTrees, LoweringError};

pub(super) mod control;
