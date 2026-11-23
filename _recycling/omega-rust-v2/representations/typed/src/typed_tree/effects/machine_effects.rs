use crate::{CrashCause, InvocationTarget, PauseEffects, ServiceApplication};
use omega_resolved::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct MachineEffects {
    pub invokes: Vec<InvocationTarget>,
    pub machine: DeclarationId,
    pub pause: PauseEffects,
    pub fixed_pause: PauseEffects,
    pub callback_pause: std::collections::BTreeMap<usize, PauseEffects>,
    /// Published service envelope for call admission, including contractual contributions from callees.
    pub reach: Vec<ServiceApplication>,
    /// Transitive services of a checked body. Interface-only declarations have no observed body row.
    pub realized_reach: Option<Vec<ServiceApplication>>,
    pub crash_ceiling: Option<Vec<CrashCause>>,
    pub terminates: bool,
}
