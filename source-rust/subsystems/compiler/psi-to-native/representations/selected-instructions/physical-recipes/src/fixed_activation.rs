//! Locations retained by a Linux x86-64 activation while its worker or caller is parked.
use calling_conventions::MachineRegister;
pub use target_vocabulary::FixedActivationAction;

/// A SysV caller can keep these registers live across a call. Each side of
/// the transfer needs its own saved values and floating-point controls.
pub const SAVED_REGISTERS: [MachineRegister; 7] = [
    MachineRegister::X86Rsp,
    MachineRegister::X86Rbx,
    MachineRegister::X86Rbp,
    MachineRegister::X86R12,
    MachineRegister::X86R13,
    MachineRegister::X86R14,
    MachineRegister::X86R15,
];
const WORD: u32 = 8;
pub const MXCSR: u32 = SAVED_REGISTERS.len() as u32 * WORD;
pub const X87_CONTROL: u32 = MXCSR + 4;
const CONTEXT_BYTES: u32 = MXCSR + WORD;

// All offsets are from the start of the exclusive retained byte loan.
pub const CALLER_CONTEXT: u32 = 0;
pub const WORKER_CONTEXT: u32 = CALLER_CONTEXT + CONTEXT_BYTES;
pub const BACKING_LENGTH: u32 = WORKER_CONTEXT + CONTEXT_BYTES;
pub const PENDING_PUBLICATION: u32 = BACKING_LENGTH + WORD;
pub const REPLY_DESTINATION: u32 = PENDING_PUBLICATION + WORD;
pub const REPLY_TYPE: u32 = REPLY_DESTINATION + WORD;
pub const REPLY_SIZE: u32 = REPLY_TYPE + WORD;
pub const PHASE: u32 = REPLY_SIZE + WORD;
pub const BACKING_BASE: u32 = PHASE + WORD;
pub const CONTROL_BYTES: u32 = BACKING_BASE + WORD;

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Phase {
    Ready = 0,
    Running = 1,
    Parked = 2,
    Finished = 3,
}

/// The fixed worker runs on the stack its physical frame planner measured.
/// Its linked entry accepts the owner's address and writes back the result.
pub type FixedTransferPlan<Origin> = crate::PhysicalRecipe<Origin, FixedTransfer>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedTransfer {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub worker: semantic_vocabulary::MachineId,
    pub stack_top: u32,
    pub alignment: u32,
    pub argument_offset: u32,
    pub argument_bytes: u32,
    pub result_offset: u32,
    pub job: post_allocation_machine::adapters::AdapterInstructions,
}

impl<Origin> FixedTransferPlan<Origin> {
    pub fn transfer(&self) -> &FixedTransfer {
        &self.instructions
    }
}

/// The two post-call sequences differ only when the core operation returns a
/// tagged outcome. The nonzero path is Ready for prepare/resume and Parked for step.
pub struct FixedCall<Origin> {
    pub physical_identity: [u8; 32],
    pub frame: std::sync::Arc<machine_code::TargetFrameLayoutPlan>,
    pub binding: target_vocabulary::NormalizedForeignCallBinding,
    pub before: post_allocation_machine::adapters::AdapterInstructions,
    pub zero: post_allocation_machine::adapters::AdapterInstructions,
    pub nonzero: Option<post_allocation_machine::adapters::AdapterInstructions>,
    pub finish: post_allocation_machine::adapters::AdapterInstructions,
    pub transfer: Option<FixedTransferPlan<Origin>>,
    pub entry_bridge: Option<crate::native_entry_bridge::NativeEntryBridgePlan<Origin>>,
    pub caller_stack_bytes: u64,
}

pub type FixedCallPlan<Origin> = crate::PhysicalRecipe<Origin, FixedCall<Origin>>;
impl<Origin> FixedCallPlan<Origin> {
    pub fn call(&self) -> &FixedCall<Origin> {
        &self.instructions
    }
}
