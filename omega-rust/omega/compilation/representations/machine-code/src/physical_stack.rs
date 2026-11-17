//! Physical stack extents and call edges retained after register allocation.
use function_identity::MachineFunctionIdentity;
use semantic_vocabulary::MachineId;
use target::NativeTarget;
use target_vocabulary::CallSiteOwner;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalStackExtent {
    Finite(u64),
    Conflicting,
    Unaccounted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalStackCall {
    pub target: MachineId,
    pub owner: CallSiteOwner,
    pub caller_live_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignStackExtent {
    None,
    Admitted(task_plans::AdmittedSameStackContribution),
    Hosted { receipt: Option<[u8; 32]> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalForeignStackCall {
    pub owner: CallSiteOwner,
    pub caller_live_bytes: u32,
    pub extent: ForeignStackExtent,
    pub invoked_callbacks: Vec<MachineFunctionIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalStackFunction {
    pub machine: MachineId,
    pub local: LocalStackExtent,
    pub calls: Vec<PhysicalStackCall>,
    pub foreign_calls: Vec<PhysicalForeignStackCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalStackProgram {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub functions: Vec<PhysicalStackFunction>,
}
