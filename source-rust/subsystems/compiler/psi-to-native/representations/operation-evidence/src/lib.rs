//! Source coordinates, charged work, and ownership facts carried through native lowering.
//! Each row records what was established at an operation, block or control-flow edge.

#![forbid(unsafe_code)]

pub mod completion_claims;
pub mod ownership_frontier;
pub mod provenance;
pub mod provider_calls;
