// Envelope framing and exact source associations for `omega.lock`, delegating source, policy, and decision semantics to child owners.

pub(super) mod acceptance;
mod budget;
mod compilation;
mod framing;
mod reader;
mod writer;

const HEADER: &str = "omega_lock 4\n";
const MAXIMUM_SOURCE_BYTES: usize = 64 * 1024 * 1024;
const MAXIMUM_POLICY_TEXT_BYTES: usize = 32 * 1024 * 1024;
const MAXIMUM_DECISION_BYTES: usize = 8 * 1024 * 1024;
