//! State graph lowering and transition analysis will live here.
//!
//! This is Omega's equivalent of a compiler middle representation: machines,
//! states, operations, transitions, guards, and continuations.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphStage;
