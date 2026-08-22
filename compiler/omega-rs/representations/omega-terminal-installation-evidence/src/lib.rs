#![forbid(unsafe_code)]

//! Read-only projections of admitted installation evidence consumed below
//! Omega orchestration.
//!
//! These traits carry no constructors and grant no authority. Orchestration
//! owns the sealed values that implement them; lowering and image emission can
//! inspect only the exact identities required to preserve those decisions.

use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalFuelAttributionSite {
    Operation(psi_core::OperationId),
    Edge(psi_core::EdgeId),
}

/// Read-only normalized projection of one byte-validated native fuel site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalFuelAttributionEvidence {
    pub machine: psi_core::MachineId,
    pub schedule: psi_core::FuelScheduleIdentity,
    pub site: TerminalFuelAttributionSite,
    pub units: u64,
    pub operation_ordinal: usize,
    pub text_offset: usize,
    pub byte_count: usize,
}

/// Exact admitted provider-execution identity projected into terminal
/// lowering and installation records.
pub trait TerminalProviderExecutionEvidence: std::fmt::Debug {
    fn provider_plan(&self) -> u64;
    fn provider_execution_identity(&self) -> u64;
    fn provider_execution_fingerprint(&self) -> u64;
    fn normalized_root_identity(&self) -> u64;
    fn boundary_contract_fingerprint(&self) -> u64;
}

/// Relocation-free terminal object facts required to bind installed entry and
/// fixed-fuel evidence.
pub trait TerminalObjectEvidence {
    fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity;
    fn target(&self) -> omega_target::NativeTarget;
    fn architecture(&self) -> omega_target::Architecture {
        self.target().architecture
    }
    fn text_bytes(&self) -> &[u8];
    fn function_text_offset(&self, machine: psi_core::MachineId) -> Option<usize>;
    fn fuel_attribution(&self) -> Vec<TerminalFuelAttributionEvidence>;
}

/// Emitter-derived stack closure for one terminal entry.
pub trait TerminalStackDemandEvidence {
    fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity;
    fn architecture(&self) -> omega_target::Architecture;
    fn entry(&self) -> psi_core::MachineId;
    fn ceiling_bytes(&self) -> u64;
    fn stack_alignment(&self) -> u32;
    fn contributing_machines(&self) -> &BTreeSet<psi_core::MachineId>;
}
