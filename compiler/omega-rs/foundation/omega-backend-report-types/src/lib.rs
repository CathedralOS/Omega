//! Shared, foundational report/plan *data types* produced by the backend and
//! consumed by both backend report passes and the orchestration layer.
//!
//! These are pure data structures (no behaviour beyond a small constructor
//! helper). They were extracted out of `omega-artifacts` (orchestration layer)
//! so that backend crates (`omega-backend-report`, `omega-emission-planning`)
//! can depend on them *downward* (foundation) instead of reaching up into
//! orchestration. The orchestration-only `omega-artifacts` crate re-exports
//! these types and keeps the builder/IO helpers that genuinely depend on the
//! image and representation layers.

use omega_target::ObjectFormat;
use psi_arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionPlan {
    pub image_format: ObjectFormat,
    pub entry_symbol: String,
    pub sections: usize,
    pub symbols: usize,
    pub host_bindings: usize,
    pub host_calls: usize,
    pub data_bytes: usize,
    pub selected_instructions: usize,
    pub instruction_operands: usize,
    pub machine_code_bytes: usize,
    pub encoded_machine_bytes: usize,
    pub relocations: usize,
    pub blockers: Arena<EmissionBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmissionBlocker {
    pub stage: String,
    pub reason: String,
}

pub fn emission_blocker(stage: &str, reason: &str) -> EmissionBlocker {
    EmissionBlocker {
        stage: stage.to_owned(),
        reason: reason.to_owned(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackendSurfaceReport {
    pub entry_points: Arena<BackendEntryPoint>,
    pub machines: Arena<BackendMachineSurface>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackendEntryPoint {
    pub machine: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackendMachineSurface {
    pub name: String,
    pub contained_machines: usize,
    pub owned_data: usize,
    pub states: usize,
}
