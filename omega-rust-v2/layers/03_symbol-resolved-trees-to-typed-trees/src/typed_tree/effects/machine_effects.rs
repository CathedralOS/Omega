use crate::{CrashCause, InvocationTarget, PauseEffects, ServiceApplication};
use omega_resolver::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct MachineEffects {
    pub invokes: Vec<InvocationTarget>,
    pub machine: DeclarationId,
    pub pause: PauseEffects,
    pub fixed_pause: PauseEffects,
    pub callback_pause: std::collections::BTreeMap<usize, PauseEffects>,
    pub reach: Vec<ServiceApplication>,
    pub crash_ceiling: Option<Vec<CrashCause>>,
    pub terminates: bool,
}
