//! Optimizer module role: stage group. Atomic projected-roster producer and independent replay.

mod replay;
mod source;

pub(super) use replay::replay;
pub(super) use source::derive;
