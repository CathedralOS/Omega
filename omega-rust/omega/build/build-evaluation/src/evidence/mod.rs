//! Evaluation evidence: durable observation records and their identity,
//! the filesystem scope that sponsors a run, and the replay records and
//! eligibility rules that decide when a run replays exactly.
//! `execution.rs` is the route that consults them.

pub(crate) mod filesystem_scope;
pub(crate) mod observation_identity;
pub(crate) mod observations;
pub(crate) mod replay_eligibility;
pub(crate) mod replay_record;
