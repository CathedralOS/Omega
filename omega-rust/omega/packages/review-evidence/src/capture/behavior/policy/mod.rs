//! Checked callable behavior projection into receipt-free policy.

mod crash;
mod flow;
mod mutation;
mod receivers;
mod termination;

use crate::capture::refusal;
pub(crate) use crash::{crash, crash_routes};
pub(crate) use flow::{CapabilityFlowProjection, capability_flows};
pub(crate) use mutation::mutation;
pub(crate) use receivers::validate_call_receiver_roots;
pub(crate) use termination::{declared_termination, termination};

refusal!(message, "callable behavior policy: {message}");
