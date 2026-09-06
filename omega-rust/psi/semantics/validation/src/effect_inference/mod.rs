//! Derive target-neutral effects from typed bodies and declared contracts.
//! Stored summaries live in `flow-effects`; fixed-point work stays here.

mod invocations;
mod operational;
mod service_reach;

pub use invocations::{
    declared_machine_invocations, declared_signature_invocations,
    has_self_forwarded_boundary_parameter, infer_synchronous_invocations, invocation_target_label,
};
pub use operational::infer_operational_may;
pub use service_reach::infer_service_reaches;
