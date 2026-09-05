//! The lock owns envelope framing and exact source associations only. Child
//! owners interpret source, compiler policy, and historical decision meaning.

mod budget;
mod framing;
mod reader;
mod writer;

const HEADER: &str = "omega_lock 1\n";
const MAXIMUM_SOURCE_BYTES: usize = 64 * 1024 * 1024;
const MAXIMUM_POLICY_TEXT_BYTES: usize = 32 * 1024 * 1024;
const MAXIMUM_DECISION_BYTES: usize = 8 * 1024 * 1024;
